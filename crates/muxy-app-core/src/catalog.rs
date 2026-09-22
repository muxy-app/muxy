use crate::{AppError, AppState, PaneId, Project, ProjectId, ProjectStatus, ServerId};
use muxy_protocol::{
    CatalogPage, OperationId, ProjectDescriptor, ProjectIntent, ProjectMutation, ProjectPatch,
    ServerPath,
};
use std::collections::{BTreeMap, HashSet};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

impl Project {
    pub fn descriptor(&self) -> ProjectDescriptor {
        ProjectDescriptor {
            id: self.id,
            home: self.home,
            name: self.name.clone(),
            icon: self.icon.clone(),
            logo: self.logo.clone(),
            color: self.color.to_string(),
            directory: ServerPath(self.directory.as_os_str().as_bytes().into()),
            kind: self.kind,
            parent_id: self.parent_id,
        }
    }
    fn with_descriptor(&mut self, descriptor: &ProjectDescriptor) -> Result<(), AppError> {
        self.id = descriptor.id;
        self.home = descriptor.home;
        self.name.clone_from(&descriptor.name);
        self.icon.clone_from(&descriptor.icon);
        self.logo.clone_from(&descriptor.logo);
        self.color = descriptor.color.parse()?;
        self.directory =
            PathBuf::from(std::ffi::OsString::from_vec(descriptor.directory.0.clone()));
        self.kind = descriptor.kind;
        self.parent_id = descriptor.parent_id;
        self.refresh_status();
        Ok(())
    }
}

impl AppState {
    pub fn pending_cancellations(&self) -> &[OperationId] {
        &self.pending_cancellations
    }
    pub fn complete_cancellation(&mut self, operation: OperationId) {
        self.pending_cancellations
            .retain(|pending| *pending != operation);
    }
    pub(crate) fn cancel_pending_creation(&mut self, pane: PaneId) {
        if self.starting_directories.remove(&pane).is_some()
            && !self.pending_cancellations.contains(&pane.creation_token())
        {
            self.pending_cancellations.push(pane.creation_token());
        }
    }

    pub fn project_intents(&self) -> &[ProjectIntent] {
        &self.project_intents
    }
    pub fn catalog_revision(&self) -> u64 {
        self.catalog_revision
    }

    pub(crate) fn queue_project(&mut self, mutation: ProjectMutation) -> Result<(), AppError> {
        mutation
            .validate()
            .map_err(|_| AppError::InvalidState("invalid project metadata".into()))?;
        if self.project_intents.len() >= 1024 {
            return Err(AppError::InvalidState(
                "pending project edits are full; reconnect before editing more projects".into(),
            ));
        }
        self.project_intents.push(ProjectIntent {
            operation: OperationId::new(),
            mutation,
        });
        Ok(())
    }

    pub fn complete_project_intent(&mut self, operation: OperationId) -> Result<(), AppError> {
        if self
            .project_intents
            .first()
            .is_some_and(|intent| intent.operation == operation)
        {
            self.project_intents.remove(0);
            Ok(())
        } else {
            Err(AppError::InvalidState(
                "project acknowledgement is out of order".into(),
            ))
        }
    }

    pub fn prepare_creation(&mut self, pane: PaneId, directory: &Path) -> ServerPath {
        self.starting_directories
            .entry(pane)
            .or_insert_with(|| ServerPath(directory.as_os_str().as_bytes().into()))
            .clone()
    }

    pub fn apply_catalog(&mut self, page: &CatalogPage) -> Result<(), AppError> {
        if page.next.is_some() {
            return Err(AppError::InvalidState("incomplete project catalog".into()));
        }
        if self
            .catalog_server
            .is_some_and(|server| server != page.server)
        {
            return Err(AppError::InvalidState(
                "server identity changed; original desktop state preserved".into(),
            ));
        }
        let mut next = self.clone();
        next.merge_catalog(page)?;
        next.validate()?;
        next.catalog_server = Some(page.server);
        next.catalog_revision = page.revision;
        next.version = 2;
        *self = next;
        Ok(())
    }

