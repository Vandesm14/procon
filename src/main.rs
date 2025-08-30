use std::{
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use clap::{Parser, command};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
  projects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Instance {
  path: PathBuf,

  modules: HashMap<String, Project>,
  projects: HashMap<String, Project>,
}

impl Instance {
  pub fn modules_path(path: &Path) -> PathBuf {
    path.join("modules")
  }

  pub fn projects_path(path: &Path) -> PathBuf {
    path.join("projects")
  }

  pub fn state_path(path: &Path) -> PathBuf {
    path.join("state.ron")
  }

  pub fn from_path(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let mut modules: Vec<Project> = Vec::new();
    for file in WalkDir::new(Self::modules_path(&path))
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|f| f.file_name().to_str().unwrap().ends_with(".toml"))
    {
      let module: Project =
        toml::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();
      modules.push(module);
    }

    let mut projects: Vec<Project> = Vec::new();
    for file in WalkDir::new(Self::projects_path(&path))
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|f| f.file_name().to_str().unwrap().ends_with(".toml"))
    {
      let project: Project =
        toml::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();
      projects.push(project);
    }

    let instance = Self {
      path,
      modules: HashMap::from_iter(
        modules.into_iter().map(|m| (m.name.clone(), m)),
      ),
      projects: HashMap::from_iter(
        projects.into_iter().map(|p| (p.name.clone(), p)),
      ),
    };

    println!("Loaded: {instance:?}");

    Ok(instance)
  }

  pub fn write_state(&self) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(Self::state_path(&self.path), ron::to_string(self)?)?;
    Ok(())
  }

  pub fn entrypoint(
    &self,
    filter: Vec<String>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    for project in self.projects.values().filter(|p| {
      if filter.is_empty() {
        true
      } else {
        filter.contains(&p.name)
      }
    }) {
      project.source.prepare(&self.path, &project.name)?;
      for cmd in project.phase.build.to_vec() {
        project.nix_shell(&self.path, cmd).status()?;
      }
    }

    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Project {
  name: String,
  #[serde(default)]
  source: Source,
  #[serde(default)]
  deps: HashMap<String, Vec<String>>,
  #[serde(default)]
  phase: Phases,
  #[serde(default)]
  env: HashMap<String, String>,
}

impl Project {
  pub fn nix_shell(&self, path: &Path, cmd: String) -> Command {
    let mut command = Command::new("nix-shell");
    command
      .current_dir(self.source.artifact_path(path, &self.name))
      .arg("-p")
      .args(self.deps.get("nix").unwrap_or(&Vec::new()))
      .arg("--run")
      .arg(cmd);

    command
  }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Source {
  #[default]
  None,
  Path(PathBuf),
  Git(String),
  Zip(PathBuf),
}

impl Source {
  pub fn artifact_path(&self, path: &Path, project_name: &str) -> PathBuf {
    path.join("artifacts").join(project_name).join("source")
  }

  pub fn exists(
    &self,
    path: &Path,
    project_name: &str,
  ) -> Result<bool, std::io::Error> {
    fs::exists(self.artifact_path(path, project_name))
  }

  pub fn mkdir(
    &self,
    path: &Path,
    project_name: &str,
  ) -> Result<(), std::io::Error> {
    fs::create_dir_all(self.artifact_path(path, project_name))
  }

  pub fn prepare(
    &self,
    path: &Path,
    project_name: &str,
  ) -> Result<(), Box<dyn std::error::Error>> {
    self.mkdir(path, project_name)?;
    let artifact_path = self.artifact_path(path, project_name);
    println!("Preparing: {} with {:?}", artifact_path.display(), self);
    match self {
      Source::None => return Ok(()),
      Source::Path(path_buf) => {
        fs::copy(path_buf, artifact_path)?;
      }
      Source::Git(url) => {
        Command::new("git")
          .arg("clone")
          .arg(url)
          .arg(artifact_path)
          .status()?;
      }
      Source::Zip(path_buf) => {
        println!("unzip: {} {}", path_buf.display(), artifact_path.display());
        Command::new("nix-shell")
          .arg("-p")
          .arg("unzip")
          .arg("--run")
          .arg(format!(
            "unzip -o {} -d {}",
            path_buf.display(),
            artifact_path.display()
          ))
          .status()?;
      }
    }

    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Phases {
  /// Runs once, before source and deps are installed.
  #[serde(default)]
  setup: Cmds,
  /// Runs once, after the source and deps are installed.
  #[serde(default)]
  install: Cmds,
  /// Runs on an update trigger.
  #[serde(default)]
  update: Cmds,
  /// Runs after an update.
  #[serde(default)]
  build: Cmds,
  /// Starts the project.
  #[serde(default)]
  start: Cmds,
  /// Stops the project.
  #[serde(default)]
  stop: Cmds,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(untagged)]
enum Cmds {
  #[default]
  None,
  Single(String),
  Many(Vec<String>),
}

impl Cmds {
  pub fn to_vec(&self) -> Vec<String> {
    match self {
      Cmds::None => Vec::new(),
      Cmds::Single(single) => vec![single.clone()],
      Cmds::Many(items) => items.clone(),
    }
  }
}

fn main() {
  let cli = Cli::parse();
  let path: PathBuf = ".".into();
  let instance = Instance::from_path(path).unwrap();
  instance.entrypoint(cli.projects).unwrap();
  instance.write_state().unwrap();
}
