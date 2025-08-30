use std::{
  collections::HashMap,
  fs::{self},
  path::{Path, PathBuf},
  process::{Command, ExitStatus},
};

use clap::{Parser, command};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

fn exit_status(status: ExitStatus, project_name: &str, cmd: &str) {
  if let Some(code) = status.code()
    && code != 0
  {
    panic!(
      "Project {project_name} failed to run command: {cmd} with exit code: {code}"
    );
  }
}

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
  projects: Vec<String>,
  #[arg(short, long)]
  path: Option<PathBuf>,
  #[arg(short, long)]
  exec: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Instance {
  path: PathBuf,

  projects: HashMap<String, Project>,
}

impl Instance {
  pub fn projects_path(path: &Path) -> PathBuf {
    path.join("projects")
  }

  pub fn state_path(path: &Path) -> PathBuf {
    path.join("state.ron")
  }

  pub fn from_path(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
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
      projects: HashMap::from_iter(
        projects.into_iter().map(|p| (p.name.clone(), p)),
      ),
    };

    println!("Loaded: {instance:#?}");

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
    for project in self.projects.values().filter(|p| match filter.is_empty() {
      true => true,
      false => filter.contains(&p.name),
    }) {
      project.source.prepare(project, &self.path)?;
      // for cmd in project.phase.build.to_vec() {
      //   exit_status(
      //     project.nix_shell(&self.path, &cmd).status()?,
      //     &project.name,
      //     &cmd,
      //   );
      // }
      // for cmd in project.phase.start.to_vec() {
      //   exit_status(
      //     project.nix_shell(&self.path, &cmd).status()?,
      //     &project.name,
      //     &cmd,
      //   );
      // }
      if let Some(content) =
        project.service.generate_service(project, &self.path)
      {
        fs::write(
          project.main_path(&self.path).join("daemon.service"),
          content,
        )?;
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
  #[serde(default)]
  service: Service,
}

impl Project {
  pub fn main_path(&self, path: &Path) -> PathBuf {
    path.join("artifacts").join(&self.name)
  }

  pub fn source_path(&self, path: &Path) -> PathBuf {
    self.main_path(path).join("source")
  }

  pub fn nix_shell(&self, path: &Path, cmd: &str) -> Command {
    let mut command = Command::new("nix-shell");
    command
      .current_dir(self.source_path(path))
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
  pub fn prepare(
    &self,
    project: &Project,
    path: &Path,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = project.source_path(path);
    fs::create_dir_all(&source_path)?;
    println!("Preparing: {} with {:?}", source_path.display(), self);
    match self {
      Source::None => return Ok(()),
      Source::Path(path_buf) => {
        fs::copy(path_buf, source_path)?;
      }
      Source::Git(url) => {
        exit_status(
          Command::new("git")
            .arg("clone")
            .arg(url)
            .arg(source_path)
            .status()?,
          &project.name,
          &format!("git clone {}", url),
        );
      }
      Source::Zip(path_buf) => {
        println!("unzip: {} {}", path_buf.display(), source_path.display());
        exit_status(
          Command::new("nix-shell")
            .arg("-p")
            .arg("unzip")
            .arg("--run")
            .arg(format!(
              "unzip -o {} -d {}",
              path_buf.display(),
              source_path.display()
            ))
            .status()?,
          &project.name,
          &format!(
            "unzip -o {} -d {}",
            path_buf.display(),
            source_path.display()
          ),
        );
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

  pub fn to_option(&self) -> Option<Vec<String>> {
    match self {
      Cmds::None => None,
      Cmds::Single(single) => Some(vec![single.clone()]),
      Cmds::Many(items) => Some(items.clone()),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(untagged)]
enum Service {
  #[default]
  None,
  File(PathBuf),
  Config(ServiceConfig),
}

impl Service {
  pub fn generate_service(
    &self,
    project: &Project,
    path: &Path,
  ) -> Option<String> {
    match self {
      Service::None => None,
      Service::File(path_buf) => fs::read_to_string(dbg!(path_buf)).ok(),
      Service::Config(service_config) => {
        service_config.generate_service(project, path)
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ServiceConfig {
  enable: bool,
  #[serde(default)]
  exec: Cmds,

  #[serde(default)]
  working_directory: Option<PathBuf>,
  #[serde(default)]
  dynamic_user: bool,
  #[serde(default)]
  restart_on: RestartOn,
}

impl ServiceConfig {
  pub fn generate_service(
    &self,
    project: &Project,
    path: &Path,
  ) -> Option<String> {
    println!("generating service for {}", project.name);
    if self.enable {
      let cmds = self
        .exec
        .to_option()
        .or(project.phase.start.to_option())
        .unwrap_or_default();
      let template = format!(
        r#"[Service]
  DynamicUser=yes
  WorkingDirectory={}
  ExecStart=/usr/bin/env bash -c \"{}\"
  "#,
        project.main_path(path).to_str().unwrap(),
        cmds.join("&&"),
      );
      Some(template)
    } else {
      None
    }
  }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RestartOn {
  Always,
  #[default]
  Never,
  OnFailure,
}

fn main() {
  let cli = Cli::parse();
  let path: PathBuf = cli.path.unwrap_or(".".into());
  let instance = Instance::from_path(path).unwrap();
  if cli.exec {
    instance.entrypoint(cli.projects).unwrap();
  }
  instance.write_state().unwrap();
}
