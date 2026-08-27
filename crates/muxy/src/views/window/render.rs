use super::*;

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.reconcile_terminals(window, cx);
        let project_ids = self
            .state
            .workspace
            .projects
            .iter()
            .map(|project| project.id.clone())
            .collect();
        self.view.worktrees.retain_projects(&project_ids);
        if let Some(handle) = self.view.pending_focus.take() {
            window.focus(&handle);
        } else if !self.view.overlay.is_open()
            && self.view.terminal.search_inputs.is_empty()
            && !self.view.workspace_focus.contains_focused(window, cx)
        {
            window.focus(&self.view.workspace_focus);
        }
        let layout = crate::views::app::AppLayout::new(
            &self.state.prefs,
            self.state.metrics,
            self.view.sidebar_expanded,
        );
        if let Some(workspace) = self.state.active_tab_workspace() {
            let tabs: HashSet<String> = workspace.top_level_order.iter().cloned().collect();
            let areas: HashSet<String> = workspace
                .visible_area_tabs()
                .into_iter()
                .map(|(area_id, _)| area_id)
                .collect();
            let groups = workspace.top_level_root.clone();
            self.view
                .workspace
                .tab_bounds
                .retain(|id, _| tabs.contains(id));
            self.view
                .workspace
                .area_bounds
                .retain(|id, _| areas.contains(id));
            self.view
                .workspace
                .group_bounds
                .retain(|id, _| groups.as_ref().is_some_and(|root| root.contains_group(id)));
        } else {
            self.view.workspace.tab_bounds.clear();
            self.view.workspace.area_bounds.clear();
            self.view.workspace.group_bounds.clear();
            self.view.workspace.split_bounds.clear();
        }
        let focused_working_directory = self.focused_working_directory();
        let drop_highlight = self.workspace_drop_highlight();
        let drag = self
            .view
            .terminal
            .scrollbar_drag
            .as_ref()
            .map(|drag| (drag.tab_id.as_str(), drag.origin));
        crate::views::app::render(
            crate::views::app::AppView {
                state: &self.state,
                layout,
                workspace_focus: &self.view.workspace_focus,
                menu_focus: &self.view.menu_focus,
                terminals: &self.terminal_runtime.surfaces,
                area_bounds: &self.view.workspace.area_bounds,
                search_inputs: &self.view.terminal.search_inputs,
                scrollbar_reveal: &self.view.terminal.scrollbar_reveal,
                terminal_attention: &self.view.terminal.attention,
                bell_flashes: &self.view.terminal.bell_flashes,
                drag,
                now: self.view.terminal.started_at.elapsed(),
                overlay: &self.view.overlay,
                drop_highlight,
                focused_working_directory,
                expanded_worktree_projects: self.view.worktrees.expanded_projects(),
            },
            window,
            cx,
        )
    }
}
