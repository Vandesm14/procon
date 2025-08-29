use std::{
  collections::{BTreeMap, HashMap},
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
  projects: HashMap<PathBuf, String>,
}

impl Generations {
  pub fn new() -> Self {
    Self {
      projects: HashMap::new(),
    }
  }

  pub fn add(&mut self, project: PathBuf, hash: u64) {
    self.projects.insert(project, hash.to_string());
  }

  pub fn compare(&self, other: &Self) -> Vec<(PathBuf, Status)> {
    let mut changes: Vec<(PathBuf, Status)> = Vec::new();
    for (path, hash) in self.projects.iter() {
      if let Some(other_project) = other.projects.get(path) {
        if hash != other_project {
          changes.push((path.clone(), Status::Changed));
        }
      } else {
        changes.push((path.clone(), Status::Added));
      }
    }

    for (path, _) in other.projects.iter() {
      if !self.projects.contains_key(path) {
        changes.push((path.clone(), Status::Removed));
      }
    }

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
  source: Source,
  deps: BTreeMap<String, Vec<String>>,
  phase: Phases,
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Source {
  Path(PathBuf),
  Git(String),
  Zip(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Hash, Default, Serialize, Deserialize)]
#[serde(untagged)]
enum Cmds {
  #[default]
  None,
  Single(String),
  Many(Vec<String>),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cli = Cli::parse();
  match cli.cmd {
    Commands::Debug { path } => {
      let path = path.unwrap_or(".".into());
      let project_path = path.join("projects");
      let generations_path = path.join("generations.toml");

      let generations: Generations = toml::from_str(
        &fs::read_to_string(&generations_path).unwrap_or_default(),
      )?;

      let mut projects: Vec<Project> = Vec::new();
      let mut new_generations = Generations::new();
      println!("projects:");
      for entry in fs::read_dir(project_path)? {
        let path = entry?.path();
        let string = fs::read_to_string(path.clone())?;
        let project: Project = toml::from_str(&string)?;

        let mut hasher = DefaultHasher::new();
        project.hash(&mut hasher);
        let hash = hasher.finish();
        println!("hash: {hash}, project: {project:?}");
        projects.push(project);

        new_generations.add(path, hash);
      }

      println!();
      println!("changes: {:?}", new_generations.compare(&generations));

      fs::write(generations_path, toml::to_string(&new_generations)?)?
    }
  }

  Ok(())
}
