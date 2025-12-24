use std::{fs, path::PathBuf};

use colored::Colorize;
use path_clean::PathClean;

use crate::config::{Cmds, Config, Step};

#[derive(Debug, Clone, Default)]
pub struct Instance {
  config: Config,
  path: PathBuf,
}

impl Instance {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      config: Config::default(),
    }
  }

  pub fn try_init(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let mut instance = Instance::new(path.canonicalize()?);
    let content = fs::read_to_string(&instance.path)?;
    let config: Config = serde_norway::from_str(&content).unwrap_or_else(|e| {
      panic!("Failed to parse {}: {e}", instance.path.display())
    });
    instance.config = config;

    Ok(instance)
  }

  pub fn cmd_run(
    &self,
    phase_strings: Vec<String>,
    project_filter: Option<Vec<String>>,
    dry_run: bool,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let mut ignore: Vec<String> = Vec::new();
    for phase_string in phase_strings.into_iter() {
      for (project_name, project) in self.config.projects.iter() {
        if let Some(ref filter) = project_filter
          && !filter.contains(project_name)
        {
          continue;
        }

        if ignore.contains(project_name) {
          continue;
        }

        if let Some(phase) = project.phases.get(&phase_string)
          && !phase.run(&self.config, project, project_name, dry_run)
        {
          ignore.push(project_name.clone());
        }
      }
    }

    Ok(())
  }

  pub fn cmd_run_global(
    &self,
    keys: Vec<String>,
    dry_run: bool,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = self
      .path
      .parent()
      .unwrap_or_else(|| std::path::Path::new("."))
      .to_path_buf();

    for key in keys {
      let steps = self
        .config
        .global
        .get(&key)
        .ok_or_else(|| format!("global command '{}' not found", key))?;

      for step in steps.iter() {
        let path = if let Some(cwd) = &step.cwd {
          config_dir.join(cwd).clean()
        } else {
          config_dir.clone()
        };

        let cmds = Step::assemble(&self.config, step);
        for cmd in cmds {
          let mut command = Cmds::Single(cmd).assemble(
            &path,
            if step.deps.is_empty() {
              None
            } else {
              Some(step.deps.iter())
            },
            "global",
            &config_dir,
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
                  return Err(format!("global command '{}' failed", key).into());
                }
              }
              Err(e) => {
                println!("error: {e}");
                return Err(
                  format!("global command '{}' error: {}", key, e).into(),
                );
              }
            }
          }
        }
      }
    }

    Ok(())
  }
}
