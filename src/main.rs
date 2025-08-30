use std::{
  collections::BTreeMap,
  fs,
  hash::{DefaultHasher, Hash, Hasher},
  io::Write,
  path::PathBuf,
  sync::Arc,
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

  modules: Vec<Project>,
  projects: Vec<Project>,

  existing_hashes: Hashes,
  current_hashes: Hashes,
}

impl Instance {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      modules: Vec::new(),
      projects: Vec::new(),
      existing_hashes: Hashes::new(),
      current_hashes: Hashes::new(),
    }
  }

  pub fn modules_path(&self) -> PathBuf {
    self.path.join("modules")
  }

  pub fn projects_path(&self) -> PathBuf {
    self.path.join("projects")
  }

  pub fn hashes_path(&self) -> PathBuf {
    self.path.join("hashes.toml")
  }

  pub fn read_from_path(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let mut current_hashes = Hashes::new();
    let existing_hashes: Hashes =
      if fs::exists(self.hashes_path()).is_ok_and(|x| x) {
        let str = fs::read_to_string(self.hashes_path()).unwrap();
        toml::from_str(&str)?
      } else {
        Hashes::new()
      };

    let mut modules: Vec<Project> = Vec::new();
    for path in fs::read_dir(self.modules_path())?
      .filter_map(|e| e.ok().map(|e| e.path()))
    {
      let module: Project = toml::from_str(&fs::read_to_string(path.clone())?)?;
      current_hashes.add_module(path, hash_once(module.clone()));
      modules.push(module);
    }

    let mut projects: Vec<Project> = Vec::new();
    for path in fs::read_dir(self.projects_path())?
      .filter_map(|e| e.ok().map(|e| e.path()))
    {
      let project: Project =
        toml::from_str(&fs::read_to_string(path.clone())?)?;
      current_hashes.add_project(path, hash_once(project.clone()));
      projects.push(project);
    }

    self.modules = modules;
    self.projects = projects;
    self.existing_hashes = existing_hashes;
    self.current_hashes = current_hashes;

    Ok(())
  }

  pub fn compare_hashes(&self) -> Vec<(PathBuf, Status)> {
    self.existing_hashes.compare(&self.current_hashes)
  }

  pub fn write_hashes(&self) -> Result<(), std::io::Error> {
    fs::write(
      self.hashes_path(),
      toml::to_string(&self.current_hashes).unwrap(),
    )
  }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Hashes {
  modules: BTreeMap<PathBuf, String>,
  projects: BTreeMap<PathBuf, String>,
}

impl Hashes {
  pub fn new() -> Self {
    Self {
      modules: BTreeMap::new(),
      projects: BTreeMap::new(),
    }
  }

  pub fn add_module(&mut self, module: PathBuf, hash: u64) {
    self.modules.insert(module, hash.to_string());
  }

  pub fn add_project(&mut self, project: PathBuf, hash: u64) {
    self.projects.insert(project, hash.to_string());
  }

  fn compare_list(
    some: &BTreeMap<PathBuf, String>,
    other: &BTreeMap<PathBuf, String>,
  ) -> Vec<(PathBuf, Status)> {
    let mut changes: Vec<(PathBuf, Status)> = Vec::new();
    for (path, hash) in some.iter() {
      if let Some(other_project) = other.get(path) {
        if hash != other_project {
          changes.push((path.clone(), Status::Changed));
        }
      } else {
        changes.push((path.clone(), Status::Added));
      }
    }

    for (path, _) in other.iter() {
      if !some.contains_key(path) {
        changes.push((path.clone(), Status::Removed));
      }
    }

    changes
  }

  pub fn compare_modules(&self, other: &Self) -> Vec<(PathBuf, Status)> {
    Self::compare_list(&self.modules, &other.modules)
  }

  pub fn compare_projects(&self, other: &Self) -> Vec<(PathBuf, Status)> {
    Self::compare_list(&self.projects, &other.projects)
  }

  pub fn compare(&self, other: &Self) -> Vec<(PathBuf, Status)> {
    let mut changes = Vec::new();
    changes.extend(self.compare_modules(other));
    changes.extend(self.compare_projects(other));
    changes
  }
}

#[derive(Debug, Clone, PartialEq)]
enum Status {
  Added,
  Removed,
  Changed,
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
struct Project {
  #[serde(default)]
  source: Source,
  #[serde(default)]
  deps: BTreeMap<String, Vec<String>>,
  #[serde(default)]
  phase: Phases,
  #[serde(default)]
  env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Source {
  #[default]
  None,
  Path(PathBuf),
  Git(String),
  Zip(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Hash, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Hash, Default, Serialize, Deserialize)]
#[serde(untagged)]
enum Cmds {
  #[default]
  None,
  Single(String),
  Many(Vec<String>),
}

fn hash_once<T>(hashable: T) -> u64
where
  T: Hash,
{
  let mut hasher = DefaultHasher::new();
  hashable.hash(&mut hasher);
  hasher.finish()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cli = Cli::parse();
  match cli.cmd {
    Commands::Debug { path } => {
      let path = path.unwrap_or(".".into());
      let mut instance = Instance::new(path);
      instance.read_from_path()?;

      println!("{:?}", instance.compare_hashes());

      instance.write_hashes()?;
    }
  }

  Ok(())
}
