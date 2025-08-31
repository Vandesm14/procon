use std::{
  collections::{BTreeMap, HashMap, HashSet},
  fs::{self},
  path::{Path, PathBuf},
  process::{Command, ExitStatus},
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
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Instance {
  path: PathBuf,
  projects: HashMap<String, Project>,
}

impl Instance {
  pub fn projects_path(path: &Path) -> PathBuf {
    path.join("projects")
  }

  pub fn state_path(path: &Path) -> PathBuf {
    path.join("state.ron")
  }

  pub fn from_path(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let mut projects: Vec<Project> = Vec::new();
    for file in WalkDir::new(Self::projects_path(&path))
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|f| f.file_name().to_str().unwrap().ends_with(".toml"))
    {
      let project: Project =
        toml::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();
      projects.push(project);
    }

    let instance = Self {
      path,
      projects: HashMap::from_iter(
        projects.into_iter().map(|p| (p.name.clone(), p)),
      ),
    };

    Ok(instance)
  }

  pub fn write_state(&self) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(Self::state_path(&self.path), ron::to_string(self)?)?;
    Ok(())
  }

  pub fn compare(
    &self,
  ) -> Result<HashMap<String, ConfigChange>, Box<dyn std::error::Error>> {
    let mut changes: HashMap<String, ConfigChange> = HashMap::new();
    let old_state = fs::read_to_string(Self::state_path(&self.path))
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

    Ok(changes)
  }

  pub fn plan(
    &self,
  ) -> Result<BTreeMap<Phase, Vec<String>>, Box<dyn std::error::Error>> {
    let changes = self.compare()?;
    let mut plan: BTreeMap<Phase, Vec<String>> = BTreeMap::new();
    for (name, change) in changes.into_iter() {
      for (phase, name) in
        change.to_phases().into_iter().map(|p| (p, name.clone()))
      {
        if let Some(entry) = plan.get_mut(&phase) {
          entry.push(name);
        } else {
          plan.insert(phase, vec![name]);
        }
      }
    }

    Ok(plan)
  }

  pub fn apply(&self) -> Result<(), Box<dyn std::error::Error>> {
    let plan = self.plan()?;
    let mut skip: HashSet<String> = HashSet::new();

    for (phase, names) in plan.iter() {
      for project in names.iter().filter_map(|n| self.projects.get(n)) {
        println!("{phase:?} {}", project.name);
        match phase {
          Phase::Setup => {
            project.source.install(project, &self.path)?;
            if let Some(service) =
              project.service.generate_service(project, &self.path)
            {
              fs::write(
                project
                  .main_path(&self.path)
                  .join(format!("procon-{}.service", project.name)),
                service,
              )?;
            }
            for cmd in project.phase.setup.to_vec() {
              exit_status(
                project.nix_shell(&self.path, &cmd).status()?,
                &project.name,
                &cmd,
              );
            }
          }
          Phase::Update => {
            for cmd in project.phase.setup.to_vec() {
              exit_status(
                project.nix_shell(&self.path, &cmd).status()?,
                &project.name,
                &cmd,
              );
            }
            todo!("update source");
          }
          Phase::Build => {
            for cmd in project.phase.build.to_vec() {
              exit_status(
                project.nix_shell(&self.path, &cmd).status()?,
                &project.name,
                &cmd,
              );
            }
          }
          Phase::Start => todo!("start"),
          Phase::Stop => todo!("stop"),
          Phase::Teardown => todo!("teardown"),
        }
      }
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
}

#[derive(Debug, Clone, Copy, PartialEq, Hash)]
enum ConfigChange {
  Added,
  Changed,
  Removed,
}

impl ConfigChange {
  pub fn to_phases(&self) -> Vec<Phase> {
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
}

impl Project {
  pub fn main_path(&self, path: &Path) -> PathBuf {
    path.join("artifacts").join(&self.name)
  }

  pub fn source_path(&self, path: &Path) -> PathBuf {
    self.main_path(path).join("source")
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
  pub fn install(
    &self,
    project: &Project,
    path: &Path,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = project.source_path(path);
    fs::create_dir_all(&source_path)?;
    match self {
      Source::None => return Ok(()),
      Source::Path(path_buf) => {
        fs::copy(path_buf, source_path)?;
      }
      Source::Git(url) => {
        exit_status(
          Command::new("git")
            .arg("clone")
            .arg(url)
            .arg(source_path)
            .status()?,
          &project.name,
          &format!("git clone {}", url),
        );
      }
      Source::Zip(path_buf) => {
        exit_status(
          Command::new("nix-shell")
            .arg("-p")
            .arg("unzip")
            .arg("--run")
            .arg(format!(
              "unzip -o {} -d {}",
              path_buf.display(),
              source_path.display()
            ))
            .status()?,
          &project.name,
          &format!(
            "unzip -o {} -d {}",
            path_buf.display(),
            source_path.display()
          ),
        );
      }
    }

    Ok(())
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
  pub fn generate_service(
    &self,
    project: &Project,
    path: &Path,
  ) -> Option<String> {
    if let Some(cmds) = project.phase.start.to_option() {
      let template = format!(
        r#"[Service]
  DynamicUser=yes
  WorkingDirectory={}
  ExecStart=/usr/bin/env bash -c "nix-shell -p {} --run "{}""
  "#,
        project
          .main_path(path)
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
  }
}
