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
    cmds: Vec<Intern<String>>,
    dry_run: bool,
  ) -> Result<(), Box<dyn std::error::Error>> {
    for cmd in cmds.into_iter() {
      for config in self.configs.iter() {
        if let Some(cmds) = config.phases.get(&cmd) {
          let mut command = cmds.run(
            &config.path,
            config.deps.get(&Intern::from_ref("nix")).map(|d| d.iter()),
          );
          if dry_run {
            println!("would run: {command:?}");
          } else {
            println!("$ {command:?}");
            match command.output() {
              Ok(output) => {
                if output.status.success() {
                  println!("{}", String::from_utf8_lossy(&output.stdout));
                } else {
                  println!("{}", String::from_utf8_lossy(&output.stderr));
                }
              }
              Err(e) => {
                println!("error: {e}");
              }
            }
          }
        }
      }
    }

    Ok(())
  }
}
