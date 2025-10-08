use std::{collections::HashMap, path::PathBuf};

use internment::Intern;
use serde::Deserialize;

use crate::nix_shell;

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
  call: HashMap<Intern<String>, Vec<String>>,
  #[serde(default)]
  define: HashMap<Intern<String>, Definition>,
  #[serde(default)]
  phases: HashMap<Intern<String>, Cmds>,
  #[serde(default)]
  deps: HashMap<Intern<String>, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
  pub path: PathBuf,

  pub call: HashMap<Intern<String>, Vec<String>>,
  pub define: HashMap<Intern<String>, Definition>,
  pub phases: HashMap<Intern<String>, Cmds>,
  pub deps: HashMap<Intern<String>, Vec<String>>,
}

impl Config {
  pub fn from_config_toml(path: PathBuf, config_toml: ConfigToml) -> Self {
    Self {
      path,
      call: config_toml.call,
      define: config_toml.define,
      deps: config_toml.deps,
      phases: config_toml.phases,
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
