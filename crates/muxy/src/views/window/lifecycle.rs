use super::*;
use crate::project_operations::{
    BeginOperationError, ProbeToken, ProjectOperationKind, ProjectOperationToken,
};
use muxy_api::truth::{ProjectProbe, ProjectTruth};

pub(super) struct ProjectRuntime {
    pub(super) watchers: muxy_api::watcher::Watchers,
    pub(super) git_options: muxy_api::git::GitOptions,
    pub(super) _watcher_task: Task<()>,
}

impl ProjectRuntime {
    pub(super) fn new(watchers: muxy_api::watcher::Watchers, watcher_task: Task<()>) -> Self {
        Self {
            watchers,
            git_options: crate::git::options(),
            _watcher_task: watcher_task,
        }
    }
}

impl MainWindow {
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
            respond_refresh(responder, format!("error:project not found {project_id}"));
            return;
        };
        let token = match self
            .state
            .project_operations
            .begin_operation(&project_id, ProjectOperationKind::Refresh)
        {
            Ok(token) => token,
            Err(BeginOperationError::Busy(_)) => {
                respond_refresh(responder, "error:worktree operation busy".to_owned());
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
        let home = muxy_core::prefs::home_dir();
        let options = muxy_api::worktree_lifecycle::CreateWorktreeOptions {
            git_options: self.project_runtime.git_options.clone(),
            worktrees_dir: muxy_core::store::worktrees::worktrees_dir(),
            current_worktrees: self
                .state
                .worktrees
                .get(&project_id)
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
        };
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

    fn commit_project_truth(&mut self, truth: ProjectTruth) -> Result<usize, String> {
        let refresh_error = truth
            .candidate
            .as_ref()
            .and_then(muxy_api::worktrees::RefreshCandidate::error)
            .map(str::to_owned);
        if let Some(candidate) = truth.candidate.as_ref() {
            if candidate.worktrees().is_none() {
                return Err(
                    refresh_error.unwrap_or_else(|| "worktree refresh unavailable".to_owned())
                );
            }
            muxy_api::worktrees::save_candidate(
                &muxy_core::store::worktrees::worktrees_dir(),
                &truth.project_id,
                candidate,
            )
            .map_err(|error| error.to_string())?;
        }
        let count = truth.worktrees.as_ref().map_or(0, Vec::len);
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

pub(super) fn spawn_terminal_pumps(
    terminals: &mut TerminalSurfaces,
    cx: &mut Context<MainWindow>,
) -> Vec<Task<()>> {
    let mut tasks = Vec::new();
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
    tasks
}

#[cfg(test)]
mod tests {
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
}
