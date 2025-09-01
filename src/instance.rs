use std::{
  collections::{BTreeMap, HashMap},
  fs,
  path::PathBuf,
};

use serde::{Deserialize, Serialize};
use systemctl::SystemCtl;
use walkdir::WalkDir;

use crate::{
  action::{
    Action, ActionKind, ActionKindCommand, ActionKindFilesystem,
    ActionKindSystemCtl, Phase,
  },
  project::{Cmds, Project, ProjectStatus, ProjectToml},
};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Instance {
  projects: HashMap<String, Project>,
  #[serde(skip)]
  path: PathBuf,
}

impl Instance {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      projects: HashMap::new(),
    }
  }

  pub fn artifacts_path(&self) -> PathBuf {
    self.path.join("artifacts")
  }

  pub fn projects_path(&self) -> PathBuf {
    self.path.join("projects")
  }

  pub fn state_path(&self) -> PathBuf {
    self.path.join("state.ron")
  }

  pub fn services_path(&self) -> PathBuf {
    dirs::config_dir().unwrap().join("systemd/user")
  }

  pub fn try_init(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let mut instance = Instance::new(path.canonicalize()?);
    instance.read_toml()?;
    Ok(instance)
  }

  pub fn from_path(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let instance = Instance::new(path.clone());
    let mut instance = fs::read_to_string(instance.state_path())
      .ok()
      .and_then(|s| ron::from_str::<Instance>(&s).ok())
      .unwrap_or_else(|| {
        panic!(
          "Instance file (state.ron) not found in {}. Run `procon init` to initialize.",
          path.canonicalize().unwrap().display()
        )
      });
    instance.path = path.canonicalize()?;
    instance.read_toml()?;

    Ok(instance)
  }

  pub fn read_toml(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let mut live_projects: Vec<Project> = Vec::new();

    for file in WalkDir::new(self.projects_path())
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|f| f.file_name().to_str().unwrap().ends_with(".toml"))
    {
      let project_toml: ProjectToml =
        toml::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();

      let live_project = Project::from_project_toml(project_toml);
      live_projects.push(live_project);
    }

    for live_project in live_projects.iter() {
      if let Some(project) = self.projects.get_mut(&live_project.name) {
        if !project.non_status_equal(live_project)
          && matches!(
            project.status,
            ProjectStatus::Success | ProjectStatus::Failed(..)
          )
        {
          let mut live_project = live_project.clone();
          live_project.status = ProjectStatus::Changed;
          self
            .projects
            .insert(live_project.name.clone(), live_project);
        }
      } else {
        self
          .projects
          .insert(live_project.name.clone(), live_project.clone());
      }
    }

    for (name, project) in self.projects.iter_mut() {
      if !live_projects.iter().any(|lp| lp.name == *name) {
        project.status = ProjectStatus::Remove;
      }
    }

    Ok(())
  }

  pub fn write_state(&self) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(
      self.state_path(),
      ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())?,
    )?;
    Ok(())
  }

  pub fn project_phase_list(
    &self,
    project_filter: Option<&[String]>,
  ) -> HashMap<String, Vec<Phase>> {
    HashMap::from_iter(
      self
        .projects
        .iter()
        .filter(|(p, _)| project_filter.map(|f| f.contains(p)).unwrap_or(true))
        .map(|(name, project)| (name.clone(), project.status.to_phases())),
    )
  }

  pub fn make_actions(&self, project_filter: Option<&[String]>) -> Vec<Action> {
    let phase_list = self.project_phase_list(project_filter);
    let mut actions: Vec<Action> = Vec::new();

    for (name, phases) in phase_list.iter() {
      if let Some(project) = self.projects.get(name) {
        for phase in phases.iter() {
          match phase {
            Phase::Setup => {
              actions.extend(project.source.setup(project, &self.path));
              if let Some(service) =
                project.service.generate_service_string(project, &self.path)
              {
                actions.push(Action::new(
                  &project.name,
                  *phase,
                  ActionKind::Filesystem(ActionKindFilesystem::Write(
                    project.service_path(&self.path),
                    service,
                  )),
                ));
                actions.push(Action::new(
                  &project.name,
                  *phase,
                  ActionKind::Filesystem(ActionKindFilesystem::Copy(
                    project.service_path(&self.path),
                    self
                      .services_path()
                      .join(format!("procon-proj-{}.service", project.name)),
                  )),
                ));
              }
              for cmd in project.phase.setup.to_vec() {
                actions.push(Action::new(
                  &project.name,
                  *phase,
                  ActionKind::Command(ActionKindCommand::NixShell(
                    project.source_path(&self.path),
                    project.deps_nix(),
                    Cmds::Single(cmd),
                  )),
                ));
              }
            }
            Phase::Update => {
              // TODO: Update source.
              for cmd in project.phase.setup.to_vec() {
                actions.push(Action::new(
                  &project.name,
                  *phase,
                  ActionKind::Command(ActionKindCommand::NixShell(
                    project.source_path(&self.path),
                    project.deps_nix(),
                    Cmds::Single(cmd),
                  )),
                ));
              }
            }
            Phase::Build => {
              for cmd in project.phase.build.to_vec() {
                actions.push(Action::new(
                  &project.name,
                  *phase,
                  ActionKind::Command(ActionKindCommand::NixShell(
                    project.source_path(&self.path),
                    project.deps_nix(),
                    Cmds::Single(cmd),
                  )),
                ));
              }
            }
            Phase::Start => {
              // TODO: This shouldn't be restart, but this is required for the step.
              if project.service.autostart {
                actions.push(Action::new(
                  &project.name,
                  *phase,
                  ActionKind::SystemCtl(ActionKindSystemCtl::Restart(format!(
                    "procon-proj-{}.service",
                    project.name
                  ))),
                ));
              }
            }
            Phase::Stop => {
              actions.push(Action::new(
                &project.name,
                *phase,
                ActionKind::SystemCtl(ActionKindSystemCtl::Stop(format!(
                  "procon-proj-{}.service",
                  project.name
                ))),
              ));
            }
            Phase::Teardown => {
              // TODO: Teardown.
            }
          }
        }
      }
    }

    actions
  }

  pub fn apply(
    &mut self,
    project_filter: Option<&[String]>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    // Init services path.
    fs::create_dir_all(self.services_path()).unwrap();
    for (_, project) in self
      .projects
      .iter()
      .filter(|(p, _)| project_filter.map(|f| f.contains(p)).unwrap_or(true))
    {
      fs::create_dir_all(project.artifact_path(&self.path)).unwrap();
    }

    let mut skip: HashMap<String, Phase> = HashMap::new();
    let mut action_phases: BTreeMap<Phase, Vec<Action>> =
      self.make_actions(project_filter).into_iter().fold(
        BTreeMap::new(),
        |mut acc: BTreeMap<Phase, Vec<Action>>, action| {
          acc.entry(action.phase).or_default().push(action);
          acc
        },
      );

    for (phase, actions) in action_phases.iter_mut() {
      println!("Phase: {:?}.", phase);

      if *phase == Phase::Start {
        println!(" - Sub-Phase: Daemon Reload.");
        let systemctl = SystemCtl::builder()
          .additional_args(vec!["--user".to_string()])
          .build();
        systemctl
          .daemon_reload()
          .map_err(|e| format!("Failed to reload systemd daemon: {e:?}."))?;
      }

      for action in actions.iter_mut() {
        if skip.contains_key(&action.project_name) {
          action.mark_cancelled();
        } else {
          match &action.kind {
            ActionKind::Command(action_kind_command) => {
              let result = action_kind_command.apply(true);
              match result {
                Ok(output) => {
                  if output.status.success() {
                    action.mark_done()
                  } else {
                    let err = String::from_utf8(output.stderr);
                    action.mark_failed(format!("{err:?}"));
                    skip.insert(action.project_name.clone(), *phase);
                  }
                }
                Err(err) => {
                  action.mark_failed(format!("{err:?}"));
                  skip.insert(action.project_name.clone(), *phase);
                }
              }
            }
            ActionKind::Filesystem(action_kind_filesystem) => {
              let error = action_kind_filesystem.apply();
              if let Some(error) = error {
                action.mark_failed(format!("{error:?}"));
                skip.insert(action.project_name.clone(), *phase);
              } else {
                action.mark_done();
              }
            }
            ActionKind::SystemCtl(action_kind_system_ctl) => {
              let result = action_kind_system_ctl.apply();
              match result {
                Ok(output) => {
                  if output.success() {
                    action.mark_done();
                  } else {
                    action.mark_failed("Systemctl failed.".to_owned());
                    skip.insert(action.project_name.clone(), *phase);
                  }
                }
                Err(err) => {
                  action.mark_failed(format!("{err:?}"));
                  skip.insert(action.project_name.clone(), *phase);
                }
              }
            }
          }
        }

        println!(" - {}", action);
      }
    }

    for project in self.projects.values_mut() {
      if let Some(phase) = skip.get(&project.name) {
        project.status = ProjectStatus::Failed(*phase);
      } else {
        project.status = ProjectStatus::Success;
      }
    }

    self.write_state()?;

    Ok(())
  }

  pub fn cmd_plan(
    &self,
    project_filter: Option<&[String]>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let actions = self.make_actions(project_filter);
    self.write_state()?;
    println!("Status:");
    for (name, project) in self.projects.iter() {
      println!(" - {}: {:?}", name, project.status);
    }
    println!("Plan:");
    for action in actions.iter() {
      println!(" - {:?}", action);
    }
    Ok(())
  }

  pub fn cmd_apply(
    &mut self,
    project_filter: Option<&[String]>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    self.apply(project_filter)?;
    Ok(())
  }

  pub fn cmd_clean(
    &self,
    project_filter: Option<&[String]>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    for project_name in self
      .projects
      .keys()
      .filter(|p| project_filter.map(|f| f.contains(p)).unwrap_or(true))
    {
      let project_artifact_path = self.artifacts_path().join(project_name);
      if fs::exists(&project_artifact_path).is_ok_and(|b| b) {
        fs::remove_dir_all(&project_artifact_path)?;
      }
    }

    Ok(())
  }

  pub fn cmd_run_proxy(
    &self,
    project_name: String,
  ) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(project) = self.projects.get(&project_name) {
      println!("Running {}", project.name);
      let command = ActionKindCommand::NixShell(
        project.source_path(&self.path),
        project.deps_nix(),
        project.phase.start.clone(),
      );
      match command.apply(false) {
        Ok(ok) => {
          if let Some(code) = ok.status.code() {
            println!("Exited with code: {}.", code);
          } else {
            println!("Process terminated by signal.");
          }
        }
        Err(err) => {
          println!("Action: {:?}", command);
          panic!("Execution error: {:?}", err);
        }
      }
    }
    Ok(())
  }
}
