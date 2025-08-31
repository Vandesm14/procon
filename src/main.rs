use std::{
  collections::{BTreeMap, HashMap, HashSet},
  fs::{self},
  path::{Path, PathBuf},
  process::{Child, Command, ExitStatus, Stdio},
};

use clap::{Parser, Subcommand, command};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

fn exit_status(status: ExitStatus, project_name: &str, cmd: &str) {
  if let Some(code) = status.code()
    && code != 0
  {
    panic!(
      "Project {project_name} failed to run command: {cmd} with exit code: {code}"
    );
  }
}

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
  Plan,
  Apply,
  Clean,
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
    dirs::home_dir().unwrap().join("systemd/user/procon")
  }

  pub fn from_path(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let mut projects: Vec<Project> = Vec::new();
    let mut instance = Self::new(path);

    for file in WalkDir::new(instance.projects_path())
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|f| f.file_name().to_str().unwrap().ends_with(".toml"))
    {
      let mut project: Project =
        toml::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();
      project.config_path = file.path().parent().unwrap().canonicalize()?;
      projects.push(project);
    }

    instance.projects =
      HashMap::from_iter(projects.into_iter().map(|p| (p.name.clone(), p)));

    println!("load: {:?}", instance);

    Ok(instance)
  }

  pub fn write_state(&self) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(self.state_path(), ron::to_string(self)?)?;
    Ok(())
  }

  pub fn compare(&self) -> HashMap<String, ConfigChange> {
    let mut changes: HashMap<String, ConfigChange> = HashMap::new();
    let old_state = fs::read_to_string(self.state_path())
      .ok()
      .and_then(|s| ron::from_str::<Instance>(&s).ok())
      .unwrap_or_default();
    for project in self.projects.values() {
      if let Some(old_project) = old_state.projects.get(&project.name) {
        if old_project != project {
          changes.insert(project.name.clone(), ConfigChange::Changed);
        }
      } else {
        changes.insert(project.name.clone(), ConfigChange::Added);
      }
    }

    for project in old_state.projects.values() {
      if !self.projects.contains_key(&project.name) {
        changes.insert(project.name.clone(), ConfigChange::Removed);
      }
    }

    changes
  }

  pub fn plan(&self) -> BTreeMap<String, Vec<Phase>> {
    BTreeMap::from_iter(
      self
        .compare()
        .into_iter()
        .map(|(name, change)| (name, change.to_phases())),
    )
  }

  pub fn make_actions(&self) -> Vec<Action> {
    let plan = self.plan();
    let mut actions: Vec<Action> = Vec::new();

    for (name, phases) in plan.iter() {
      if let Some(project) = self.projects.get(name) {
        for phase in phases.iter() {
          match phase {
            Phase::Setup => {
              fs::create_dir_all(project.artifact_path(&self.path)).unwrap();
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
                      .join(format!("{}.service", project.name)),
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
              // TODO: Start.
            }
            Phase::Stop => {
              // TODO: Stop.
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

  pub fn apply(&self) -> Result<(), Box<dyn std::error::Error>> {
    // Init necessary paths.
    fs::create_dir_all(self.services_path()).unwrap();

    println!("Actions:");
    let mut skip: HashSet<String> = HashSet::new();
    let mut actions = self.make_actions();
    for action in actions.iter_mut() {
      if skip.contains(&action.project_name) {
        action.mark_cancelled();
      } else {
        match &action.kind {
          ActionKind::Command(action_kind_command) => {
            let result = action_kind_command.apply();
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
        }
      }

      println!(
        "{:?}: {:?} {:?} {:?}",
        action.status, action.phase, action.project_name, action.kind
      );
    }

    self.write_state()?;

    Ok(())
  }

  pub fn cmd_plan(&self) -> Result<(), Box<dyn std::error::Error>> {
    let plan = self.plan();
    println!("plan: {plan:#?}");
    Ok(())
  }

  pub fn cmd_apply(&self) -> Result<(), Box<dyn std::error::Error>> {
    self.apply()?;
    Ok(())
  }

  pub fn cmd_clean(&self) -> Result<(), Box<dyn std::error::Error>> {
    fs::remove_dir_all(self.artifacts_path())?;
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
}

#[derive(Debug, Clone, PartialEq)]
enum ActionKindCommand {
  GitClone(String, PathBuf),
  NixShell(PathBuf, Vec<String>, Cmds),
  Unzip(PathBuf, PathBuf),
}

impl ActionKindCommand {
  pub fn apply(&self) -> std::io::Result<std::process::Output> {
    match self {
      ActionKindCommand::GitClone(url, path) => {
        let mut cmd = Command::new("git");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        cmd.arg("clone").arg(url).arg(path);
        cmd.output()
      }
      ActionKindCommand::NixShell(path, deps, cmds) => {
        let mut cmd = Command::new("nix-shell");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        cmd.current_dir(path);
        cmd
          .arg("-p")
          .args(deps)
          .arg("--run")
          .arg(cmds.to_vec().join("&&"));
        cmd.output()
      }
      ActionKindCommand::Unzip(from, to) => {
        let mut cmd = Command::new("nix-shell");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

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
enum ActionStatus {
  Todo,
  Done,
  Fail(String),
  Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Project {
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
  #[serde(default)]
  config_path: PathBuf,
}

impl Project {
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

  pub fn nix_shell(&self, path: &Path, cmd: &str) -> Command {
    let mut command = Command::new("nix-shell");
    command
      .current_dir(self.source_path(path))
      .arg("-p")
      .args(self.deps.get("nix").unwrap_or(&Vec::new()))
      .arg("--run")
      .arg(cmd);

    command
  }
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
    let mut actions = vec![Action::new(
      &project.name,
      Phase::Setup,
      ActionKind::Filesystem(ActionKindFilesystem::CreateDirAll(
        source_path.clone(),
      )),
    )];
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
            project.config_path.join(path_buf),
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
    if let Some(cmds) = project.phase.start.to_option() {
      let template = format!(
        r#"[Service]
  DynamicUser=yes
  WorkingDirectory={}
  ExecStart=/nix/var/nix/profiles/default/bin/nix-shell -p {} --run "{}"
  "#,
        project
          .artifact_path(path)
          .canonicalize()
          .unwrap()
          .to_str()
          .unwrap(),
        project.deps_nix().join(" "),
        cmds.join("&&"),
      );
      Some(template)
    } else {
      None
    }
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
  let cli = Cli::parse();
  let path: PathBuf = cli.path.unwrap_or(".".into());
  let instance = Instance::from_path(path).unwrap();

  match cli.command {
    Commands::Plan => instance.cmd_plan(),
    Commands::Apply => instance.cmd_apply(),
    Commands::Clean => instance.cmd_clean(),
  }
}
