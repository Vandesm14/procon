use std::{collections::HashMap, path::PathBuf};

use internment::Intern;
use serde::Deserialize;

use crate::{multi::Multi, nix_shell};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Cmds {
  Single(String),
  Many(Vec<String>),
}

impl Cmds {
  pub fn assemble<'a, T>(
    &self,
    path: &PathBuf,
    deps: Option<T>,
  ) -> std::process::Command
  where
    T: Iterator<Item = &'a String>,
  {
    match self {
      Cmds::Single(cmd) => nix_shell(path, deps, &[cmd.to_string()], true),
      Cmds::Many(cmds) => nix_shell(path, deps, cmds, true),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Step {
  run: Cmds,
  #[serde(default)]
  deps: Multi<String>,
  #[serde(default)]
  cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Phase {
  steps: Vec<Step>,
}

impl Phase {
  pub fn run(&self, config: &Project, dry_run: bool) {
    for step in self.steps.iter() {
      let path = if let Some(cwd) = &step.cwd {
        &config.dir.join(cwd)
      } else {
        &config.dir
      };

      let mut command = step
        .run
        .assemble(path, step.deps.to_option().as_ref().map(|d| d.iter()));

      if dry_run {
        println!("would run: {command:?}");
      } else {
        println!("$ {command:?}");
        match command.output() {
          Ok(output) => {
            if output.status.success() {
              for _ in output.stdout {
                print!("\\33[2K");
              }
            } else {
              panic!("failed.");
            }
          }
          Err(e) => {
            println!("error: {e}");
          }
        }
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Project {
  pub dir: PathBuf,
  pub phases: HashMap<Intern<String>, Phase>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Config {
  pub projects: HashMap<String, Project>,
  #[serde(default)]
  pub global: HashMap<String, String>,
  #[serde(default)]
  pub enumerations: HashMap<String, Phase>,
}
