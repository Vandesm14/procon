use std::{
  collections::{BTreeMap, HashMap, HashSet},
  fs::{self},
  path::{Path, PathBuf},
  process::{Command, Stdio},
  str::FromStr,
  sync::LazyLock,
};

use clap::{Parser, Subcommand, command};
use serde::{Deserialize, Serialize};
use systemctl::SystemCtl;
use walkdir::WalkDir;

pub static SELF_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
  // Absolute path to the currently running executable.
  // On Linux this resolves /proc/self/exe to a real path.
  let exe = std::env::current_exe().expect("cannot get current exe");
  // If it's a symlink, canonicalize to the real file (best-effort).
  exe.canonicalize().unwrap_or(exe)
});

// pub static NIX_SHELL_PATH: LazyLock<PathBuf> =
//   LazyLock::new(|| which::which("nix-shell").unwrap());
pub static NIX_SHELL_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
  PathBuf::from_str("/nix/var/nix/profiles/default/bin/nix-shell").unwrap()
});

pub static IS_DEVELOPMENT_MODE: LazyLock<bool> = LazyLock::new(|| {
  std::env::var_os("ENVIRONMENT").is_some_and(|v| v == "development")
});

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
  #[arg(short, long)]
  path: Option<PathBuf>,
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  Plan { projects: Vec<String> },
  Apply { projects: Vec<String> },
  Clean { projects: Vec<String> },
  RunProxy { project_name: String },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Instance {
  path: PathBuf,
  projects: HashMap<String, Project>,
}

