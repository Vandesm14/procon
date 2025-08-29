use std::{
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
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Source {
  Path(PathBuf),
  Git(String),
  Zip(PathBuf),
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
