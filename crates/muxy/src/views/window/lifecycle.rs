use super::*;
use crate::project_operations::{
    BeginOperationError, ProbeToken, ProjectOperationKind, ProjectOperationToken,
};
use muxy_api::truth::{ProjectProbe, ProjectTruth};

const P8_STAGED_PHASE_FOUR_ENV: &str = "MUXY_TEST_P8_PHASE4_CASE";
const P8_STAGED_PHASE_FOUR_STATUS: &str = ".muxy-p8-phase4-status.json";
const P8_STAGED_PHASE_FOUR_PROCEED: &str = ".muxy-p8-phase4-proceed";

struct StagedPhaseFourPaths {
    status: PathBuf,
    proceed: PathBuf,
    owned_processes: PathBuf,
}

fn staged_phase_four_paths() -> Option<(String, StagedPhaseFourPaths)> {
    if !muxy_core::prefs::is_test_process() {
        return None;
    }
    let case_name = std::env::var(P8_STAGED_PHASE_FOUR_ENV).ok()?;
    if !matches!(case_name.as_str(), "background" | "reopen") {
        return None;
    }
    let app_support = muxy_core::prefs::app_support_dir();
    let injected = std::env::var_os("MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY").map(PathBuf::from);
    if injected.as_deref() != Some(app_support.as_path())
        || !app_support.is_absolute()
        || app_support.starts_with(muxy_core::prefs::home_dir())
        || path_has_symlink_or_missing_component(&app_support)
    {
        return None;
    }
    let root = app_support.parent()?.to_path_buf();
    if std::fs::read_to_string(root.join(".muxy-p8-owner"))
        .ok()?
        .trim()
        != "muxy-p8-terminal-memory-v1"
    {
        return None;
    }
    Some((
        case_name,
        StagedPhaseFourPaths {
            status: root.join(P8_STAGED_PHASE_FOUR_STATUS),
            proceed: root.join(P8_STAGED_PHASE_FOUR_PROCEED),
            owned_processes: root.join("owned-processes"),
        },
    ))
}

fn write_staged_phase_four_json(path: &Path, value: &serde_json::Value) -> std::io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(temporary, path)
}

fn write_staged_phase_four_processes(
    path: &Path,
    identities: &[muxy_proto::session::ProcessIdentity],
) -> std::io::Result<()> {
    if identities.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "staged session runtime has no process identities",
        ));
    }
    let mut contents = String::new();
    for (index, identity) in identities.iter().enumerate() {
        contents.push_str(&format!(
            "{} {} {}\n",
            identity.process_id,
            identity.start_identity,
            if index == 0 { "daemon" } else { "shell" }
        ));
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, contents)?;
    std::fs::rename(temporary, path)
}

pub(super) struct ProjectRuntime {
    pub(super) watchers: muxy_api::watcher::Watchers,
    pub(super) git_options: muxy_api::git::GitOptions,
    pub(super) execution_environment: muxy_api::execution_environment::ExecutionEnvironmentSource,
    pub(super) _watcher_task: Task<()>,
    pub(super) _environment_task: Task<()>,
}

impl ProjectRuntime {
    pub(super) fn new(
        watchers: muxy_api::watcher::Watchers,
        watcher_task: Task<()>,
        execution_environment: muxy_api::execution_environment::ExecutionEnvironmentSource,
        environment_task: Task<()>,
    ) -> Self {
        let git_options = crate::git::options(&execution_environment.snapshot());
        Self {
            watchers,
            git_options,
            execution_environment,
            _watcher_task: watcher_task,
            _environment_task: environment_task,
        }
    }
}

impl Drop for ProjectRuntime {
    fn drop(&mut self) {
        self.execution_environment.close();
    }
}