impl Instance {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      projects: HashMap::new(),
    }
  }

  pub fn artifacts_path(&self) -> PathBuf {
    self.path.join("artifacts")
  }

  pub fn projects_path(&self) -> PathBuf {
    self.path.join("projects")
  }

  pub fn state_path(&self) -> PathBuf {
    self.path.join("state.ron")
  }

  pub fn services_path(&self) -> PathBuf {
    dirs::config_dir().unwrap().join("systemd/user")
  }

  pub fn from_path(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let path = path.canonicalize().unwrap();

    let mut projects: Vec<Project> = Vec::new();
    let mut instance = Self::new(path);

    for file in WalkDir::new(instance.projects_path())
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|f| f.file_name().to_str().unwrap().ends_with(".toml"))
    {
      let project_toml: ProjectToml =
        toml::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();

      let project = Project::from_project_toml(
        project_toml,
        file.path().parent().unwrap().canonicalize()?,
      );
      projects.push(project);
    }

    instance.projects =
      HashMap::from_iter(projects.into_iter().map(|p| (p.name.clone(), p)));

    Ok(instance)
  }

  pub fn write_state(&self) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(self.state_path(), ron::to_string(self)?)?;
    Ok(())
  }

  pub fn compare(
    &self,
    project_filter: Option<&[String]>,
  ) -> HashMap<String, ConfigChange> {
    let mut changes: HashMap<String, ConfigChange> = HashMap::new();
    let old_state = fs::read_to_string(self.state_path())
      .ok()
      .and_then(|s| ron::from_str::<Instance>(&s).ok())
      .unwrap_or_default();
    for project in self
      .projects
      .values()
      .filter(|p| project_filter.map(|f| f.contains(&p.name)).unwrap_or(true))
    {
      if let Some(old_project) = old_state.projects.get(&project.name) {
        if old_project != project {
          changes.insert(project.name.clone(), ConfigChange::Changed);
        }
      } else {
        changes.insert(project.name.clone(), ConfigChange::Added);
      }
    }

    for project in old_state
      .projects
      .values()
      .filter(|p| project_filter.map(|f| f.contains(&p.name)).unwrap_or(true))
    {
      if !self.projects.contains_key(&project.name) {
        changes.insert(project.name.clone(), ConfigChange::Removed);
      }
    }

    changes
  }

  pub fn plan(
    &self,
    project_filter: Option<&[String]>,
  ) -> BTreeMap<String, Vec<Phase>> {
    BTreeMap::from_iter(
      self
        .compare(project_filter)
        .into_iter()
        .map(|(name, change)| (name, change.to_phases())),
    )
  }

  pub fn make_actions(&self, project_filter: Option<&[String]>) -> Vec<Action> {
    let plan = self.plan(project_filter);
    let mut actions: Vec<Action> = Vec::new();

    for (name, phases) in plan.iter() {
      if let Some(project) = self.projects.get(name) {
        for phase in phases.iter() {
          match phase {
            Phase::Setup => {
              actions.extend(project.source.setup(project, &self.path));
              if let Some(service) =
                project.service.generate_service_string(project, &self.path)
              {
                actions.push(Action::new(
                  &project.name,
                  Phase::Setup,
                  ActionKind::Filesystem(ActionKindFilesystem::Write(
                    project.service_path(&self.path),
                    service,
                  )),
                ));
                actions.push(Action::new(
                  &project.name,
                  Phase::Setup,
                  ActionKind::Filesystem(ActionKindFilesystem::Copy(
                    project.service_path(&self.path),
                    self
                      .services_path()
                      .join(format!("procon-proj-{}.service", project.name)),
                  )),
                ));
              }
              for cmd in project.phase.setup.to_vec() {
                actions.push(Action::new(
                  &project.name,
                  Phase::Setup,
                  ActionKind::Command(ActionKindCommand::NixShell(
                    project.source_path(&self.path),
                    project.deps_nix(),
                    Cmds::Single(cmd),
                  )),
                ));
              }
            }
            Phase::Update => {
              // TODO: Update source.
              for cmd in project.phase.setup.to_vec() {
                actions.push(Action::new(
                  &project.name,
                  Phase::Update,
                  ActionKind::Command(ActionKindCommand::NixShell(
                    project.source_path(&self.path),
                    project.deps_nix(),
                    Cmds::Single(cmd),
                  )),
                ));
              }
            }
            Phase::Build => {
              for cmd in project.phase.build.to_vec() {
                actions.push(Action::new(
                  &project.name,
                  Phase::Build,
                  ActionKind::Command(ActionKindCommand::NixShell(
                    project.source_path(&self.path),
                    project.deps_nix(),
                    Cmds::Single(cmd),
                  )),
                ));
              }
            }
            Phase::Start => {
              // TODO: This shouldn't be restart, but this is required for the step.
              if project.service.autostart {
                actions.push(Action::new(
                  &project.name,
                  Phase::Start,
                  ActionKind::SystemCtl(ActionKindSystemCtl::Restart(format!(
                    "procon-proj-{}.service",
                    project.name
                  ))),
                ));
              }
            }
            Phase::Stop => {
              actions.push(Action::new(
                &project.name,
                Phase::Stop,
                ActionKind::SystemCtl(ActionKindSystemCtl::Stop(format!(
                  "procon-proj-{}.service",
                  project.name
                ))),
              ));
            }
            Phase::Teardown => {
              // TODO: Teardown.
            }
          }
        }
      }
    }

    actions
  }

  pub fn apply(
    &self,
    project_filter: Option<&[String]>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    // Init services path.
    fs::create_dir_all(self.services_path()).unwrap();
    for (_, project) in self
      .projects
      .iter()
      .filter(|(p, _)| project_filter.map(|f| f.contains(p)).unwrap_or(true))
    {
      fs::create_dir_all(project.artifact_path(&self.path)).unwrap();
    }

    let mut skip: HashSet<String> = HashSet::new();
    let mut action_phases: BTreeMap<Phase, Vec<Action>> =
      self.make_actions(project_filter).into_iter().fold(
        BTreeMap::new(),
        |mut acc: BTreeMap<Phase, Vec<Action>>, action| {
          acc.entry(action.phase).or_default().push(action);
          acc
        },
      );

    for (phase, actions) in action_phases.iter_mut() {
      println!("Phase: {:?}.", phase);

      if *phase == Phase::Start {
        println!("Sub-Phase: Daemon Reload.");
        let systemctl = SystemCtl::builder()
          .additional_args(vec!["--user".to_string()])
          .build();
        systemctl
          .daemon_reload()
          .map_err(|e| format!("Failed to reload systemd daemon: {e:?}."))?;
      }

      for action in actions.iter_mut() {
        if skip.contains(&action.project_name) {
          action.mark_cancelled();
        } else {
          match &action.kind {
            ActionKind::Command(action_kind_command) => {
              let result = action_kind_command.apply(true);
              match result {
                Ok(output) => {
                  if output.status.success() {
                    action.mark_done()
                  } else {
                    let err = String::from_utf8(output.stderr);
                    action.mark_failed(format!("{err:?}"));
                    skip.insert(action.project_name.clone());
                  }
                }
                Err(err) => {
                  action.mark_failed(format!("{err:?}"));
                  skip.insert(action.project_name.clone());
                }
              }
            }
            ActionKind::Filesystem(action_kind_filesystem) => {
              let error = action_kind_filesystem.apply();
              if let Some(error) = error {
                action.mark_failed(format!("{error:?}"));
                skip.insert(action.project_name.clone());
              } else {
                action.mark_done();
              }
            }
            ActionKind::SystemCtl(action_kind_system_ctl) => {
              let result = action_kind_system_ctl.apply();
              match result {
                Ok(output) => {
                  if output.success() {
                    action.mark_done();
                  } else {
                    action.mark_failed("Systemctl failed.".to_owned());
                    skip.insert(action.project_name.clone());
                  }
                }
                Err(err) => {
                  action.mark_failed(format!("{err:?}"));
                  skip.insert(action.project_name.clone());
                }
              }
            }
          }
        }

        println!("{}", action);
      }
    }

    self.write_state()?;

    Ok(())
  }

  pub fn cmd_plan(
    &self,
    project_filter: Option<&[String]>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let plan = self.make_actions(project_filter);
    println!("plan: {plan:#?}");
    Ok(())
  }

  pub fn cmd_apply(
    &self,
    project_filter: Option<&[String]>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    self.apply(project_filter)?;
    Ok(())
  }

  pub fn cmd_clean(
    &self,
    project_filter: Option<&[String]>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    for project_name in self
      .projects
      .keys()
      .filter(|p| project_filter.map(|f| f.contains(p)).unwrap_or(true))
    {
      let project_artifact_path = self.artifacts_path().join(project_name);
      if fs::exists(&project_artifact_path).is_ok_and(|b| b) {
        fs::remove_dir_all(&project_artifact_path)?;
      }
    }

    Ok(())
  }

  pub fn cmd_run_proxy(
    &self,
    project_name: String,
  ) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(project) = self.projects.get(&project_name) {
      println!("Running {}", project.name);
      let command = ActionKindCommand::NixShell(
        project.source_path(&self.path),
        project.deps_nix(),
        project.phase.start.clone(),
      );
      match command.apply(false) {
        Ok(ok) => {
          if let Some(code) = ok.status.code() {
            println!("Exited with code: {}.", code);
          } else {
            println!("Process terminated by signal.");
          }
        }
        Err(err) => {
          println!("Action: {:?}", command);
          panic!("Execution error: {:?}", err);
        }
      }
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Hash)]
enum ConfigChange {
  Added,
  Changed,
  Removed,
}

