use std::path::PathBuf;

use clap::{Parser, Subcommand, command};
use procon::{IS_SAFE_MODE, instance::Instance};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
  if *IS_SAFE_MODE {
    println!("SAFE MODE");
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
