use std::{fs, path::PathBuf};

use crate::config::Config;

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
}