impl ConfigChange {
  pub fn to_phases(self) -> Vec<Phase> {
    match self {
      ConfigChange::Added => vec![Phase::Setup, Phase::Build, Phase::Start],
      ConfigChange::Changed => {
        vec![Phase::Teardown, Phase::Setup, Phase::Build, Phase::Start]
      }
      ConfigChange::Removed => vec![Phase::Stop, Phase::Teardown],
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Phase {
  Teardown,
  Setup,
  Update,
  Build,
  Start,
  Stop,
}

#[derive(Debug)]
struct Action {
  project_name: String,
  phase: Phase,
  kind: ActionKind,
  status: ActionStatus,
}

impl std::fmt::Display for Action {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{:?}: {:?} {:?} {:?}",
      self.status, self.phase, self.project_name, self.kind
    )
  }
}

impl Action {
  fn new(project_name: &str, phase: Phase, kind: ActionKind) -> Self {
    Self {
      project_name: project_name.to_string(),
      phase,
      kind,
      status: ActionStatus::Todo,
    }
  }

  pub fn mark_todo(&mut self) {
    self.status = ActionStatus::Todo;
  }

  pub fn mark_done(&mut self) {
    self.status = ActionStatus::Done;
  }

  pub fn mark_failed(&mut self, reason: String) {
    self.status = ActionStatus::Fail(reason);
  }

