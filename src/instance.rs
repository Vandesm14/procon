use std::{
  collections::{BTreeMap, HashMap, HashSet},
  fs,
  path::PathBuf,
};

use serde::{Deserialize, Serialize};
use systemctl::SystemCtl;
use walkdir::WalkDir;

use crate::{
  IS_SAFE_MODE,
  action::{
    Action, ActionKind, ActionKindCommand, ActionKindFilesystem,
    ActionKindSystemCtl, ConfigChange, Phase,
  },
  project::{Cmds, Project, ProjectToml},
};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Instance {
  path: PathBuf,
  projects: HashMap<String, Project>,
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

  pub fn from_path(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let path = path.canonicalize().unwrap();

    let mut projects: Vec<Project> = Vec::new();
    let mut instance = Self::new(path);

    for file in WalkDir::new(instance.projects_path())
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|f| f.file_name().to_str().unwrap().ends_with(".toml"))
    {
      let project_toml: ProjectToml =
        toml::from_str(&fs::read_to_string(file.path()).unwrap()).unwrap();

      let project = Project::from_project_toml(
        project_toml,
        file.path().parent().unwrap().canonicalize()?,
      );
      projects.push(project);
    }

    instance.projects =
      HashMap::from_iter(projects.into_iter().map(|p| (p.name.clone(), p)));

    Ok(instance)
  }

  pub fn write_state(&self) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(self.state_path(), ron::to_string(self)?)?;
    Ok(())
  }

  pub fn compare(
    &self,
    project_filter: Option<&[String]>,
  ) -> HashMap<String, ConfigChange> {
    let mut changes: HashMap<String, ConfigChange> = HashMap::new();
    let old_state = fs::read_to_string(self.state_path())
      .ok()
      .and_then(|s| ron::from_str::<Instance>(&s).ok())
      .unwrap_or_default();
    for project in self
      .projects
      .values()
      .filter(|p| project_filter.map(|f| f.contains(&p.name)).unwrap_or(true))
    {
      if let Some(old_project) = old_state.projects.get(&project.name) {
        if old_project != project {
          changes.insert(project.name.clone(), ConfigChange::Changed);
        }
      } else {
        changes.insert(project.name.clone(), ConfigChange::Added);
      }
    }

    for project in old_state
      .projects
      .values()
      .filter(|p| project_filter.map(|f| f.contains(&p.name)).unwrap_or(true))
    {
      if !self.projects.contains_key(&project.name) {
        changes.insert(project.name.clone(), ConfigChange::Removed);
      }
    }

    changes
  }

  pub fn plan(
    &self,
    project_filter: Option<&[String]>,
  ) -> BTreeMap<String, Vec<Phase>> {
    BTreeMap::from_iter(
      self
        .compare(project_filter)
        .into_iter()
        .map(|(name, change)| (name, change.to_phases())),
    )
  }

  pub fn make_actions(&self, project_filter: Option<&[String]>) -> Vec<Action> {
    let plan = self.plan(project_filter);
    let mut actions: Vec<Action> = Vec::new();

    for (name, phases) in plan.iter() {
      if let Some(project) = self.projects.get(name) {
        for phase in phases.iter() {
          match phase {
            Phase::Setup => {
              actions.extend(project.source.setup(project, &self.path));
              if let Some(service) =
                project.service.generate_service_string(project, &self.path)
                && !*IS_SAFE_MODE
              {
                actions.push(Action::new(
                  &project.name,
                  Phase::Setup,
                  ActionKind::Filesystem(ActionKindFilesystem::Write(
                    project.service_path(&self.path),
                    service,
                  )),
                ));
                actions.push(Action::new(
                  &project.name,
                  Phase::Setup,
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
                  Phase::Setup,
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
                  Phase::Update,
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
                  Phase::Build,
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
              if !*IS_SAFE_MODE && project.service.autostart {
                actions.push(Action::new(
                  &project.name,
                  Phase::Start,
                  ActionKind::SystemCtl(ActionKindSystemCtl::Restart(format!(
                    "procon-proj-{}.service",
                    project.name
                  ))),
                ));
              }
            }
            Phase::Stop => {
              if !*IS_SAFE_MODE {
                actions.push(Action::new(
                  &project.name,
                  Phase::Stop,
                  ActionKind::SystemCtl(ActionKindSystemCtl::Stop(format!(
                    "procon-proj-{}.service",
                    project.name
                  ))),
                ));
              }
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
    &self,
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

    let mut skip: HashSet<String> = HashSet::new();
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
        println!("Sub-Phase: Daemon Reload.");
        let systemctl = SystemCtl::builder()
          .additional_args(vec!["--user".to_string()])
          .build();
        systemctl
          .daemon_reload()
          .map_err(|e| format!("Failed to reload systemd daemon: {e:?}."))?;
      }

      for action in actions.iter_mut() {
        if skip.contains(&action.project_name) {
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
                    skip.insert(action.project_name.clone());
                  }
                }
                Err(err) => {
                  action.mark_failed(format!("{err:?}"));
                  skip.insert(action.project_name.clone());
                }
              }
            }
            ActionKind::Filesystem(action_kind_filesystem) => {
              let error = action_kind_filesystem.apply();
              if let Some(error) = error {
                action.mark_failed(format!("{error:?}"));
                skip.insert(action.project_name.clone());
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
                    skip.insert(action.project_name.clone());
                  }
                }
                Err(err) => {
                  action.mark_failed(format!("{err:?}"));
                  skip.insert(action.project_name.clone());
                }
              }
            }
          }
        }

        println!("{}", action);
      }
    }

    self.write_state()?;

    Ok(())
  }

  pub fn cmd_plan(
    &self,
    project_filter: Option<&[String]>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let plan = self.make_actions(project_filter);
    println!("plan: {plan:#?}");
    Ok(())
  }

  pub fn cmd_apply(
    &self,
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
