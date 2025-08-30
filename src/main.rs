use std::{
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
};

use clap::{Parser, Subcommand, command};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
  #[command(subcommand)]
  cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
  Debug { path: Option<PathBuf> },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Instance {
  path: PathBuf,

  modules: HashMap<String, Project>,
  projects: HashMap<String, Project>,
}

impl Instance {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      modules: HashMap::new(),
      projects: HashMap::new(),
    }
  }

  pub fn artifacts_path(path: &Path) -> PathBuf {
    path.join("artifacts")
  }

  pub fn modules_path(path: &Path) -> PathBuf {
    path.join("modules")
  }

  pub fn projects_path(path: &Path) -> PathBuf {
    path.join("projects")
  }

  pub fn state_path(path: &Path) -> PathBuf {
    path.join("state.ron")
  }

  pub fn from_path(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let mut modules: Vec<Project> = Vec::new();
    for file in fs::read_dir(Self::modules_path(&path))
      .unwrap()
      .filter_map(|e| e.ok())
    {
      let module: Project =
        toml::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();
      modules.push(module);
    }

    let mut projects: Vec<Project> = Vec::new();
    for file in fs::read_dir(Self::projects_path(&path))
      .unwrap()
      .filter_map(|e| e.ok())
    {
      let project: Project =
        toml::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();
      projects.push(project);
    }

    Ok(Self {
      path,
      modules: HashMap::from_iter(
        modules.into_iter().map(|m| (m.name.clone(), m)),
      ),
      projects: HashMap::from_iter(
        projects.into_iter().map(|p| (p.name.clone(), p)),
      ),
    })
  }

  pub fn write_state(&self) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(Self::state_path(&self.path), ron::to_string(self)?)?;
    Ok(())
  }

  pub fn entrypoint(&self) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
  }
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

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Phases {
  /// Runs once, before source and deps are installed.
  #[serde(default)]
  install: Cmds,
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
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(untagged)]
enum Cmds {
  #[default]
  None,
  Single(String),
  Many(Vec<String>),
}

fn main() {
  let cli = Cli::parse();
  match cli.cmd {
    Commands::Debug { path } => {
      let path = path.unwrap_or(".".into());
      let instance = Instance::from_path(path).unwrap();
      instance.write_state().unwrap();
    }
  }
}
