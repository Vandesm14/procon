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
  pub cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
  Debug { path: Option<PathBuf> },
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

      let mut projects: Vec<Project> = Vec::new();
      println!("projects:");
      for entry in fs::read_dir(project_path)? {
        let path = entry?.path();
        let string = fs::read_to_string(path)?;
        let project: Project = toml::from_str(&string)?;

        let mut hasher = DefaultHasher::new();
        project.hash(&mut hasher);
        let hash = hasher.finish();
        println!("hash: {hash}, project: {project:?}");
        projects.push(project);
      }
    }
  }

  Ok(())
}
