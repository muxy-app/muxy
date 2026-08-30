use super::MainWindow;
use gpui::Context;
use muxy_terminal::backend::{ExternalDrop, shell_escape};
use muxy_terminal::input::{TerminalInputStep, TerminalInputTransaction};
use std::path::{Path, PathBuf};

fn path_values(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| path.is_absolute())
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn parsed_external_paths(paths: &[PathBuf]) -> Vec<String> {
    muxy_core::dropped_paths::parse(&path_values(paths), None)
}

fn terminal_drop_payload(dropped: &ExternalDrop) -> Option<String> {
    let paths =
        muxy_core::dropped_paths::parse(&dropped.file_values, dropped.plain_text.as_deref());
    (!paths.is_empty()).then(|| {
        paths
            .iter()
            .map(|path| shell_escape(path))
            .collect::<Vec<_>>()
            .join(" ")
    })
}

fn sidebar_drop_directories(paths: &[PathBuf]) -> Vec<String> {
    parsed_external_paths(paths)
        .into_iter()
        .filter(|path| Path::new(path).is_dir())
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
struct TerminalDropTarget {
    project_id: String,
    worktree_id: String,
    area_id: String,
}

fn terminal_drop_target(
    workspaces: &[muxy_core::workspace::WorkspaceState],
    tab_id: &str,
) -> Option<TerminalDropTarget> {
    workspaces.iter().find_map(|workspace| {
        let area_id = workspace.area_containing_tab(tab_id)?.id.clone();
        Some(TerminalDropTarget {
            project_id: workspace.project_id.clone(),
            worktree_id: workspace.worktree_id.clone()?,
            area_id,
        })
    })
}

impl MainWindow {
    pub(crate) fn handle_composer_drop(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        self.add_composer_files(parsed_external_paths(paths), cx);
    }

    pub(crate) fn handle_sidebar_drop(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        for directory in sidebar_drop_directories(paths) {
            self.add_dropped_project_directory(&directory, cx);
        }
    }

    fn add_dropped_project_directory(&mut self, path: &str, cx: &mut Context<Self>) {
        let existing = self
            .state
            .workspace
            .contains_path(path)
            .map(|project| project.id.clone());
        let id = match existing {
            Some(id) => Some(id),
            None => {
                let name = muxy_api::picker::path_service::last_component(path);
                self.state.workspace.add(name, path.to_owned())
            }
        };
        let Some(id) = id else {
            return;
        };
        if let Some(group_id) = self.state.workspace.active_group_id.clone() {
            self.state.workspace.groups.add_project(&id, &group_id);
        }
        self.state.workspace.sort();
        self.state.select_project(&id);
        self.refresh_project_truth(None, cx);
        cx.notify();
    }

    pub(crate) fn handle_terminal_drop(
        &mut self,
        tab_id: &str,
        dropped: ExternalDrop,
        cx: &mut Context<Self>,
    ) {
        let _ = self.enqueue_terminal_drop(tab_id, dropped, cx);
    }

    pub(crate) fn enqueue_terminal_drop(
        &mut self,
        tab_id: &str,
        dropped: ExternalDrop,
        cx: &mut Context<Self>,
    ) -> Option<async_channel::Receiver<muxy_terminal::input::TerminalInputResult>> {
        let payload = terminal_drop_payload(&dropped)?;
        self.terminal_runtime.surfaces.handle(tab_id)?;
        let target = terminal_drop_target(self.state.tab_workspaces.states(), tab_id)?;
        if !self
            .state
            .try_select_worktree(&target.project_id, &target.worktree_id)
        {
            return None;
        }
        self.focus_workspace_tab(&target.area_id, tab_id, cx);
        self.terminal_runtime.surfaces.set_focused_tab(Some(tab_id));
        Some(self.enqueue_terminal_input(
            tab_id.to_owned(),
            TerminalInputTransaction::new(vec![TerminalInputStep::BracketedText(payload)], false),
            cx,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_paths_composer_policy_preserves_order_for_files_and_images() {
        let paths = vec![
            PathBuf::from("/tmp/first.txt"),
            PathBuf::from("/tmp/image.png"),
            PathBuf::from("/tmp/first.txt"),
        ];
        assert_eq!(
            parsed_external_paths(&paths),
            ["/tmp/first.txt", "/tmp/image.png", "/tmp/first.txt"]
        );
    }

    #[test]
    fn dropped_paths_terminal_policy_uses_file_precedence_shell_escaping_and_no_return() {
        let dropped = ExternalDrop {
            file_values: vec![
                "file:///tmp/first%20file.txt".to_owned(),
                "/tmp/second's.png".to_owned(),
            ],
            plain_text: Some("/tmp/ignored".to_owned()),
        };
        assert_eq!(
            terminal_drop_payload(&dropped).as_deref(),
            Some("'/tmp/first file.txt' '/tmp/second'\\''s.png'")
        );
        let transaction = TerminalInputTransaction::new(
            vec![TerminalInputStep::BracketedText(
                terminal_drop_payload(&dropped).unwrap(),
            )],
            false,
        );
        assert!(!transaction.append_return);
    }

    #[test]
    fn dropped_paths_terminal_target_resolves_beyond_the_active_workspace() {
        let mut first = muxy_core::workspace::WorkspaceState::with_worktree(
            "project-a",
            "worktree-a",
            "/tmp/a",
        );
        first.new_top_level_tab(muxy_core::workspace::Tab::new(
            muxy_core::workspace::TabKind::Terminal,
        ));
        let mut second = muxy_core::workspace::WorkspaceState::with_worktree(
            "project-b",
            "worktree-b",
            "/tmp/b",
        );
        second.new_top_level_tab(muxy_core::workspace::Tab::new(
            muxy_core::workspace::TabKind::Terminal,
        ));
        let tab_id = second.top_level_order[0].clone();
        let area_id = second.area_containing_tab(&tab_id).unwrap().id.clone();
        assert_eq!(
            terminal_drop_target(&[first, second], &tab_id),
            Some(TerminalDropTarget {
                project_id: "project-b".to_owned(),
                worktree_id: "worktree-b".to_owned(),
                area_id,
            })
        );
    }

    #[test]
    fn dropped_paths_sidebar_policy_prefilters_files_and_preserves_directory_order() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let file = root.path().join("file.txt");
        let second = root.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::write(&file, b"file").unwrap();
        std::fs::create_dir(&second).unwrap();
        assert_eq!(
            sidebar_drop_directories(&[first.clone(), file, second.clone()]),
            [
                first.to_string_lossy().into_owned(),
                second.to_string_lossy().into_owned(),
            ]
        );
    }
}
