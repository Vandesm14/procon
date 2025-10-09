use std::{collections::HashMap, path::PathBuf};

use internment::Intern;
use serde::Deserialize;

use crate::nix_shell;

type Deps = HashMap<Intern<String>, Vec<String>>;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Definition {
  Single(String),
  Many(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Cmds {
  Single(String),
  Many(Vec<String>),
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
    before: Vec<String>,
    #[serde(default)]
    after: Vec<String>,
  },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phase {
  pub cmds: Cmds,
  pub deps: Deps,
  pub before: Vec<String>,
  pub after: Vec<String>,
}

impl From<PhaseToml> for Phase {
  fn from(value: PhaseToml) -> Self {
    match value {
      PhaseToml::Cmds(cmds) => Self::from_cmds(cmds),
      PhaseToml::Expanded {
        cmds,
        deps,
        before,
        after,
      } => Self {
        cmds,
        deps,
        before,
        after,
      },
    }
  }
}

impl Phase {
  pub fn from_cmds(cmds: Cmds) -> Self {
    Self {
      cmds,
      deps: Default::default(),
      before: Default::default(),
      after: Default::default(),
    }
  }
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
