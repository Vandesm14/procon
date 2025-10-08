use std::path::PathBuf;

use clap::{Parser, Subcommand, command};
use internment::Intern;
use procon::instance::Instance;

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
  Debug,
  Run {
    cmds: Vec<Intern<String>>,
    #[arg(short, long)]
    dry_run: bool,
  },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cli = Cli::parse();
  let path: PathBuf = cli.path.unwrap_or(".".into());

  let instance = Instance::try_init(path).unwrap();

  match cli.command {
    Commands::Debug => {
      println!("{:#?}", instance);
    }
    Commands::Run { cmds, dry_run } => {
      instance.cmd_run(cmds, dry_run).unwrap();
    }
  }

  Ok(())
}
