use std::{
  fs,
  path::PathBuf,
  process::{Command, ExitStatus, Stdio},
};

use systemctl::SystemCtl;

use crate::{IS_SAFE_MODE, NIX_SHELL_PATH, project::Cmds};

#[derive(Debug, Clone, Copy, PartialEq, Hash)]
pub enum ConfigChange {
  Added,
  Changed,
  Removed,
}

impl ConfigChange {
  pub fn to_phases(self) -> Vec<Phase> {
    match self {
      ConfigChange::Added => vec![Phase::Setup, Phase::Build, Phase::Start],
      ConfigChange::Changed => {
        vec![Phase::Teardown, Phase::Setup, Phase::Build, Phase::Start]
      }
      ConfigChange::Removed => vec![Phase::Stop, Phase::Teardown],
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
  Teardown,
  Setup,
  Update,
  Build,
  Start,
  Stop,
}

#[derive(Debug)]
pub struct Action {
  pub project_name: String,
  pub phase: Phase,
  pub kind: ActionKind,
  pub status: ActionStatus,
}

impl std::fmt::Display for Action {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{:?}: {:?} {:?} {:?}",
      self.status, self.phase, self.project_name, self.kind
    )
  }
}

impl Action {
  pub fn new(project_name: &str, phase: Phase, kind: ActionKind) -> Self {
    Self {
      project_name: project_name.to_string(),
      phase,
      kind,
      status: ActionStatus::Todo,
    }
  }

  pub fn mark_todo(&mut self) {
    self.status = ActionStatus::Todo;
  }

  pub fn mark_done(&mut self) {
    self.status = ActionStatus::Done;
  }

  pub fn mark_failed(&mut self, reason: String) {
    self.status = ActionStatus::Fail(reason);
  }

  pub fn mark_cancelled(&mut self) {
    self.status = ActionStatus::Cancelled;
  }
}

#[derive(Debug)]
pub enum ActionKind {
  Command(ActionKindCommand),
  Filesystem(ActionKindFilesystem),
  SystemCtl(ActionKindSystemCtl),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionKindCommand {
  GitClone(String, PathBuf),
  NixShell(PathBuf, Vec<String>, Cmds),
  Unzip(PathBuf, PathBuf),
}

impl ActionKindCommand {
  pub fn apply(&self, piped: bool) -> std::io::Result<std::process::Output> {
    match self {
      ActionKindCommand::GitClone(url, path) => {
        let mut cmd = Command::new("git");
        if piped {
          cmd.stdout(Stdio::piped());
          cmd.stderr(Stdio::piped());
        }

        cmd.arg("clone").arg(url).arg(path);
        cmd.output()
      }
      ActionKindCommand::NixShell(path, deps, cmds) => {
        let mut cmd = Command::new(NIX_SHELL_PATH.as_path());
        if piped {
          cmd.stdout(Stdio::piped());
          cmd.stderr(Stdio::piped());
        }

        cmd.current_dir(path);
        cmd
          .arg("-p")
          .args(deps)
          .arg("--run")
          .arg(cmds.to_vec().join("&&"));
        cmd.output()
      }
      ActionKindCommand::Unzip(from, to) => {
        let mut cmd = Command::new(NIX_SHELL_PATH.as_path());
        if piped {
          cmd.stdout(Stdio::piped());
          cmd.stderr(Stdio::piped());
        }

        cmd.arg("-p").arg("unzip").arg("--run").arg(format!(
          "unzip -o {} -d {}",
          from.display(),
          to.display()
        ));
        cmd.output()
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionKindFilesystem {
  CreateDirAll(PathBuf),
  Copy(PathBuf, PathBuf),
  Write(PathBuf, String),
}

impl ActionKindFilesystem {
  pub fn apply(&self) -> Option<std::io::Error> {
    match self {
      ActionKindFilesystem::CreateDirAll(path) => {
        fs::create_dir_all(path).err()
      }
      ActionKindFilesystem::Copy(from, to) => fs::copy(from, to).err(),
      ActionKindFilesystem::Write(path, content) => {
        fs::write(path, content).err()
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionKindSystemCtl {
  Restart(String),
  Start(String),
  Stop(String),
  Enable(String),
  Disable(String),
}

impl ActionKindSystemCtl {
  pub fn apply(&self) -> std::io::Result<std::process::ExitStatus> {
    if *IS_SAFE_MODE {
      return Ok(ExitStatus::default());
    }

    let systemctl = SystemCtl::builder()
      .additional_args(vec!["--user".to_string()])
      .build();
    match self {
      ActionKindSystemCtl::Restart(unit) => systemctl.restart(unit),
      ActionKindSystemCtl::Start(unit) => systemctl.start(unit),
      ActionKindSystemCtl::Stop(unit) => systemctl.stop(unit),
      ActionKindSystemCtl::Enable(unit) => systemctl.enable(unit),
      ActionKindSystemCtl::Disable(unit) => systemctl.disable(unit),
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionStatus {
  Todo,
  Done,
  Fail(String),
  Cancelled,
}