impl MainWindow {
    pub(super) fn prepare_staged_phase_four(&mut self, cx: &mut Context<Self>) {
        let Some((case_name, paths)) = staged_phase_four_paths() else {
            return;
        };
        if case_name == "background" {
            if let Err(error) = self.run_staged_phase_four_background(&paths) {
                let _ = write_staged_phase_four_json(
                    &paths.status,
                    &serde_json::json!({ "state": "error", "error": error }),
                );
            }
            return;
        }
        cx.spawn(async move |window, cx| {
            for _ in 0..600 {
                if paths.proceed.is_file() {
                    let _ = window.update(cx, |window, _| {
                        if let Err(error) = window.run_staged_phase_four_reopen(&paths) {
                            let _ = write_staged_phase_four_json(
                                &paths.status,
                                &serde_json::json!({ "state": "error", "error": error }),
                            );
                        }
                    });
                    return;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
            }
            let _ = write_staged_phase_four_json(
                &paths.status,
                &serde_json::json!({
                    "state": "error",
                    "error": "staged Phase 4 proceed request timed out"
                }),
            );
        })
        .detach();
    }

    fn run_staged_phase_four_background(
        &mut self,
        paths: &StagedPhaseFourPaths,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.managed_sessions(&self.state.tab_workspaces)?;
        sessions.retain(|session| {
            session.state == crate::sessions::ManagedSessionState::Workspace
                && session.placement.is_some()
                && session.shell.is_some()
                && self
                    .state
                    .workspace
                    .project(&session.owner.project_id)
                    .is_some_and(|project| !project.is_home())
        });
        sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        let first = sessions
            .first()
            .cloned()
            .ok_or_else(|| "staged app has no persistent workspace session".to_owned())?;
        let unrelated = sessions
            .iter()
            .find(|session| {
                !session
                    .owner
                    .project_id
                    .eq_ignore_ascii_case(&first.owner.project_id)
            })
            .cloned()
            .ok_or_else(|| "staged app has no unrelated persistent session".to_owned())?;
        let identities = self.sessions.runtime_process_identities()?;
        write_staged_phase_four_processes(&paths.owned_processes, &identities)
            .map_err(|error| error.to_string())?;
        let placement = first
            .placement
            .clone()
            .ok_or_else(|| "staged session has no workspace placement".to_owned())?;
        self.sessions
            .send_to_background(&placement.tab_id, first.session_id, &first.owner)?;
        let mut updated = self.state.tab_workspaces.clone();
        let removed = updated
            .states_mut()
            .iter_mut()
            .find(|workspace| workspace.tab(&placement.tab_id).is_some())
            .map(|workspace| {
                workspace.close_tab(&placement.tab_id, muxy_core::workspace::CloseMode::Single)
            })
            .unwrap_or_default();
        if removed != [placement.tab_id.clone()] {
            return Err("staged Background workspace candidate changed".to_owned());
        }
        updated.save().map_err(|error| error.to_string())?;
        self.state.tab_workspaces = updated;
        let managed = self.sessions.managed_sessions(&self.state.tab_workspaces)?;
        if !managed.iter().any(|session| {
            session.session_id == first.session_id
                && session.state == crate::sessions::ManagedSessionState::Background
        }) {
            return Err("staged app did not move the exact session to Background".to_owned());
        }
        write_staged_phase_four_json(
            &paths.status,
            &serde_json::json!({
                "state": "background",
                "sessionId": first.session_id,
                "unrelatedSessionId": unrelated.session_id,
                "owner": first.owner,
                "originalPlacement": placement,
            }),
        )
        .map_err(|error| error.to_string())
    }

    fn run_staged_phase_four_reopen(&mut self, paths: &StagedPhaseFourPaths) -> Result<(), String> {
        let status: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&paths.status).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if status.get("state").and_then(serde_json::Value::as_str) != Some("background") {
            return Err("staged Phase 4 status is not Background".to_owned());
        }
        let session_id: muxy_proto::session::SessionId = serde_json::from_value(
            status
                .get("sessionId")
                .cloned()
                .ok_or_else(|| "staged session ID is missing".to_owned())?,
        )
        .map_err(|error| error.to_string())?;
        let unrelated_session_id: muxy_proto::session::SessionId = serde_json::from_value(
            status
                .get("unrelatedSessionId")
                .cloned()
                .ok_or_else(|| "staged unrelated session ID is missing".to_owned())?,
        )
        .map_err(|error| error.to_string())?;
        let owner: muxy_proto::session::SessionOwner = serde_json::from_value(
            status
                .get("owner")
                .cloned()
                .ok_or_else(|| "staged session owner is missing".to_owned())?,
        )
        .map_err(|error| error.to_string())?;
        let original: muxy_proto::session::WorkspacePlacement = serde_json::from_value(
            status
                .get("originalPlacement")
                .cloned()
                .ok_or_else(|| "staged original placement is missing".to_owned())?,
        )
        .map_err(|error| error.to_string())?;
        let outcome = self
            .sessions
            .reattach(session_id, &mut self.state.tab_workspaces)?;
        let placement = match outcome {
            crate::sessions::SessionReattachOutcome::Focused(placement)
            | crate::sessions::SessionReattachOutcome::Reattached(placement) => placement,
        };
        if placement.project_id != original.project_id
            || placement.worktree_id != original.worktree_id
            || placement.tab_id != original.tab_id
        {
            return Err("staged reattach changed the original owner placement".to_owned());
        }
        let project = self
            .state
            .workspace
            .project(&owner.project_id)
            .cloned()
            .ok_or_else(|| "staged session project is missing".to_owned())?;
        let token = self
            .state
            .project_operations
            .begin_operation(&project.id, ProjectOperationKind::Remove)
            .map_err(|_| "staged project deletion operation is busy".to_owned())?;
        let cleanup_plan =
            self.sessions
                .owner_cleanup_plan(&crate::sessions::SessionOwnerScope {
                    project_id: owner.project_id.clone(),
                    worktree_id: None,
                })?;
        if !cleanup_plan
            .candidates
            .iter()
            .any(|candidate| candidate.session_id == session_id && candidate.owner == owner)
        {
            return Err(
                "staged exact-owner cleanup plan omitted the reattached session".to_owned(),
            );
        }
        let project_is_current = |window: &Self| {
            window
                .state
                .workspace
                .project(&project.id)
                .is_some_and(|current| {
                    current.path == project.path
                        && current.remote_workspace_id == project.remote_workspace_id
                })
                && window.state.project_operations.matches_operation(&token)
        };
        if !project_is_current(self) {
            return Err("staged project changed before exact-owner cleanup".to_owned());
        }
        self.sessions.end_owner_cleanup_plan(&cleanup_plan)?;
        if !project_is_current(self) {
            return Err("staged project changed during exact-owner cleanup".to_owned());
        }
        match self
            .state
            .remove_project_with_operation(&project.id, &token)
        {
            crate::state::RemoveProjectOutcome::Removed => {}
            crate::state::RemoveProjectOutcome::NotRemoved => {
                return Err("staged project was not removed after cleanup".to_owned());
            }
            crate::state::RemoveProjectOutcome::RemovedWithWorkspacePersistenceError(error) => {
                return Err(format!(
                    "staged project was removed but workspace persistence failed: {error}"
                ));
            }
        }
        let managed = self.sessions.managed_sessions(&self.state.tab_workspaces)?;
        if managed.iter().any(|session| {
            session.session_id == session_id
                && matches!(
                    session.state,
                    crate::sessions::ManagedSessionState::Workspace
                        | crate::sessions::ManagedSessionState::Background
                )
        }) {
            return Err("staged exact-owner cleanup left its session running".to_owned());
        }
        if !managed.iter().any(|session| {
            session.session_id == unrelated_session_id
                && matches!(
                    session.state,
                    crate::sessions::ManagedSessionState::Workspace
                        | crate::sessions::ManagedSessionState::Background
                )
        }) {
            return Err("staged exact-owner cleanup ended the unrelated session".to_owned());
        }
        self.sessions.end_all_sessions()?;
        let managed = self.sessions.managed_sessions(&self.state.tab_workspaces)?;
        if managed.iter().any(|session| {
            matches!(
                session.state,
                crate::sessions::ManagedSessionState::Workspace
                    | crate::sessions::ManagedSessionState::Background
            )
        }) {
            return Err("staged End All left a session running".to_owned());
        }
        write_staged_phase_four_json(
            &paths.status,
            &serde_json::json!({
                "state": "complete",
                "sessionId": session_id,
                "unrelatedSessionId": unrelated_session_id,
                "ownerCleanupCount": cleanup_plan.candidates.len(),
            }),
        )
        .map_err(|error| error.to_string())
    }

