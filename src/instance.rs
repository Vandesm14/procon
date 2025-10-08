use std::{fs, path::PathBuf};

use ignore::Walk;

use crate::config::{Config, ConfigHead, Configs};

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

  pub fn services_path(&self) -> PathBuf {
    dirs::config_dir().unwrap().join("systemd/user")
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
      let project_toml: Config =
        toml::from_str(&fs::read_to_string(entry.path()).unwrap())
          .unwrap_or_else(|e| {
            panic!("Failed to parse {}: {e}", entry.path().display())
          });
      self.configs.add(
        ConfigHead::new(entry.path().to_path_buf(), entry.depth()),
        project_toml,
      );
    }

    Ok(())
  }
}