    fn merge_catalog(&mut self, page: &CatalogPage) -> Result<(), AppError> {
        self.remap_home(page.home);
        let mut descriptors: BTreeMap<_, _> = page
            .projects
            .iter()
            .map(|project| (project.id, project.clone()))
            .collect();
        for intent in &self.project_intents {
            match &intent.mutation {
                ProjectMutation::Create(project) => {
                    descriptors.insert(project.id, project.clone());
                }
                ProjectMutation::Patch { project, patch } => {
                    if let Some(project) = descriptors.get_mut(project) {
                        patch.apply(project);
                    }
                }
                ProjectMutation::Delete(project) => {
                    descriptors.retain(|id, descriptor| {
                        id != project && descriptor.parent_id != Some(*project)
                    });
                }
            }
        }
        self.projects
            .retain(|project| descriptors.contains_key(&project.id));
        for project in &mut self.projects {
            if let Some(descriptor) = descriptors.remove(&project.id) {
                project.with_descriptor(&descriptor)?;
            }
        }
        for descriptor in descriptors.into_values() {
            let mut project = Project {
                id: descriptor.id,
                home: descriptor.home,
                name: String::new(),
                icon: None,
                logo: None,
                color: crate::Color::default(),
                server_id: ServerId::local(),
                directory: PathBuf::new(),
                kind: None,
                parent_id: None,
                tabs: Vec::new(),
                status: ProjectStatus::Available,
            };
            project.with_descriptor(&descriptor)?;
            self.projects.push(project);
        }
        let home = self
            .projects
            .iter()
            .position(|project| project.id == page.home)
            .ok_or_else(|| AppError::InvalidState("catalog has no Home".into()))?;
        let home = self.projects.remove(home);
        self.projects.insert(0, home);
        let projects: HashSet<_> = self.projects.iter().map(|project| project.id).collect();
        self.window
            .selected_tab
            .retain(|id, _| projects.contains(id));
        let panes: HashSet<_> = self
            .projects
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .map(|pane| pane.id)
            .collect();
        self.window.focus_history.retain(|id| panes.contains(id));
        self.starting_directories.retain(|id, _| {
            panes.contains(id)
                || self
                    .quick_terminal
                    .as_ref()
                    .is_some_and(|pane| pane.id == *id)
        });
        if !projects.contains(&self.window.current_project) {
            self.window.current_project = page.home;
        }
        if self
            .window
            .active_pane
            .is_some_and(|id| !panes.contains(&id))
        {
            self.window.active_pane = None;
            self.focus_selected_tab();
        }
        Ok(())
    }

    fn remap_home(&mut self, home: ProjectId) {
        let old_home = self.home().id;
        if old_home != home {
            self.projects[0].id = home;
            if self.window.current_project == old_home {
                self.window.current_project = home;
            }
            if let Some(tab) = self.window.selected_tab.remove(&old_home) {
                self.window.selected_tab.insert(home, tab);
            }
            for intent in &mut self.project_intents {
                match &mut intent.mutation {
                    ProjectMutation::Patch { project, .. } | ProjectMutation::Delete(project)
                        if *project == old_home =>
                    {
                        *project = home;
                    }
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn patch_project(
        &mut self,
        project: ProjectId,
        patch: ProjectPatch,
    ) -> Result<(), AppError> {
        self.queue_project(ProjectMutation::Patch { project, patch })
    }
}

pub(crate) mod directory {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::ffi::OsString;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    pub(crate) fn serialize<S: Serializer>(
        directory: &Path,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        directory.as_os_str().as_bytes().serialize(serializer)
    }
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<PathBuf, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Directory {
            Text(String),
            Bytes(Vec<u8>),
        }
        Ok(match Directory::deserialize(deserializer)? {
            Directory::Text(text) => PathBuf::from(text),
            Directory::Bytes(bytes) => PathBuf::from(OsString::from_vec(bytes)),
        })
    }
}
