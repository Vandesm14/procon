use std::{collections::HashMap, path::PathBuf};

use internment::Intern;
use serde::Deserialize;

use crate::nix_shell;

type Deps = HashMap<Intern<String>, Vec<String>>;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Cmds {
  Single(String),
  Many(Vec<String>),
}

impl Cmds {
  pub fn run<'a, T>(
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

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(untagged)]
pub enum Multi<T> {
  #[default]
  None,
  Single(T),
  Many(Vec<T>),
}

impl<T> Multi<T> {
  pub fn to_vec(&self) -> Vec<T>
  where
    T: Clone,
  {
    match self {
      Multi::None => Vec::new(),
      Multi::Single(t) => vec![t.clone()],
      Multi::Many(ts) => ts.clone(),
    }
  }

  pub fn to_option(&self) -> Option<Vec<T>>
  where
    T: Clone,
  {
    match self {
      Multi::None => None,
      Multi::Single(t) => Some(vec![t.clone()]),
      Multi::Many(ts) => Some(ts.clone()),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum PhaseToml {
  Cmds(Cmds),
  Expanded {
    cmds: Cmds,
    #[serde(default)]
    deps: Deps,
    #[serde(default)]
    before: Multi<Intern<String>>,
  },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phase {
  pub cmds: Cmds,
  pub deps: Deps,
  pub before: Multi<Intern<String>>,
}

impl From<PhaseToml> for Phase {
  fn from(value: PhaseToml) -> Self {
    match value {
      PhaseToml::Cmds(cmds) => Self::from_cmds(cmds),
      PhaseToml::Expanded { cmds, deps, before } => Self { cmds, deps, before },
    }
  }
}

impl Phase {
  pub fn from_cmds(cmds: Cmds) -> Self {
    Self {
      cmds,
      deps: Default::default(),
      before: Default::default(),
    }
  }

  pub fn run(&self, config: &Config, dry_run: bool) {
    let mut command = self.cmds.run(
      &config.path,
      config.deps.get(&Intern::from_ref("nix")).map(|d| d.iter()),
    );

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

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ConfigToml {
  #[serde(default)]
  phases: HashMap<Intern<String>, PhaseToml>,
  #[serde(default)]
  deps: Deps,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
  pub path: PathBuf,

  pub phases: HashMap<Intern<String>, Phase>,
  pub deps: HashMap<Intern<String>, Vec<String>>,
}

impl Config {
  pub fn from_config_toml(path: PathBuf, config_toml: ConfigToml) -> Self {
    Self {
      path,
      deps: config_toml.deps,
      phases: config_toml
        .phases
        .into_iter()
        .map(|(key, val)| (key, Phase::from(val)))
        .collect(),
    }
  }
}

#[derive(Debug, Default, Clone)]
pub struct Configs {
  configs: Vec<Config>,
}

impl Configs {
  pub fn new() -> Self {
    Self {
      configs: Vec::new(),
    }
  }

  pub fn push(&mut self, config: Config) {
    self.configs.push(config);
  }

  pub fn iter(&self) -> impl Iterator<Item = &Config> {
    self.configs.iter()
  }
}
