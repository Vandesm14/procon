use std::path::PathBuf;

use clap::{Parser, Subcommand};
use procon::instance::Instance;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
  /// Specify an alternate config file (default: procon.yaml)
  #[arg(short, long)]
  file: Option<PathBuf>,

  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  Debug,
  Run {
    /// Phase(s) to run
    phases: Vec<String>,

    /// Project name(s) to filter (if not specified, runs on all projects)
    #[arg(short, long)]
    projects: Vec<String>,

    /// Dry run. Prints out commands that procon will run instead of running
    /// them.
    #[arg(short = 'n', long)]
    dry_run: bool,
  },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cli = Cli::parse();
  let path: PathBuf = cli.file.unwrap_or("procon.yaml".into());

  let instance = Instance::try_init(path).unwrap();

  match cli.command {
    Commands::Debug => {
      println!("{:#?}", instance);
    }
    Commands::Run {
      projects,
      phases,
      dry_run,
    } => {
      let project_filter = if projects.is_empty() {
        None
      } else {
        Some(projects)
      };

      instance.cmd_run(phases, project_filter, dry_run).unwrap();
    }
  }

  Ok(())
}
