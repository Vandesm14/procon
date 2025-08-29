use std::{
  collections::BTreeMap,
  fs,
  hash::{DefaultHasher, Hash, Hasher},
  path::PathBuf,
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
struct Generations {
  modules: BTreeMap<PathBuf, String>,
  projects: BTreeMap<PathBuf, String>,
}

impl Generations {
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
      let modules_path = path.join("modules");
      let projects_path = path.join("projects");
      let generations_path = path.join("generations.toml");

      let generations: Generations =
        if fs::exists(&generations_path).is_ok_and(|x| x) {
          let str = fs::read_to_string(&generations_path).unwrap();
          toml::from_str(&str)?
        } else {
          Generations::new()
        };

      let mut new_generations = Generations::new();
      println!("modules:");
      for path in
        fs::read_dir(modules_path)?.filter_map(|e| e.ok().map(|e| e.path()))
      {
        let module: Project =
          toml::from_str(&fs::read_to_string(path.clone())?)?;
        let hash = hash_once(&module);
        println!("hash: {hash}, module: {module:?}");

        new_generations.add_module(path, hash);
      }

      println!();
      println!("projects:");
      for path in
        fs::read_dir(projects_path)?.filter_map(|e| e.ok().map(|e| e.path()))
      {
        let project: Project =
          toml::from_str(&fs::read_to_string(path.clone())?)?;
        let hash = hash_once(&project);
        println!("hash: {hash}, project: {project:?}");
        new_generations.add_project(path, hash);
      }

      println!();
      println!("changes: {:?}", new_generations.compare(&generations));

      fs::write(generations_path, toml::to_string(&new_generations)?)?
    }
  }

  Ok(())
}
