use std::{fs, path::PathBuf};

use ignore::Walk;
use internment::Intern;

use crate::config::{Config, ConfigToml, Configs};

#[derive(Debug, Clone, Default)]
pub struct Instance {
  configs: Configs,
  path: PathBuf,
}

impl Instance {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      configs: Configs::new(),
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
      .filter(|f| f.file_name().to_str().unwrap() == "procon.toml")
    {
      let project_toml: ConfigToml =
        toml::from_str(&fs::read_to_string(entry.path()).unwrap())
          .unwrap_or_else(|e| {
            panic!("Failed to parse {}: {e}", entry.path().display())
          });
      self.configs.push(Config::from_config_toml(
        entry.path().parent().unwrap().to_path_buf(),
        project_toml,
      ));
    }

    Ok(())
  }

  pub fn cmd_run(
    &self,
    phase_strings: Vec<Intern<String>>,
    dry_run: bool,
  ) -> Result<(), Box<dyn std::error::Error>> {
    for phase_string in phase_strings.into_iter() {
      for config in self.configs.iter() {
        if let Some(phase) = config.phases.get(&phase_string) {
          phase.run(config, dry_run);
        }
      }
    }

    Ok(())
  }
}
