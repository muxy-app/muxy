use crate::{AppError, AppState, PaneId, Project, ProjectId, ProjectStatus, ServerId};
use muxy_protocol::{
    CatalogPage, OperationId, ProjectDescriptor, ProjectIntent, ProjectMutation, ProjectPatch,
    ServerIdentity, ServerPath,
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
    pub fn pending_cancellations(&self, server: ServerId) -> &[OperationId] {
        self.servers
            .get(&server)
            .map_or(&[], |state| &state.pending_cancellations)
    }
    pub fn complete_cancellation(&mut self, server: ServerId, operation: OperationId) {
        if let Some(state) = self.servers.get_mut(&server) {
            state
                .pending_cancellations
                .retain(|pending| *pending != operation);
        }
    }
    pub(crate) fn cancel_pending_creation(&mut self, pane: PaneId) {
        self.startup_commands.remove(&pane);
        if self.starting_directories.remove(&pane).is_some()
            && self.pane_mut(pane).is_ok_and(|pane| {
                matches!(pane.content, crate::PaneContent::Terminal { session: None })
            })
            && let Some(server) = self.pane_server(pane)
        {
            let cancellations = &mut self
                .servers
                .entry(server)
                .or_default()
                .pending_cancellations;
            if !cancellations.contains(&pane.creation_token()) {
                cancellations.push(pane.creation_token());
            }
        }
    }

    pub fn project_intents(&self, server: ServerId) -> &[ProjectIntent] {
        self.servers
            .get(&server)
            .map_or(&[], |state| &state.project_intents)
    }
    /// Whether `project` still waits for its server to create it, even if it
    /// was removed in the meantime.
    pub fn project_creation_pending(&self, project: ProjectId) -> bool {
        self.servers
            .values()
            .flat_map(|state| &state.project_intents)
            .any(|intent| {
                matches!(&intent.mutation, ProjectMutation::Create(record) if record.id == project)
            })
    }
    pub fn catalog_revision(&self, server: ServerId) -> u64 {
        self.servers
            .get(&server)
            .map_or(0, |state| state.catalog_revision)
    }

    /// The server that answered for `server` last time, once a catalog was applied.
    pub fn server_identity(&self, server: ServerId) -> Option<ServerIdentity> {
        self.servers.get(&server).and_then(|state| state.identity)
    }

    /// How many more project edits `server` can hold before it confirms some.
    pub fn project_intent_capacity(&self, server: ServerId) -> usize {
        1024_usize.saturating_sub(self.project_intents(server).len())
    }

    pub(crate) fn queue_project(
        &mut self,
        server: ServerId,
        mutation: ProjectMutation,
    ) -> Result<(), AppError> {
        mutation
            .validate()
            .map_err(|_| AppError::InvalidState("invalid project metadata".into()))?;
        if self.project_intent_capacity(server) == 0 {
            return Err(AppError::InvalidState(
                "pending project edits are full; reconnect before editing more projects".into(),
            ));
        }
        let intents = &mut self.servers.entry(server).or_default().project_intents;
        intents.push(ProjectIntent {
            operation: OperationId::new(),
            mutation,
        });
        Ok(())
    }

    pub fn complete_project_intent(
        &mut self,
        server: ServerId,
        operation: OperationId,
    ) -> Result<(), AppError> {
        match self.servers.get_mut(&server) {
            Some(state)
                if state
                    .project_intents
                    .first()
                    .is_some_and(|intent| intent.operation == operation) =>
            {
                state.project_intents.remove(0);
                Ok(())
            }
            _ => Err(AppError::InvalidState(
                "project acknowledgement is out of order".into(),
            )),
        }
    }

    pub fn prune_worktree(&mut self, id: ProjectId) -> Result<(), AppError> {
        let Some(project) = self.project(id) else {
            return Ok(());
        };
        if project.kind != Some(crate::ProjectKind::Worktree)
            || project.parent_id.is_none()
            || !project.tabs.is_empty()
            || self
                .project_intents(project.server_id)
                .iter()
                .any(|intent| match &intent.mutation {
                    ProjectMutation::Create(project) => project.id == id,
                    ProjectMutation::Delete(project) | ProjectMutation::PruneWorktree(project) => {
                        *project == id
                    }
                    ProjectMutation::Patch { .. } => false,
                })
        {
            return Ok(());
        }
        self.remove_project_with_mutation(id, ProjectMutation::PruneWorktree(id))
            .map(|_| ())
    }

    pub fn prepare_creation(&mut self, pane: PaneId, directory: &Path) -> ServerPath {
        self.starting_directories
            .entry(pane)
            .or_insert_with(|| ServerPath(directory.as_os_str().as_bytes().into()))
            .clone()
    }

    /// Replaces `server`'s projects with its catalog, overlaid with its
    /// pending edits. Other servers' projects are untouched.
    pub fn apply_catalog(&mut self, server: ServerId, page: &CatalogPage) -> Result<(), AppError> {
        if page.next.is_some() {
            return Err(AppError::InvalidState("incomplete project catalog".into()));
        }
        if self
            .servers
            .get(&server)
            .and_then(|state| state.identity)
            .is_some_and(|identity| identity != page.server)
        {
            return Err(AppError::ServerChanged(server));
        }
        if let Some(existing) = self.servers.iter().find_map(|(id, state)| {
            (*id != server && state.identity == Some(page.server)).then_some(*id)
        }) {
            return Err(AppError::DuplicateServer { server, existing });
        }
        let mut next = self.clone();
        next.merge_catalog(server, page)?;
        next.validate()?;
        let state = next.servers.entry(server).or_default();
        state.identity = Some(page.server);
        state.catalog_revision = page.revision;
        *self = next;
        Ok(())
    }

    fn merge_catalog(&mut self, server: ServerId, page: &CatalogPage) -> Result<(), AppError> {
        self.remap_home(server, page.home);
        let mut descriptors: BTreeMap<_, _> = page
            .projects
            .iter()
            .map(|project| (project.id, project.clone()))
            .collect();
        for intent in self.project_intents(server) {
            match &intent.mutation {
                ProjectMutation::Create(project) => {
                    descriptors.insert(project.id, project.clone());
                }
                ProjectMutation::Patch { project, patch } => {
                    if let Some(project) = descriptors.get_mut(project) {
                        patch.apply(project);
                    }
                }
                ProjectMutation::Delete(project) | ProjectMutation::PruneWorktree(project) => {
                    descriptors.retain(|id, descriptor| {
                        id != project && descriptor.parent_id != Some(*project)
                    });
                }
            }
        }
        self.projects
            .retain(|project| project.server_id != server || descriptors.contains_key(&project.id));
        for project in &mut self.projects {
            if project.server_id == server
                && let Some(descriptor) = descriptors.remove(&project.id)
            {
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
                server_id: server,
                directory: PathBuf::new(),
                kind: None,
                parent_id: None,
                tabs: Vec::new(),
                groups: None,
                status: ProjectStatus::Available,
            };
            project.with_descriptor(&descriptor)?;
            self.projects.push(project);
        }
        let home = self
            .projects
            .iter()
            .position(|project| project.id == page.home && project.server_id == server)
            .ok_or_else(|| AppError::InvalidState("catalog has no Home".into()))?;
        self.place_home(home);
        self.retain_workspace_members();
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
        self.startup_commands.retain(|id, _| panes.contains(id));
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

    fn remap_home(&mut self, server: ServerId, home: ProjectId) {
        let Some(old_home) = self.server_home(server).map(|project| project.id) else {
            return;
        };
        if old_home != home {
            if let Some(project) = self
                .projects
                .iter_mut()
                .find(|project| project.id == old_home)
            {
                project.id = home;
            }
            if self.window.current_project == old_home {
                self.window.current_project = home;
            }
            if let Some(tab) = self.window.selected_tab.remove(&old_home) {
                self.window.selected_tab.insert(home, tab);
            }
            for intent in self
                .servers
                .get_mut(&server)
                .into_iter()
                .flat_map(|state| &mut state.project_intents)
            {
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
        let server = self
            .project_server(project)
            .ok_or(AppError::UnknownProject(project))?;
        self.queue_project(server, ProjectMutation::Patch { project, patch })
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
