use std::{fs, path::PathBuf};

use ignore::Walk;
use internment::Intern;

use crate::config::Project;

#[derive(Debug, Clone, Default)]
pub struct Instance {
  projects: Vec<Project>,
  path: PathBuf,
}

impl Instance {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      projects: Vec::new(),
    }
  }

  pub fn try_init(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let mut instance = Instance::new(path.canonicalize()?);
    instance.read_dir()?;
    Ok(instance)
  }

  pub fn read_dir(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    for entry in Walk::new(&self.path)
      .filter_map(|e| e.ok())
      .filter(|f| f.file_name().to_str().unwrap() == "procon.yaml")
    {
      let project: Project =
        serde_norway::from_str(&fs::read_to_string(entry.path()).unwrap())
          .unwrap_or_else(|e| {
            panic!("Failed to parse {}: {e}", entry.path().display())
          });
      self.projects.push(project);
    }

    Ok(())
  }

  pub fn cmd_run(
    &self,
    phase_strings: Vec<Intern<String>>,
    dry_run: bool,
  ) -> Result<(), Box<dyn std::error::Error>> {
    for phase_string in phase_strings.into_iter() {
      for config in self.projects.iter() {
        if let Some(phase) = config.phases.get(&phase_string) {
          phase.run(config, dry_run);
        }
      }
    }

    Ok(())
  }
}
