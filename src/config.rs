use std::{
  collections::{HashMap, VecDeque},
  path::{Path, PathBuf},
};

use colored::Colorize;
use path_clean::PathClean;
use serde::Deserialize;

use crate::nix_shell;

fn substitute_args(cmd: &str, args: &HashMap<String, String>) -> String {
  let mut result = cmd.to_string();
  for (k, v) in args {
    result = result.replace(&format!("{{{{{k}}}}}"), v);
  }
  result
}

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
    project_name: &str,
    project_dir: &Path,
  ) -> std::process::Command
  where
    T: Iterator<Item = &'a String>,
  {
    match self {
      Cmds::Single(cmd) => nix_shell(
        path,
        deps,
        &[cmd.to_string()],
        true,
        project_name,
        project_dir,
      ),
      Cmds::Many(cmds) => {
        nix_shell(path, deps, cmds, true, project_name, project_dir)
      }
    }
  }

  pub fn to_vec(&self) -> Vec<String> {
    match self {
      Cmds::Single(cmd) => vec![cmd.to_string()],
      Cmds::Many(cmds) => cmds.clone(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ExecTask {
  task: String,
  #[serde(default)]
  with: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(untagged)]
pub enum Exec {
  Run { run: Cmds },
  Task(ExecTask),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Step {
  #[serde(flatten)]
  exec: Exec,
  #[serde(default)]
  pub deps: Vec<String>,
  #[serde(default)]
  pub cwd: Option<PathBuf>,
}

impl Step {
  pub fn assemble(config: &Config, step: &Step) -> Vec<String> {
    let mut cmds = Vec::new();
    let mut queue: VecDeque<(&Step, HashMap<String, String>)> = VecDeque::new();
    queue.push_back((step, HashMap::new()));

    while let Some((current, args)) = queue.pop_front() {
      match &current.exec {
        Exec::Run { run } => {
          for cmd in run.to_vec() {
            cmds.push(substitute_args(&cmd, &args));
          }
        }
        Exec::Task(exec_task) => {
          let task = config.tasks.get(&exec_task.task).expect("task not found");

          let missing_args: Vec<String> = task
            .args
            .iter()
            .filter(|arg| !exec_task.with.contains_key(*arg))
            .cloned()
            .collect();

          if !missing_args.is_empty() {
            panic!(
              "task '{}' requires arguments: {}, but only provided: {}",
              exec_task.task,
              missing_args.join(", "),
              exec_task
                .with
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
            );
          }

          let task_args = exec_task.with.clone();
          for task_step in &task.steps {
            queue.push_back((task_step, task_args.clone()));
          }
        }
      }
    }

    cmds
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Phase {
  steps: Vec<Step>,
}

impl Phase {
  pub fn run(
    &self,
    config: &Config,
    project: &Project,
    project_name: &str,
    dry_run: bool,
  ) -> bool {
    for step in self.steps.iter() {
      let path = if let Some(cwd) = &step.cwd {
        project.dir.join(cwd).clean()
      } else {
        project.dir.clone()
      };

      let cmds = Step::assemble(config, step);
      for cmd in cmds {
        let mut command = Cmds::Single(cmd).assemble(
          &path,
          if step.deps.is_empty() {
            None
          } else {
            Some(step.deps.iter())
          },
          project_name,
          &project.dir,
        );

        if dry_run {
          println!("would run: {command:?}");
        } else {
          println!("{}", format!("$ {command:?}").bold());
          match command.output() {
            Ok(output) => {
              if output.status.success() {
                for _ in output.stdout {
                  print!("\\33[2K");
                }
              } else {
                println!("failed.");
                return false;
              }
            }
            Err(e) => {
              println!("error: {e}");
            }
          }
        }
      }
    }

    true
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Project {
  pub dir: PathBuf,
  pub phases: HashMap<String, Phase>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Task {
  #[serde(default)]
  args: Vec<String>,
  steps: Vec<Step>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Config {
  pub projects: HashMap<String, Project>,
  #[serde(default)]
  pub tasks: HashMap<String, Task>,
  #[serde(default)]
  pub global: HashMap<String, Vec<Step>>,
}
