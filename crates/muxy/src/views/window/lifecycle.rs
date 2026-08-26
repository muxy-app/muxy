use super::*;

pub(super) struct ProjectRuntime {
    pub(super) truth_task: Option<Task<()>>,
    pub(super) watchers: muxy_api::watcher::Watchers,
    pub(super) git_options: muxy_api::git::GitOptions,
    pub(super) _watcher_task: Task<()>,
}

impl ProjectRuntime {
    pub(super) fn new(watchers: muxy_api::watcher::Watchers, watcher_task: Task<()>) -> Self {
        Self {
            truth_task: None,
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
        let projects: Vec<muxy_api::truth::ProjectProbe> = self
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
                )
            })
            .collect();
        if projects.is_empty() {
            return;
        }
        let git_options = self.project_runtime.git_options.clone();
        self.project_runtime.truth_task = Some(cx.spawn(async move |window, cx| {
            let truth = cx
                .background_executor()
                .spawn(async move { muxy_api::truth::refresh_truth(&git_options, &projects) })
                .await;
            let _ = window.update(cx, |window, cx| {
                window.state.apply_truth(truth);
                window.sync_watchers();
                cx.notify();
            });
        }));
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
    tasks
}
