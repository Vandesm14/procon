use std::{collections::HashMap, fs, path::PathBuf, process::Command};

use walkdir::WalkDir;

use crate::config::Config;

fn setup_nginx_configs(
  instance: &Instance,
) -> Result<(), Box<dyn std::error::Error>> {
  // Create blank nginx.conf in project root
  let nginx_conf_path = instance.path.join("nginx.conf");
  let nginx_conf_content = "# Global nginx configuration for procon projects\n# Add your nginx configuration here\n";
  fs::write(&nginx_conf_path, nginx_conf_content)?;

  // Create procon.conf in project directory first
  let temp_procon_conf_path = instance.path.join("procon.conf");
  let procon_conf_content = format!("include {};", nginx_conf_path.display());
  fs::write(&temp_procon_conf_path, procon_conf_content)?;

  println!(
    "Root access is required to create nginx configuration in /etc/nginx/conf.d/"
  );
  println!(
    "This will create procon.conf that includes the project's nginx.conf"
  );

  print!("Creating /etc/nginx/conf.d directory...");

  // Ensure /etc/nginx/conf.d directory exists
  let mkdir_result = Command::new("sudo")
    .arg("mkdir")
    .arg("-p")
    .arg("/etc/nginx/conf.d")
    .output();

  match mkdir_result {
    Err(e) => {
      return Err(
        format!("Could not create /etc/nginx/conf.d directory: {}", e).into(),
      );
    }
    Ok(output) => {
      if !output.status.success() {
        return Err(
          format!(
            "Failed to create /etc/nginx/conf.d directory: {}",
            String::from_utf8_lossy(&output.stderr)
          )
          .into(),
        );
      }
    }
  }

  println!(" done.");

  print!("Moving procon.conf to /etc/nginx/conf.d/");

  // Move procon.conf to /etc/nginx/conf.d/
  let mv_result = Command::new("sudo")
    .arg("mv")
    .arg(&temp_procon_conf_path)
    .arg("/etc/nginx/conf.d/procon.conf")
    .output();

  match mv_result {
    Err(e) => {
      return Err(format!(
        "Could not move procon.conf to /etc/nginx/conf.d/: {}. You may need to manually move {} to /etc/nginx/conf.d/procon.conf",
        e,
        temp_procon_conf_path.display()
      ).into());
    }
    Ok(output) => {
      if !output.status.success() {
        return Err(format!(
          "Failed to move procon.conf to /etc/nginx/conf.d/: {}. You may need to manually move {} to /etc/nginx/conf.d/procon.conf",
          String::from_utf8_lossy(&output.stderr),
          temp_procon_conf_path.display()
        ).into());
      }
    }
  }

  println!(" done.");

  Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct Instance {
  configs: HashMap<String, Config>,
  path: PathBuf,
}

impl Instance {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      configs: HashMap::new(),
    }
  }

  pub fn services_path(&self) -> PathBuf {
    dirs::config_dir().unwrap().join("systemd/user")
  }

  pub fn try_init(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let mut instance = Instance::new(path.canonicalize()?);
    setup_nginx_configs(&instance)?;
    instance.read_toml()?;
    Ok(instance)
  }

  pub fn read_toml(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    for entry in WalkDir::new(&self.path)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|f| f.file_name().to_str().unwrap() == "procon.toml")
    {
      let project_toml: Config =
        toml::from_str(&fs::read_to_string(entry.path()).unwrap()).unwrap();
      // self.configs
    }

    Ok(())
  }
}
