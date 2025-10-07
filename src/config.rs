use std::{collections::HashMap, path::PathBuf};

use internment::Intern;
use serde::Deserialize;

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
pub struct Config {
  #[serde(default)]
  call: HashMap<Intern<String>, Vec<String>>,
  #[serde(default)]
  define: HashMap<Intern<String>, Definition>,
  #[serde(default)]
  phases: HashMap<Intern<String>, Cmds>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigHead {
  path: PathBuf,
  depth: usize,
}

impl ConfigHead {
  pub fn new(path: PathBuf, depth: usize) -> Self {
    Self { path, depth }
  }
}

#[derive(Debug, Default, Clone)]
pub struct Configs {
  configs: HashMap<ConfigHead, Config>,
}

impl Configs {
  pub fn new() -> Self {
    Self {
      configs: HashMap::new(),
    }
  }

  pub fn add(&mut self, head: ConfigHead, config: Config) {
    self.configs.insert(head, config);
  }

  pub fn get(&self, head: &ConfigHead) -> Option<&Config> {
    self.configs.get(head)
  }
}
