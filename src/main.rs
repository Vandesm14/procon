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
  Init,
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

  match cli.command {
    Commands::Init => Instance::try_init(path)?.write_state(),
    Commands::Plan { projects } => {
      let instance = Instance::from_path(path)?;
      let filter = if projects.is_empty() {
        None
      } else {
        Some(projects.as_slice())
      };
      instance.cmd_plan(filter)
    }
    Commands::Apply { projects } => {
      let mut instance = Instance::from_path(path)?;
      let filter = if projects.is_empty() {
        None
      } else {
        Some(projects.as_slice())
      };
      instance.cmd_apply(filter)
    }
    Commands::Clean { projects } => {
      let instance = Instance::from_path(path)?;
      let filter = if projects.is_empty() {
        None
      } else {
        Some(projects.as_slice())
      };
      instance.cmd_clean(filter)
    }
    Commands::RunProxy { project_name } => {
      let instance = Instance::from_path(path)?;
      instance.cmd_run_proxy(project_name)
    }
  }
}