  pub fn mark_cancelled(&mut self) {
    self.status = ActionStatus::Cancelled;
  }
}

#[derive(Debug)]
enum ActionKind {
  Command(ActionKindCommand),
  Filesystem(ActionKindFilesystem),
  SystemCtl(ActionKindSystemCtl),
}

#[derive(Debug, Clone, PartialEq)]
enum ActionKindCommand {
  GitClone(String, PathBuf),
  NixShell(PathBuf, Vec<String>, Cmds),
  Unzip(PathBuf, PathBuf),
}

impl ActionKindCommand {
  pub fn apply(&self, piped: bool) -> std::io::Result<std::process::Output> {
    match self {
      ActionKindCommand::GitClone(url, path) => {
        let mut cmd = Command::new("git");
        if piped {
          cmd.stdout(Stdio::piped());
          cmd.stderr(Stdio::piped());
        }

        cmd.arg("clone").arg(url).arg(path);
        cmd.output()
      }
      ActionKindCommand::NixShell(path, deps, cmds) => {
        let mut cmd = Command::new(NIX_SHELL_PATH.as_path());
        if piped {
          cmd.stdout(Stdio::piped());
          cmd.stderr(Stdio::piped());
        }

        cmd.current_dir(path);
        cmd
          .arg("-p")
          .args(deps)
          .arg("--run")
          .arg(cmds.to_vec().join("&&"));
        cmd.output()
      }
      ActionKindCommand::Unzip(from, to) => {
        let mut cmd = Command::new(NIX_SHELL_PATH.as_path());
        if piped {
          cmd.stdout(Stdio::piped());
          cmd.stderr(Stdio::piped());
        }

        cmd.arg("-p").arg("unzip").arg("--run").arg(format!(
          "unzip -o {} -d {}",
          from.display(),
          to.display()
        ));
        cmd.output()
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
enum ActionKindFilesystem {
  CreateDirAll(PathBuf),
  Copy(PathBuf, PathBuf),
  Write(PathBuf, String),
}

impl ActionKindFilesystem {
  pub fn apply(&self) -> Option<std::io::Error> {
    match self {
      ActionKindFilesystem::CreateDirAll(path) => {
        fs::create_dir_all(path).err()
      }
      ActionKindFilesystem::Copy(from, to) => fs::copy(from, to).err(),
      ActionKindFilesystem::Write(path, content) => {
        fs::write(path, content).err()
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
enum ActionKindSystemCtl {
  Restart(String),
  Start(String),
  Stop(String),
  Enable(String),
  Disable(String),
}

impl ActionKindSystemCtl {
  pub fn apply(&self) -> std::io::Result<std::process::ExitStatus> {
    let systemctl = SystemCtl::builder()
      .additional_args(vec!["--user".to_string()])
      .build();
    match self {
      ActionKindSystemCtl::Restart(unit) => systemctl.restart(unit),
      ActionKindSystemCtl::Start(unit) => systemctl.start(unit),
      ActionKindSystemCtl::Stop(unit) => systemctl.stop(unit),
      ActionKindSystemCtl::Enable(unit) => systemctl.enable(unit),
      ActionKindSystemCtl::Disable(unit) => systemctl.disable(unit),
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
enum ActionStatus {
  Todo,
  Done,
  Fail(String),
  Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Project {
  name: String,
  source: Source,
  deps: HashMap<String, Vec<String>>,
  phase: Phases,
  env: HashMap<String, String>,
  service: ServiceConfig,
  toml_path: PathBuf,
}

impl Project {
  pub fn from_project_toml(
    project_toml: ProjectToml,
    toml_path: PathBuf,
  ) -> Self {
    Self {
      name: project_toml.name,
      source: project_toml.source,
      deps: project_toml.deps,
      phase: project_toml.phase,
      env: project_toml.env,
      service: project_toml.service,
      toml_path,
    }
  }

  pub fn artifact_path(&self, path: &Path) -> PathBuf {
    path.join("artifacts").join(&self.name)
  }

  pub fn source_path(&self, path: &Path) -> PathBuf {
    self.artifact_path(path).join("source")
  }

  pub fn service_path(&self, path: &Path) -> PathBuf {
    self
      .artifact_path(path)
      .join(format!("{}.service", self.name))
  }

  pub fn deps_nix(&self) -> Vec<String> {
    self.deps.get("nix").cloned().unwrap_or(Vec::new())
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct ProjectToml {
  name: String,
  #[serde(default)]
  source: Source,
  #[serde(default)]
  deps: HashMap<String, Vec<String>>,
  #[serde(default)]
  phase: Phases,
  #[serde(default)]
  env: HashMap<String, String>,
  #[serde(default)]
  service: ServiceConfig,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Source {
  #[default]
  None,
  Path(PathBuf),
  Git(String),
  Zip(PathBuf),
}

impl Source {
  pub fn setup(&self, project: &Project, path: &Path) -> Vec<Action> {
    let source_path = project.source_path(path);
    let mut actions = Vec::new();
    match self {
      Source::None => return vec![],
      Source::Path(path_buf) => actions.push(Action::new(
        &project.name,
        Phase::Setup,
        ActionKind::Filesystem(ActionKindFilesystem::Copy(
          path_buf.to_path_buf(),
          source_path,
        )),
      )),
      Source::Git(url) => {
        actions.push(Action::new(
          &project.name,
          Phase::Setup,
          ActionKind::Command(ActionKindCommand::GitClone(
            url.to_string(),
            source_path,
          )),
        ));
      }
      Source::Zip(path_buf) => {
        actions.push(Action::new(
          &project.name,
          Phase::Setup,
          ActionKind::Command(ActionKindCommand::Unzip(
            project.toml_path.join(path_buf),
            source_path,
          )),
        ));
      }
    }

    actions
  }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Phases {
  /// Runs once, after the source and deps are installed.
  #[serde(default)]
  setup: Cmds,
  /// Runs on an update trigger.
  #[serde(default)]
  update: Cmds,
  /// Runs after an update.
  #[serde(default)]
  build: Cmds,
  /// Starts the project.
  #[serde(default)]
  start: Cmds,
  /// Stops the project.
  #[serde(default)]
  stop: Cmds,
  /// Runs on removal.
  #[serde(default)]
  teardown: Cmds,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(untagged)]
enum Cmds {
  #[default]
  None,
  Single(String),
  Many(Vec<String>),
}

impl Cmds {
  pub fn new(cmds: Vec<String>) -> Self {
    Self::Many(cmds)
  }

  pub fn new_single(cmd: String) -> Self {
    Self::Single(cmd)
  }

  pub fn to_vec(&self) -> Vec<String> {
    match self {
      Cmds::None => Vec::new(),
      Cmds::Single(single) => vec![single.clone()],
      Cmds::Many(items) => items.clone(),
    }
  }

  pub fn to_option(&self) -> Option<Vec<String>> {
    match self {
      Cmds::None => None,
      Cmds::Single(single) => Some(vec![single.clone()]),
      Cmds::Many(items) => Some(items.clone()),
    }
  }
}

fn get_true() -> bool {
  true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ServiceConfig {
  #[serde(default = "get_true")]
  autostart: bool,
  #[serde(default)]
  restart_on: RestartOn,
}

impl Default for ServiceConfig {
  fn default() -> Self {
    Self {
      autostart: true,
      restart_on: Default::default(),
    }
  }
}

impl ServiceConfig {
  pub fn generate_service_string(
    &self,
    project: &Project,
    path: &Path,
  ) -> Option<String> {
    let template = format!(
      r#"[Service]
  WorkingDirectory={}
  ExecStart={} run-proxy {}
  "#,
      path.display(),
      SELF_PATH.display(),
      project.name
    );
    Some(template)
  }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RestartOn {
  #[default]
  Never,
  Always,
  OnFailure,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  if *IS_DEVELOPMENT_MODE {
    println!("DEVELOPMENT MODE");
  }

  let cli = Cli::parse();
  let path: PathBuf = cli.path.unwrap_or(".".into());
  let instance = Instance::from_path(path).unwrap();

  match cli.command {
    Commands::Plan { projects } => {
      let filter = if projects.is_empty() {
        None
      } else {
        Some(projects.as_slice())
      };
      instance.cmd_plan(filter)
    }
    Commands::Apply { projects } => {
      let filter = if projects.is_empty() {
        None
      } else {
        Some(projects.as_slice())
      };
      instance.cmd_apply(filter)
    }
    Commands::Clean { projects } => {
      let filter = if projects.is_empty() {
        None
      } else {
        Some(projects.as_slice())
      };
      instance.cmd_clean(filter)
    }
    Commands::RunProxy { project_name } => instance.cmd_run_proxy(project_name),
  }
}
