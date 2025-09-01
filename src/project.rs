use std::{
  collections::HashMap,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
  SELF_PATH,
  action::{
    Action, ActionKind, ActionKindCommand, ActionKindFilesystem, Phase,
  },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
  pub name: String,
  pub source: Source,
  pub deps: HashMap<String, Vec<String>>,
  pub phase: Phases,
  pub env: HashMap<String, String>,
  pub service: ServiceConfig,
  pub toml_path: PathBuf,
  pub status: ProjectStatus,
}

impl Project {
  pub fn non_status_equal(&self, other: &Self) -> bool {
    self.name == other.name
      && self.source == other.source
      && self.deps == other.deps
      && self.phase == other.phase
      && self.env == other.env
      && self.service == other.service
    // TODO: This means the same configs from different systems need to rebuild.
    // && self.toml_path == other.toml_path
  }

  pub fn from_project_toml(
    project_toml: ProjectToml,
    toml_path: PathBuf,
  ) -> Self {
    Self {
      name: project_toml.name,
      source: project_toml.source,
      deps: project_toml.deps,
      phase: project_toml.phase,
      env: project_toml.env,
      service: project_toml.service,
      status: ProjectStatus::default(),
      toml_path,
    }
  }

  pub fn artifact_path(&self, path: &Path) -> PathBuf {
    path.join("artifacts").join(&self.name)
  }

  pub fn source_path(&self, path: &Path) -> PathBuf {
    self.artifact_path(path).join("source")
  }

  pub fn service_path(&self, path: &Path) -> PathBuf {
    self.artifact_path(path).join("daemon.service")
  }

  pub fn deps_nix(&self) -> Vec<String> {
    self.deps.get("nix").cloned().unwrap_or(Vec::new())
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ProjectToml {
  pub name: String,
  #[serde(default)]
  pub source: Source,
  #[serde(default)]
  pub deps: HashMap<String, Vec<String>>,
  #[serde(default)]
  pub phase: Phases,
  #[serde(default)]
  pub env: HashMap<String, String>,
  #[serde(default)]
  pub service: ServiceConfig,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
  #[default]
  None,
  Path(PathBuf),
  Git(String),
  Zip(PathBuf),
}

impl Source {
  pub fn setup(&self, project: &Project, path: &Path) -> Vec<Action> {
    let source_path = project.source_path(path);
    let mut actions = Vec::new();
    match self {
      Source::None => return vec![],
      Source::Path(path_buf) => actions.push(Action::new(
        &project.name,
        Phase::Setup,
        ActionKind::Filesystem(ActionKindFilesystem::Copy(
          path_buf.to_path_buf(),
          source_path,
        )),
      )),
      Source::Git(url) => {
        actions.push(Action::new(
          &project.name,
          Phase::Setup,
          ActionKind::Command(ActionKindCommand::GitClone(
            url.to_string(),
            source_path,
          )),
        ));
      }
      Source::Zip(path_buf) => {
        actions.push(Action::new(
          &project.name,
          Phase::Setup,
          ActionKind::Command(ActionKindCommand::Unzip(
            project.toml_path.join(path_buf),
            source_path,
          )),
        ));
      }
    }

    actions
  }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Phases {
  /// Runs once, after the source and deps are installed.
  #[serde(default)]
  pub setup: Cmds,
  /// Runs on an update trigger.
  #[serde(default)]
  pub update: Cmds,
  /// Runs after an update.
  #[serde(default)]
  pub build: Cmds,
  /// Starts the project.
  #[serde(default)]
  pub start: Cmds,
  /// Stops the project.
  #[serde(default)]
  pub stop: Cmds,
  /// Runs on removal.
  #[serde(default)]
  pub teardown: Cmds,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Cmds {
  #[default]
  None,
  Single(String),
  Many(Vec<String>),
}

impl Cmds {
  pub fn new(cmds: Vec<String>) -> Self {
    Self::Many(cmds)
  }

  pub fn new_single(cmd: String) -> Self {
    Self::Single(cmd)
  }

  pub fn to_vec(&self) -> Vec<String> {
    match self {
      Cmds::None => Vec::new(),
      Cmds::Single(single) => vec![single.clone()],
      Cmds::Many(items) => items.clone(),
    }
  }

  pub fn to_option(&self) -> Option<Vec<String>> {
    match self {
      Cmds::None => None,
      Cmds::Single(single) => Some(vec![single.clone()]),
      Cmds::Many(items) => Some(items.clone()),
    }
  }
}

fn get_true() -> bool {
  true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServiceConfig {
  #[serde(default = "get_true")]
  pub autostart: bool,
  #[serde(default)]
  pub restart_on: RestartOn,
}

impl Default for ServiceConfig {
  fn default() -> Self {
    Self {
      autostart: true,
      restart_on: Default::default(),
    }
  }
}

impl ServiceConfig {
  pub fn generate_service_string(
    &self,
    project: &Project,
    path: &Path,
  ) -> Option<String> {
    let template = format!(
      r#"[Service]
  WorkingDirectory={}
  ExecStart={} run-proxy {}
  "#,
      path.display(),
      SELF_PATH.display(),
      project.name
    );
    Some(template)
  }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestartOn {
  #[default]
  Never,
  Always,
  OnFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum ProjectStatus {
  #[default]
  Added,
  Changed,
  Removed,

  Success,
  Failed(Phase),
}

impl ProjectStatus {
  pub fn to_phases(self) -> Vec<Phase> {
    match self {
      ProjectStatus::Success => vec![],
      ProjectStatus::Failed(phase) => match phase {
        Phase::Teardown => vec![Phase::Teardown],
        Phase::Setup => vec![Phase::Setup, Phase::Build, Phase::Start],
        Phase::Update => vec![Phase::Update, Phase::Build, Phase::Start],
        Phase::Build => vec![Phase::Build, Phase::Start],
        Phase::Start => vec![Phase::Start],
        Phase::Stop => vec![Phase::Stop],
      },
      ProjectStatus::Added => vec![Phase::Setup, Phase::Build, Phase::Start],
      ProjectStatus::Changed => {
        vec![Phase::Teardown, Phase::Setup, Phase::Build, Phase::Start]
      }
      ProjectStatus::Removed => vec![Phase::Stop, Phase::Teardown],
    }
  }
}
