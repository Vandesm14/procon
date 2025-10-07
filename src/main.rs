use std::path::PathBuf;

use clap::{Parser, Subcommand, command};
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
enum Commands {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cli = Cli::parse();
  let path: PathBuf = cli.path.unwrap_or(".".into());

  let instance = Instance::try_init(path);

  match cli.command {}
}