    pub(super) fn apply_environment_upgrade(
        &mut self,
        environment: muxy_api::execution_environment::ExecutionEnvironment,
        cx: &mut Context<Self>,
    ) {
        self.project_runtime.git_options = crate::git::options(&environment);
        self.repository_environment_upgraded(cx);
        self.refresh_project_truth(None, cx);
        self.sync_watchers();
    }

    pub(super) fn sync_watchers(&mut self) {
        let projects: Vec<(String, String)> = self
            .state
            .workspace
            .projects
            .iter()
            .filter(|project| project.is_git_repo && !project.is_home())
            .map(|project| (project.id.clone(), project.path.clone()))
            .collect();
        self.project_runtime.watchers.sync(&projects);
    }

    pub(crate) fn refresh_project_truth(
        &mut self,
        only: Option<&HashSet<String>>,
        cx: &mut Context<Self>,
    ) {
        let projects: Vec<_> = self
            .state
            .workspace
            .projects
            .iter()
            .filter(|project| !project.is_home())
            .filter(|project| only.is_none_or(|ids| ids.contains(&project.id)))
            .map(|project| {
                (
                    project.id.clone(),
                    project.name.clone(),
                    project.path.clone(),
                    self.state
                        .prefs
                        .active_worktree_ids
                        .get(&project.id)
                        .cloned(),
                    self.state
                        .worktrees
                        .get(&project.id)
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect();
        let requests: Vec<_> = projects
            .into_iter()
            .filter_map(
                |(
                    project_id,
                    project_name,
                    project_path,
                    preferred_worktree_id,
                    current_worktrees,
                )| {
                    let token = self
                        .state
                        .project_operations
                        .begin_background_probe(&project_id)?;
                    let probe = project_probe(
                        &token,
                        project_name,
                        project_path,
                        preferred_worktree_id,
                        current_worktrees,
                    );
                    Some((token, probe))
                },
            )
            .collect();
        if requests.is_empty() {
            return;
        }
        let (tokens, probes): (Vec<_>, Vec<_>) = requests.into_iter().unzip();
        let git_options = self.project_runtime.git_options.clone();
        cx.spawn(async move |window, cx| {
            let truth = cx
                .background_executor()
                .spawn(async move { muxy_api::truth::refresh_truth(&git_options, &probes) })
                .await;
            let _ = window.update(cx, |window, cx| {
                let mut fresh = HashSet::new();
                for (token, truth) in tokens.into_iter().zip(truth) {
                    if !token.matches(&truth.project_id, truth.generation, truth.request_id) {
                        let _ = window
                            .state
                            .project_operations
                            .commit_background_probe(&token, false);
                        continue;
                    }
                    let project_exists =
                        window.state.workspace.project(token.project_id()).is_some();
                    let Ok(committed) = window
                        .state
                        .project_operations
                        .commit_background_probe(&token, project_exists)
                    else {
                        continue;
                    };
                    if let Err(error) = window.commit_project_truth(truth) {
                        log::warn!("worktree refresh failed: {error}");
                    }
                    if committed.schedule_fresh_probe {
                        fresh.insert(token.project_id().to_owned());
                    }
                }
                if !fresh.is_empty() {
                    window.refresh_project_truth(Some(&fresh), cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn request_worktree_refresh(
        &mut self,
        project_id: String,
        responder: Option<muxy_proto::server::CommandResponder>,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.state.workspace.project(&project_id).cloned() else {
            let reply = format!("error:project not found {project_id}");
            self.surface_native_refresh_error(&responder, &reply, cx);
            respond_refresh(responder, reply);
            return;
        };
        let token = match self
            .state
            .project_operations
            .begin_operation(&project_id, ProjectOperationKind::Refresh)
        {
            Ok(token) => token,
            Err(BeginOperationError::Busy(_)) => {
                let reply = "error:worktree operation busy".to_owned();
                self.surface_native_refresh_error(&responder, &reply, cx);
                respond_refresh(responder, reply);
                return;
            }
        };
        let probe = project_probe(
            &token,
            project.name,
            project.path,
            self.state
                .prefs
                .active_worktree_ids
                .get(&project_id)
                .cloned(),
            self.state
                .worktrees
                .get(&project_id)
                .cloned()
                .unwrap_or_default(),
        );
        let git_options = self.project_runtime.git_options.clone();
        cx.spawn(async move |window, cx| {
            let mut truth = cx
                .background_executor()
                .spawn(async move { muxy_api::truth::refresh_truth(&git_options, &[probe]) })
                .await;
            let _ = window.update(cx, |window, cx| {
                let identity_matches = truth.first().is_some_and(|truth| {
                    token.matches(&truth.project_id, truth.generation, truth.request_id)
                });
                let project_exists = window.state.workspace.project(token.project_id()).is_some();
                let reply = if !identity_matches
                    || window
                        .state
                        .project_operations
                        .commit_explicit_refresh(&token, project_exists)
                        .is_err()
                {
                    "error:stale worktree refresh".to_owned()
                } else if let Some(truth) = truth.pop() {
                    match window.commit_project_truth(truth) {
                        Ok(count) => format!("ok\t{count}"),
                        Err(error) => format!("error:{error}"),
                    }
                } else {
                    "error:worktree refresh returned no result".to_owned()
                };
                let schedule_fresh_probe = window
                    .state
                    .project_operations
                    .finish_operation(&token)
                    .is_ok_and(|outcome| outcome.schedule_fresh_probe);
                window.surface_native_refresh_error(&responder, &reply, cx);
                respond_refresh(responder, reply);
                if schedule_fresh_probe
                    && window.state.workspace.project(token.project_id()).is_some()
                {
                    let ids = HashSet::from([token.project_id().to_owned()]);
                    window.refresh_project_truth(Some(&ids), cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn surface_native_refresh_error(
        &mut self,
        responder: &Option<muxy_proto::server::CommandResponder>,
        reply: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(error) = native_refresh_error(responder.is_some(), reply) {
            self.feedback(
                "Refresh Worktrees",
                error,
                crate::toast::ToastTone::Error,
                cx,
            );
        }
    }

    pub(crate) fn request_worktree_creation(
        &mut self,
        request: muxy_api::worktree_lifecycle::CreateWorktreeRequest,
        responder: muxy_proto::server::CommandResponder,
        cx: &mut Context<Self>,
    ) {
        let project_id = request.project.id.clone();
        let token = match self
            .state
            .project_operations
            .begin_operation(&project_id, ProjectOperationKind::Create)
        {
            Ok(token) => token,
            Err(BeginOperationError::Busy(_)) => {
                responder.respond(muxy_proto::server::CommandReply::new(
                    "error:worktree operation busy",
                ));
                return;
            }
        };
        let options = self.worktree_create_options(&project_id);
        cx.spawn(async move |window, cx| {
            let result = cx
                .background_executor()
                .spawn(
                    async move { muxy_api::worktree_lifecycle::create_worktree(request, &options) },
                )
                .await;
            let _ = window.update(cx, |window, cx| {
                let reply = match result {
                    Ok(outcome) => {
                        let created = outcome.worktree.clone();
                        let effects = window.state.apply_created_worktree(&project_id, outcome);
                        if !effects.navigation_recorded {
                            log::warn!("created worktree navigation was not recorded");
                        }
                        for warning in effects.warnings {
                            log::warn!("worktree creation warning: {warning:?}");
                        }
                        window.sync_watchers();
                        format!(
                            "ok\t{}\t{}\t{}\t{}",
                            created.id,
                            created.name,
                            created.path,
                            created.branch.as_deref().unwrap_or_default()
                        )
                    }
                    Err(error) => format!("error:{error}"),
                };
                let schedule_fresh_probe = window
                    .state
                    .project_operations
                    .finish_operation(&token)
                    .is_ok_and(|outcome| outcome.schedule_fresh_probe);
                responder.respond(muxy_proto::server::CommandReply::new(reply));
                if schedule_fresh_probe && window.state.workspace.project(&project_id).is_some() {
                    let ids = HashSet::from([project_id]);
                    window.refresh_project_truth(Some(&ids), cx);
                }
                cx.set_menus(crate::views::window::menu_bar::menus(&window.state));
                cx.activate(true);
                cx.notify();
            });
        })
        .detach();
    }

    fn worktree_create_options(
        &self,
        project_id: &str,
    ) -> muxy_api::worktree_lifecycle::CreateWorktreeOptions {
        let home = muxy_core::prefs::home_dir();
        muxy_api::worktree_lifecycle::CreateWorktreeOptions {
            git_options: self.project_runtime.git_options.clone(),
            worktrees_dir: muxy_core::store::worktrees::worktrees_dir(),
            current_worktrees: self
                .state
                .worktrees
                .get(project_id)
                .cloned()
                .unwrap_or_default(),
            location_context: muxy_api::worktree_location::LocationContext {
                home: home.clone(),
                profile_worktree_root: muxy_core::prefs::app_support_dir()
                    .join("worktree-checkouts"),
                default_path_template: self.state.prefs.default_worktree_path_template.clone(),
                default_parent_path: self.state.prefs.default_worktree_parent_path.clone(),
            },
            hook_options: muxy_api::worktree_hooks::HookOptions {
                global_config_path: muxy_api::worktree_config::global_config_path(
                    &home,
                    std::env::var_os("XDG_CONFIG_HOME").as_deref(),
                ),
                environment: Vec::new(),
            },
            timeout: std::time::Duration::from_secs(300),
        }
    }

    fn worktree_remove_options(
        &self,
        project_id: &str,
    ) -> muxy_api::worktree_lifecycle::RemoveWorktreeOptions {
        let create = self.worktree_create_options(project_id);
        muxy_api::worktree_lifecycle::RemoveWorktreeOptions {
            git_options: create.git_options,
            worktrees_dir: create.worktrees_dir,
            current_worktrees: create.current_worktrees,
            hook_options: create.hook_options,
            timeout: create.timeout,
        }
    }

    pub(crate) fn request_worktree_removal_inspection(
        &mut self,
        project_id: String,
        worktree_id: String,
        cx: &mut Context<Self>,
    ) {
        if self.state.project_operations.is_busy(&project_id) {
            self.feedback(
                "Remove Worktree",
                "Another worktree operation is still running.",
                crate::toast::ToastTone::Warning,
                cx,
            );
            return;
        }
        let Some(project) = self.state.workspace.project(&project_id).cloned() else {
            return;
        };
        let Some(worktree) = self.state.worktrees.get(&project_id).and_then(|worktrees| {
            worktrees
                .iter()
                .find(|worktree| worktree.id.eq_ignore_ascii_case(&worktree_id))
                .cloned()
        }) else {
            return;
        };
        if worktree.is_primary {
            self.feedback(
                "Remove Worktree",
                "The primary worktree cannot be removed.",
                crate::toast::ToastTone::Warning,
                cx,
            );
            return;
        }
        let token = match self
            .state
            .project_operations
            .begin_operation(&project_id, ProjectOperationKind::Remove)
        {
            Ok(token) => token,
            Err(BeginOperationError::Busy(_)) => {
                self.feedback(
                    "Remove Worktree",
                    "Another worktree operation is still running.",
                    crate::toast::ToastTone::Warning,
                    cx,
                );
                return;
            }
        };
        let identity =
            self.view
                .worktrees
                .begin_removal(&project_id, &worktree_id, token.generation());
        let options = self.worktree_remove_options(&project_id);
        cx.spawn(async move |window, cx| {
            let inspected = cx
                .background_executor()
                .spawn({
                    let project = project.clone();
                    let worktree = worktree.clone();
                    async move {
                        muxy_api::worktree_lifecycle::inspect_worktree_removal(
                            &project, &worktree, &options,
                        )
                    }
                })
                .await;
            let confirmation = window
                .update(cx, |window, cx| {
                    if !window.view.worktrees.matches_removal(&identity) {
                        let _ = window.state.project_operations.finish_operation(&token);
                        return None;
                    }
                    if !window.removal_request_is_current(&identity, &token, &project, &worktree) {
                        window.view.worktrees.clear_removal_if(&identity);
                        let _ = window.state.project_operations.finish_operation(&token);
                        return None;
                    }
                    let inspection = match inspected {
                        Ok(inspection) => inspection,
                        Err(error) => {
                            window.view.worktrees.clear_removal();
                            let _ = window.state.project_operations.finish_operation(&token);
                            window.feedback(
                                "Remove Worktree",
                                error.to_string(),
                                crate::toast::ToastTone::Error,
                                cx,
                            );
                            return None;
                        }
                    };
                    if let Some(diagnostic) = &inspection.inspection_diagnostic {
                        log::warn!("worktree removal inspection failed: {diagnostic}");
                    }
                    let approval = (inspection.worktree.source
                        == muxy_core::store::worktrees::Source::Muxy)
                        .then(|| {
                            muxy_api::worktree_config::ProjectHookApproval::from_resolved(
                                &inspection.teardown_commands,
                            )
                        });
                    let scope = crate::sessions::SessionOwnerScope {
                        project_id: project.id.to_ascii_uppercase(),
                        worktree_id: Some(inspection.worktree.id.to_ascii_uppercase()),
                    };
                    let cleanup_plan = match window.sessions.owner_cleanup_plan(&scope) {
                        Ok(plan) => plan,
                        Err(error) => {
                            window.view.worktrees.clear_removal();
                            let _ = window.state.project_operations.finish_operation(&token);
                            window.feedback(
                                "Remove Worktree",
                                error,
                                crate::toast::ToastTone::Error,
                                cx,
                            );
                            return None;
                        }
                    };
                    let session_count = cleanup_plan.candidates.len();
                    let mut message = removal_confirmation_message(&inspection);
                    if session_count > 0 {
                        message.push_str(&format!(
                            "\n\nThis will also end {session_count} background {} and every process inside {}.",
                            if session_count == 1 { "session" } else { "sessions" },
                            if session_count == 1 { "it" } else { "them" }
                        ));
                    }
                    let answer = window.ask(
                        format!("Remove '{}'?", inspection.worktree.name),
                        message,
                        &["Remove", "Cancel"],
                        cx,
                    );
                    Some((answer, inspection.worktree, approval, cleanup_plan))
                })
                .ok()
                .flatten();
            let Some((answer, inspected_worktree, approval, cleanup_plan)) = confirmation else {
                return;
            };
            if answer.await != Some(0) {
                let _ = window.update(cx, |window, _| {
                    window.view.worktrees.clear_removal_if(&identity);
                    let _ = window.state.project_operations.finish_operation(&token);
                });
                return;
            }
            let _ = window.update(cx, |window, cx| {
                if !window.view.worktrees.matches_removal(&identity) {
                    let _ = window.state.project_operations.finish_operation(&token);
                    return;
                }
                if !window.removal_request_is_current(
                    &identity,
                    &token,
                    &project,
                    &inspected_worktree,
                ) {
                    window.view.worktrees.clear_removal_if(&identity);
                    let _ = window.state.project_operations.finish_operation(&token);
                    return;
                }
                if let Err(error) = window.sessions.end_owner_cleanup_plan(&cleanup_plan) {
                    window.view.worktrees.clear_removal_if(&identity);
                    let _ = window.state.project_operations.finish_operation(&token);
                    window.feedback(
                        "Remove Worktree",
                        error,
                        crate::toast::ToastTone::Error,
                        cx,
                    );
                    return;
                }
                if !window.removal_request_is_current(
                    &identity,
                    &token,
                    &project,
                    &inspected_worktree,
                ) {
                    window.view.worktrees.clear_removal_if(&identity);
                    let _ = window.state.project_operations.finish_operation(&token);
                    window.feedback(
                        "Remove Worktree",
                        "The worktree changed while its background sessions were ending.",
                        crate::toast::ToastTone::Error,
                        cx,
                    );
                    return;
                }
                window.view.worktrees.clear_removal_if(&identity);
                window.request_native_worktree_removal(
                    token,
                    project,
                    inspected_worktree,
                    approval,
                    cx,
                );
            });
        })
        .detach();
    }

    fn removal_request_is_current(
        &self,
        identity: &super::view_state::RemovalRequestIdentity,
        token: &ProjectOperationToken,
        expected_project: &muxy_core::store::Project,
        expected: &muxy_core::store::worktrees::Worktree,
    ) -> bool {
        self.view.worktrees.matches_removal(identity)
            && self.state.project_operations.matches_operation(token)
            && self
                .state
                .workspace
                .project(&identity.project_id)
                .is_some_and(|project| {
                    project.path == expected_project.path
                        && project.is_git_repo == expected_project.is_git_repo
                        && project.remote_workspace_id == expected_project.remote_workspace_id
                })
            && self
                .state
                .project_operations
                .generation(&identity.project_id)
                == identity.generation
            && self
                .state
                .worktrees
                .get(&identity.project_id)
                .and_then(|worktrees| {
                    worktrees
                        .iter()
                        .find(|worktree| worktree.id.eq_ignore_ascii_case(&identity.worktree_id))
                })
                .is_some_and(|worktree| {
                    worktree.path == expected.path
                        && worktree.source == expected.source
                        && worktree.is_primary == expected.is_primary
                })
    }

    fn request_native_worktree_removal(
        &mut self,
        token: ProjectOperationToken,
        project: muxy_core::store::Project,
        worktree: muxy_core::store::worktrees::Worktree,
        project_hook_approval: Option<muxy_api::worktree_config::ProjectHookApproval>,
        cx: &mut Context<Self>,
    ) {
        let project_id = project.id.clone();
        let request = muxy_api::worktree_lifecycle::RemoveWorktreeRequest {
            project,
            worktree,
            project_hook_approval,
        };
        let options = self.worktree_remove_options(&project_id);
        cx.spawn(async move |window, cx| {
            let result = cx
                .background_executor()
                .spawn(
                    async move { muxy_api::worktree_lifecycle::remove_worktree(request, &options) },
                )
                .await;
            let _ = window.update(cx, |window, cx| {
                if !window.state.project_operations.matches_operation(&token) {
                    window.feedback(
                        "Remove Worktree",
                        "The worktree removal completed after its owner identity changed.",
                        crate::toast::ToastTone::Error,
                        cx,
                    );
                    return;
                }
                match result {
                    Ok(outcome) => {
                        let effects = window.state.apply_removed_worktree(&project_id, outcome);
                        if !effects.navigation_recorded {
                            log::warn!("removed worktree replacement navigation was not recorded");
                        }
                        for tab_id in &effects.tab_ids {
                            if let Some(handle) = window.terminal_runtime.surfaces.handle(tab_id) {
                                handle.request_close();
                            }
                        }
                        if !effects.warnings.is_empty() {
                            window.feedback(
                                "Worktree Removed",
                                native_removal_warning(&effects.warnings),
                                crate::toast::ToastTone::Warning,
                                cx,
                            );
                        }
                        window.sync_watchers();
                    }
                    Err(error) => {
                        window.feedback(
                            "Remove Worktree",
                            error.to_string(),
                            crate::toast::ToastTone::Error,
                            cx,
                        );
                    }
                }
                let schedule_fresh_probe = window
                    .state
                    .project_operations
                    .finish_operation(&token)
                    .is_ok_and(|outcome| outcome.schedule_fresh_probe);
                if schedule_fresh_probe && window.state.workspace.project(&project_id).is_some() {
                    let ids = HashSet::from([project_id.clone()]);
                    window.refresh_project_truth(Some(&ids), cx);
                }
                cx.set_menus(crate::views::window::menu_bar::menus(&window.state));
                cx.activate(true);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn request_native_worktree_creation(
        &mut self,
        request: muxy_api::worktree_lifecycle::CreateWorktreeRequest,
        path_template: Option<String>,
        parent_path: Option<String>,
        modal: Entity<create_worktree_overlay::CreateWorktreeModal>,
        cx: &mut Context<Self>,
    ) {
        let project_id = request.project.id.clone();
        let token = match self
            .state
            .project_operations
            .begin_operation(&project_id, ProjectOperationKind::Create)
        {
            Ok(token) => token,
            Err(BeginOperationError::Busy(_)) => {
                modal.update(cx, |modal, cx| {
                    modal.set_error("worktree operation busy".into(), cx)
                });
                return;
            }
        };
        modal.update(cx, |modal, cx| modal.set_running(true, cx));
        let options = self.worktree_create_options(&project_id);
        cx.spawn(async move |window, cx| {
            let result = cx
                .background_executor()
                .spawn(
                    async move { muxy_api::worktree_lifecycle::create_worktree(request, &options) },
                )
                .await;
            let _ = window.update(cx, |window, cx| {
                let succeeded = match result {
                    Ok(outcome) => {
                        let mut effects = window.state.apply_created_worktree(&project_id, outcome);
                        if !effects.navigation_recorded {
                            log::warn!("created worktree navigation was not recorded");
                        }
                        for warning in &effects.warnings {
                            log::warn!("worktree creation warning: {warning:?}");
                        }
                        let location_persisted =
                            window.state.workspace.update(&project_id, |project| {
                            project.preferred_worktree_path_template = path_template.clone();
                            project.preferred_worktree_parent_path = parent_path.clone();
                        });
                        if !location_persisted {
                            log::warn!("worktree location preference was not persisted");
                            effects.warnings.push(
                                muxy_api::worktree_lifecycle::LifecycleWarning::LocationPreferencePersistence(
                                    "the project workspace could not be saved".into(),
                                ),
                            );
                        }
                        window.view.worktrees.expand(&project_id);
                        window.view.worktrees.clear_create();
                        window.view.subscriptions.clear();
                        window.view.overlay = Overlay::None;
                        window.sync_watchers();
                        if !effects.warnings.is_empty() {
                            window.feedback(
                                "Worktree Created",
                                native_creation_warning(&effects.warnings),
                                crate::toast::ToastTone::Warning,
                                cx,
                            );
                        }
                        true
                    }
                    Err(error) => {
                        modal.update(cx, |modal, cx| modal.set_error(error.to_string(), cx));
                        false
                    }
                };
                let finish = window.state.project_operations.finish_operation(&token);
                let schedule_fresh_probe = finish
                    .as_ref()
                    .is_ok_and(|outcome| outcome.schedule_fresh_probe);
                if !succeeded && finish.is_ok() {
                    let generation = window.state.project_operations.generation(&project_id);
                    window.view.worktrees.rebase_create(&project_id, generation);
                }
                if schedule_fresh_probe && window.state.workspace.project(&project_id).is_some() {
                    let ids = HashSet::from([project_id.clone()]);
                    window.refresh_project_truth(Some(&ids), cx);
                }
                cx.set_menus(crate::views::window::menu_bar::menus(&window.state));
                if succeeded {
                    cx.activate(true);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn commit_project_truth(&mut self, truth: ProjectTruth) -> Result<usize, String> {
        let owner_existence = truth.owner_existence.clone();
        let refresh_error = truth
            .candidate
            .as_ref()
            .and_then(muxy_api::worktrees::RefreshCandidate::error)
            .map(str::to_owned);
        if truth
            .candidate
            .as_ref()
            .is_some_and(|candidate| candidate.worktrees().is_none())
        {
            return Err(refresh_error.unwrap_or_else(|| "worktree refresh unavailable".to_owned()));
        }
        let count = truth.worktrees.as_ref().map_or(0, Vec::len);
        self.sessions
            .reconcile_owner_existence(&owner_existence, &self.state.tab_workspaces)
            .map_err(|error| format!("owner reconciliation failed: {error}"))?;
        if let Some(candidate) = truth.candidate.as_ref() {
            muxy_api::worktrees::save_candidate(
                &muxy_core::store::worktrees::worktrees_dir(),
                &truth.project_id,
                candidate,
            )
            .map_err(|error| error.to_string())?;
        }
        self.state
            .apply_truth(vec![truth])
            .map_err(|_| "could not save project workspace".to_owned())?;
        self.sync_watchers();
        match refresh_error {
            Some(error) => Err(error),
            None => Ok(count),
        }
    }
}

fn project_probe(
    token: &impl ProjectRequestToken,
    project_name: String,
    project_path: String,
    preferred_worktree_id: Option<String>,
    current_worktrees: Vec<muxy_core::store::worktrees::Worktree>,
) -> ProjectProbe {
    ProjectProbe {
        project_id: token.project_id().to_owned(),
        project_name,
        project_path,
        preferred_worktree_id,
        current_worktrees,
        generation: token.generation(),
        request_id: token.request_id(),
    }
}

trait ProjectRequestToken {
    fn project_id(&self) -> &str;
    fn generation(&self) -> u64;
    fn request_id(&self) -> u64;
}

impl ProjectRequestToken for ProbeToken {
    fn project_id(&self) -> &str {
        self.project_id()
    }

    fn generation(&self) -> u64 {
        self.generation()
    }

    fn request_id(&self) -> u64 {
        self.request_id()
    }
}

impl ProjectRequestToken for ProjectOperationToken {
    fn project_id(&self) -> &str {
        self.project_id()
    }

    fn generation(&self) -> u64 {
        self.generation()
    }

    fn request_id(&self) -> u64 {
        self.request_id()
    }
}

fn respond_refresh(responder: Option<muxy_proto::server::CommandResponder>, reply: String) {
    if let Some(responder) = responder {
        responder.respond(muxy_proto::server::CommandReply::new(reply));
    }
}

fn native_refresh_error(has_responder: bool, reply: &str) -> Option<&str> {
    (!has_responder)
        .then(|| reply.strip_prefix("error:"))
        .flatten()
}

fn native_creation_warning(warnings: &[muxy_api::worktree_lifecycle::LifecycleWarning]) -> String {
    use muxy_api::worktree_lifecycle::LifecycleWarning;

    let details = warnings
        .iter()
        .map(|warning| match warning {
            LifecycleWarning::Reconciliation(error) => {
                format!("Worktree discovery could not be refreshed: {error}")
            }
            LifecycleWarning::WorktreeListPersistence(error) => {
                format!("The worktree list could not be saved: {error}")
            }
            LifecycleWarning::Setup(error) => format!("Setup did not finish: {error}"),
            LifecycleWarning::WorkspacePersistence(error) => {
                format!("The workspace could not be saved: {error}")
            }
            LifecycleWarning::ActivePreferencePersistence(error) => {
                format!("The active worktree preference could not be saved: {error}")
            }
            LifecycleWarning::LocationPreferencePersistence(error) => {
                format!("The location preference could not be saved: {error}")
            }
            LifecycleWarning::ReconciledGitRemoval(error) => {
                format!("Git removal reconciliation reported: {error}")
            }
            LifecycleWarning::PreservedFiles(error) => {
                format!("Some files were preserved: {error}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "The worktree was created and was not rolled back. Some follow-up steps need attention:\n\n{details}"
    )
}

fn removal_confirmation_message(
    inspection: &muxy_api::worktree_lifecycle::RemovalInspection,
) -> String {
    let mut message = if inspection.dirty {
        "This worktree has uncommitted changes. Removing it permanently discards those changes."
            .to_owned()
    } else {
        "This permanently removes the worktree checkout.".to_owned()
    };
    if !inspection.teardown_commands.is_empty() {
        message.push_str("\n\nThe following teardown commands will run first:");
        for command in &inspection.teardown_commands {
            let source = match command.source {
                muxy_api::worktree_config::CommandSource::Project => "Project",
                muxy_api::worktree_config::CommandSource::Global => "Per-machine",
            };
            let name = command
                .command
                .name
                .as_deref()
                .unwrap_or(&command.command.command);
            message.push_str(&format!("\n{source}: {name}"));
        }
    }
    message
}

fn native_removal_warning(warnings: &[muxy_api::worktree_lifecycle::LifecycleWarning]) -> String {
    use muxy_api::worktree_lifecycle::LifecycleWarning;

    let details = warnings
        .iter()
        .map(|warning| match warning {
            LifecycleWarning::Reconciliation(error) => {
                format!("Worktree discovery could not be refreshed: {error}")
            }
            LifecycleWarning::WorktreeListPersistence(error) => {
                format!("The worktree list could not be saved: {error}")
            }
            LifecycleWarning::Setup(error) => format!("Setup reported: {error}"),
            LifecycleWarning::WorkspacePersistence(error) => {
                format!("The workspace cleanup could not be saved: {error}")
            }
            LifecycleWarning::ActivePreferencePersistence(error) => {
                format!("The active worktree preference could not be saved: {error}")
            }
            LifecycleWarning::LocationPreferencePersistence(error) => {
                format!("The location preference reported: {error}")
            }
            LifecycleWarning::ReconciledGitRemoval(error) => error.clone(),
            LifecycleWarning::PreservedFiles(path) => {
                format!("The primary repository was missing, so files were preserved at {path}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("Removal completed and was not rolled back:\n\n{details}")
}

pub(super) fn spawn_terminal_pumps(
    terminals: &mut TerminalSurfaces,
    cx: &mut Context<MainWindow>,
) -> Vec<Task<()>> {
    let mut tasks = Vec::new();
    tasks.push(cx.spawn(async move |window, cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            if window
                .update(cx, |window, cx| {
                    let slept = window
                        .terminal_runtime
                        .surfaces
                        .poll_idle(&window.state.tab_workspaces);
                    if !slept.is_empty()
                        || window.terminal_runtime.surfaces.idle_reconcile_requested()
                    {
                        cx.notify();
                    }
                })
                .is_err()
            {
                return;
            }
        }
    }));
    if let Some(wakeups) = terminals.wakeups() {
        tasks.push(cx.spawn(async move |window, cx| {
            while wakeups.recv().await {
                let _ = window.update(cx, |window, _| window.terminal_runtime.surfaces.tick());
            }
        }));
    }
    if let Some(events) = terminals.events() {
        tasks.push(cx.spawn(async move |window, cx| {
            while let Some(event) = events.recv().await {
                let _ = window.update(cx, |window, cx| window.on_runtime_event(event, cx));
            }
        }));
    }
    if let Some(events) = terminals.navigation_events() {
        tasks.push(cx.spawn(async move |window, cx| {
            while let Ok(direction) = events.recv().await {
                let updated = window.update(cx, |window, cx| window.navigate(direction, cx));
                if updated.is_err() {
                    return;
                }
            }
        }));
    }
    if let Some(events) = terminals.external_drop_events() {
        tasks.push(cx.spawn(async move |window, cx| {
            while let Ok((identity, dropped)) = events.recv().await {
                let crate::terminal::SurfaceIdentity::Workspace(tab_id) = identity else {
                    continue;
                };
                if window
                    .update(cx, |window, cx| {
                        window.handle_terminal_drop(&tab_id, dropped, cx);
                    })
                    .is_err()
                {
                    return;
                }
            }
        }));
    }
    tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_api::worktree_lifecycle::LifecycleWarning;

    #[test]
    fn refresh_worktrees_routes_every_entry_through_the_coordinated_commit_seam() {
        let lifecycle = include_str!("lifecycle.rs");
        let window = include_str!("mod.rs");
        let socket = include_str!("../../socket/runtime.rs");

        assert!(lifecycle.contains("begin_background_probe"));
        assert!(lifecycle.contains("commit_background_probe"));
        assert!(lifecycle.contains("save_candidate"));
        assert!(lifecycle.contains("request_worktree_refresh"));
        assert!(lifecycle.contains("could not save project workspace"));
        assert!(window.contains("refresh_project_truth(None, cx)"));
        assert!(window.contains("refresh_project_truth(Some(&ids), cx)"));
        assert!(socket.contains("request_worktree_refresh"));
        assert!(!socket.contains("muxy_api::worktrees::refresh("));
    }

    #[test]
    fn environment_hydration_replaces_future_git_options_and_refreshes_project_truth() {
        let main = include_str!("../../main.rs");
        let window = include_str!("mod.rs");
        let lifecycle = include_str!("lifecycle.rs");

        assert!(main.contains("let execution_environment = git::environment_source();"));
        assert!(window.contains("execution_environment.start_hydration()"));
        assert!(window.contains("apply_environment_upgrade"));
        assert!(lifecycle.contains("crate::git::options(&environment)"));
        assert!(lifecycle.contains("self.refresh_project_truth(None, cx);"));
        assert!(lifecycle.contains("self.sync_watchers();"));
        assert!(lifecycle.contains("self.execution_environment.close();"));
    }

    #[test]
    fn native_worktree_failures_are_user_visible_and_creation_warnings_disclaim_rollback() {
        assert_eq!(
            native_refresh_error(false, "error:worktree operation busy"),
            Some("worktree operation busy")
        );
        assert_eq!(native_refresh_error(true, "error:socket failure"), None);
        assert_eq!(native_refresh_error(false, "ok\t3"), None);

        let warning = native_creation_warning(&[
            LifecycleWarning::Setup("command failed".into()),
            LifecycleWarning::LocationPreferencePersistence("save failed".into()),
        ]);
        assert!(warning.contains("was created and was not rolled back"));
        assert!(warning.contains("Setup did not finish: command failed"));
        assert!(warning.contains("location preference could not be saved: save failed"));
    }

    #[test]
    fn remove_worktree_confirmation_distinguishes_dirty_state_and_command_sources() {
        use muxy_api::worktree_config::{CommandSource, ResolvedCommand};
        use muxy_core::store::worktrees::{Source, Worktree};

        let inspection = muxy_api::worktree_lifecycle::RemovalInspection {
            project_id: "PROJECT".into(),
            worktree: Worktree {
                id: "SECONDARY".into(),
                name: "Feature".into(),
                path: "/feature".into(),
                branch: Some("feature".into()),
                source: Source::Muxy,
                is_primary: false,
                created_at: 1.0,
                last_active_at: None,
            },
            dirty: true,
            teardown_commands: vec![
                ResolvedCommand::new(
                    "project-clean",
                    Some("Project cleanup"),
                    CommandSource::Project,
                ),
                ResolvedCommand::new("machine-clean", None, CommandSource::Global),
            ],
            inspection_diagnostic: None,
        };
        let message = removal_confirmation_message(&inspection);
        assert!(message.contains("permanently discards those changes"));
        assert!(message.contains("Project: Project cleanup"));
        assert!(message.contains("Per-machine: machine-clean"));
    }
}
