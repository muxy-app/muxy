use crate::project_operations::ProjectOperationKind;
use crate::repository::{LoadState, RepositoryKey, RepositoryRefreshSet};
use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, Styled};
use muxy_api::picker::path_service::natural_compare;
use muxy_api::repository::{BranchEntry, BranchKind, StashEntry};
use muxy_ui::command_popover::{
    CommandPopover, CommandPopoverAction, CommandPopoverItem, CommandPopoverLeading,
    CommandPopoverRow, CommandPopoverStatus,
};
use muxy_ui::icon::Icon;
use std::fmt::Write;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BranchListState {
    Loading,
    Ready,
    Empty,
    Error(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BranchCreationError {
    Invalid,
    Duplicate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BranchRow {
    pub(crate) name: String,
    pub(crate) raw_name: Vec<u8>,
    pub(crate) current: bool,
    pub(crate) switch_enabled: bool,
    pub(crate) delete_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BranchPresentation {
    pub(crate) state: BranchListState,
    pub(crate) rows: Vec<BranchRow>,
    pub(crate) creation_error: Option<BranchCreationError>,
    pub(crate) create_enabled: bool,
    pub(crate) actions_disabled_reason: Option<String>,
}

pub(crate) fn present_branches(
    branches: &LoadState<Vec<Vec<u8>>>,
    current_branch: Option<&str>,
    query: &str,
    creation: &str,
    mutation_busy: bool,
) -> BranchPresentation {
    let actions_disabled_reason =
        mutation_busy.then(|| "Another project mutation is running".to_owned());
    let creation = creation.trim();
    let all_branches = match branches {
        LoadState::Ready(branches) => branches.as_slice(),
        LoadState::Idle | LoadState::Loading | LoadState::Error(_) => &[],
    };
    let creation_error = if valid_branch_name(creation) {
        all_branches
            .iter()
            .any(|branch| branch.as_slice() == creation.as_bytes())
            .then_some(BranchCreationError::Duplicate)
    } else {
        Some(BranchCreationError::Invalid)
    };
    let normalized_query = query.trim().to_lowercase();
    let mut rows = all_branches
        .iter()
        .filter_map(|branch| {
            let name = String::from_utf8_lossy(branch).into_owned();
            (normalized_query.is_empty() || name.to_lowercase().contains(&normalized_query)).then(
                || {
                    let current = current_branch == Some(name.as_str());
                    BranchRow {
                        name,
                        raw_name: branch.clone(),
                        current,
                        switch_enabled: !mutation_busy && !current,
                        delete_enabled: !mutation_busy && !current,
                    }
                },
            )
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| natural_compare(&left.name, &right.name));
    let state = match branches {
        LoadState::Idle | LoadState::Loading => BranchListState::Loading,
        LoadState::Error(error) => BranchListState::Error(error.clone()),
        LoadState::Ready(_) if rows.is_empty() => BranchListState::Empty,
        LoadState::Ready(_) => BranchListState::Ready,
    };
    BranchPresentation {
        state,
        rows,
        creation_error,
        create_enabled: !mutation_busy && creation_error.is_none(),
        actions_disabled_reason,
    }
}

fn valid_branch_name(branch: &str) -> bool {
    !branch.is_empty()
        && branch == branch.trim()
        && !branch.starts_with('-')
        && !branch.starts_with('/')
        && !branch.ends_with('/')
        && !branch.ends_with('.')
        && !branch.ends_with(".lock")
        && !branch.contains("..")
        && !branch.contains("//")
        && !branch.contains("@{")
        && branch != "@"
        && !branch
            .chars()
            .any(|character| character.is_control() || " ~^:?*[\\".contains(character))
        && branch
            .split('/')
            .all(|component| !component.is_empty() && component != ".")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingBranchDeletion {
    pub(crate) key: RepositoryKey,
    pub(crate) branch: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BranchDeletionRefusal {
    CurrentBranch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BranchEscapeAction {
    CancelDeletion,
    DismissPopover,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct BranchDeletionState {
    pending: Option<PendingBranchDeletion>,
    error: Option<String>,
}

impl BranchDeletionState {
    pub(crate) fn request(
        &mut self,
        key: RepositoryKey,
        branch: Vec<u8>,
        current_branch: Option<&str>,
    ) -> Result<(), BranchDeletionRefusal> {
        if current_branch.is_some_and(|current| current.as_bytes() == branch) {
            return Err(BranchDeletionRefusal::CurrentBranch);
        }
        self.error = None;
        self.pending = Some(PendingBranchDeletion { key, branch });
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn cancel(&mut self) {
        self.pending = None;
    }

    #[cfg(test)]
    pub(crate) fn search_changed(&mut self) {
        self.cancel();
    }

    pub(crate) fn escape(&mut self) -> BranchEscapeAction {
        if self.pending.take().is_some() {
            BranchEscapeAction::CancelDeletion
        } else {
            BranchEscapeAction::DismissPopover
        }
    }

    pub(crate) fn repository_changed(&mut self, key: &RepositoryKey) {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| &pending.key != key)
        {
            self.pending = None;
        }
        self.error = None;
    }

    pub(crate) fn retain_branches(&mut self, branches: &[Vec<u8>]) {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| !branches.iter().any(|branch| branch == &pending.branch))
        {
            self.pending = None;
        }
    }

    pub(crate) fn finish(
        &mut self,
        key: &RepositoryKey,
        branch: &[u8],
        result: Result<(), String>,
    ) {
        if !self
            .pending
            .as_ref()
            .is_some_and(|pending| &pending.key == key && pending.branch.as_slice() == branch)
        {
            return;
        }
        self.pending = None;
        self.error = result.err();
    }

    pub(crate) fn pending(&self) -> Option<&PendingBranchDeletion> {
        self.pending.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BranchMutationKind {
    Switch(Vec<u8>),
    SwitchRemote(Vec<u8>),
    Create(String),
    Delete(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BranchMutationPlan {
    pub(crate) key: RepositoryKey,
    pub(crate) expected_current_branch: Option<String>,
    pub(crate) kind: BranchMutationKind,
    pub(crate) operation_kind: ProjectOperationKind,
    pub(crate) background: bool,
    pub(crate) revalidate_key: bool,
    pub(crate) revalidate_current_branch: bool,
    pub(crate) refresh: RepositoryRefreshSet,
}

pub(crate) fn branch_mutation_plan(
    key: RepositoryKey,
    expected_current_branch: Option<String>,
    kind: BranchMutationKind,
) -> BranchMutationPlan {
    BranchMutationPlan {
        key,
        expected_current_branch,
        kind,
        operation_kind: ProjectOperationKind::RepositoryMutation,
        background: true,
        revalidate_key: true,
        revalidate_current_branch: true,
        refresh: RepositoryRefreshSet::summary_branches_pull_request(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BranchOverlayPolicy {
    pub(crate) initial_focus_search: bool,
    pub(crate) restore_workspace_focus: bool,
    pub(crate) dismiss_on_outside_click: bool,
    pub(crate) clear_input_on_repository_change: bool,
    pub(crate) target_width: f32,
    pub(crate) target_height: f32,
}

pub(crate) fn branch_overlay_policy() -> BranchOverlayPolicy {
    BranchOverlayPolicy {
        initial_focus_search: true,
        restore_workspace_focus: true,
        dismiss_on_outside_click: true,
        clear_input_on_repository_change: true,
        target_width: 520.0,
        target_height: 400.0,
    }
}

pub(crate) fn clamp_axis(origin: f32, size: f32, viewport: f32, margin: f32) -> f32 {
    origin
        .max(margin)
        .min((viewport - size - margin).max(margin))
}

pub(crate) struct BranchPopover {
    pub(crate) key: RepositoryKey,
    pub(crate) picker: Entity<CommandPopover>,
    pub(crate) deletion: BranchDeletionState,
    pub(crate) operation_error: Option<String>,
    pub(crate) branch_entries: LoadState<Vec<BranchEntry>>,
    pub(crate) stashes: LoadState<Vec<StashEntry>>,
}

pub(crate) fn render(
    popover: &BranchPopover,
    _branches: &LoadState<Vec<Vec<u8>>>,
    _current_branch: Option<&str>,
    _mutation_busy: bool,
    anchor: gpui::Bounds<gpui::Pixels>,
    _state: &crate::state::AppState,
    _cx: &mut Context<crate::views::window::MainWindow>,
) -> AnyElement {
    gpui::div()
        .absolute()
        .left(anchor.origin.x)
        .top(anchor.origin.y)
        .child(popover.picker.clone())
        .into_any_element()
}

pub(crate) fn sync_picker(
    popover: &BranchPopover,
    mutation_busy: bool,
    cx: &mut Context<crate::views::window::MainWindow>,
) {
    let active_tab = popover.picker.read(cx).active_tab().to_owned();
    let query = popover.picker.read(cx).query().trim().to_lowercase();
    let (items, status) = if active_tab == "stashes" {
        stash_items(&popover.stashes, &query, mutation_busy)
    } else {
        branch_items(&popover.branch_entries, &query, mutation_busy)
    };
    let status = popover
        .operation_error
        .as_ref()
        .map(|error| CommandPopoverStatus::Error(error.clone().into()))
        .unwrap_or(status);
    popover.picker.update(cx, |picker, cx| {
        picker.set_items(items, cx);
        picker.set_status(status, cx);
    });
}

pub(crate) fn branch_row_id(branch: &BranchEntry) -> String {
    let kind = match branch.kind {
        BranchKind::Local => "local",
        BranchKind::Remote => "remote",
    };
    encoded_row_id(&format!("branch-{kind}"), &branch.name)
}

pub(crate) fn stash_row_id(stash: &StashEntry) -> String {
    encoded_row_id("stash", stash.stable_id())
}

fn encoded_row_id(prefix: &str, value: &[u8]) -> String {
    let mut id = String::with_capacity(prefix.len() + 1 + value.len() * 2);
    id.push_str(prefix);
    id.push('-');
    for byte in value {
        write!(&mut id, "{byte:02x}").expect("writing to String cannot fail");
    }
    id
}

fn branch_items(
    branches: &LoadState<Vec<BranchEntry>>,
    query: &str,
    mutation_busy: bool,
) -> (Vec<CommandPopoverItem>, CommandPopoverStatus) {
    let LoadState::Ready(branches) = branches else {
        return (Vec::new(), load_status(branches, "Loading branches…"));
    };
    let mut local = Vec::new();
    let mut remote = Vec::new();
    for branch in branches {
        let name = String::from_utf8_lossy(&branch.name);
        if !query.is_empty() && !name.to_lowercase().contains(query) {
            continue;
        }
        let mut row = CommandPopoverRow::new(branch_row_id(branch), name.to_string());
        row.current = branch.current;
        row.disabled = mutation_busy;
        row.leading = Some(CommandPopoverLeading::Icon(Icon::GitBranch));
        row.subtitle = Some(branch_subtitle(branch).into());
        if branch.kind == BranchKind::Local && !branch.current && !mutation_busy {
            row.actions.push(
                CommandPopoverAction::new("delete", "Delete branch")
                    .icon(CommandPopoverLeading::Icon(Icon::Trash))
                    .destructive(true),
            );
        }
        match branch.kind {
            BranchKind::Local => local.push(CommandPopoverItem::Row(row)),
            BranchKind::Remote => remote.push(CommandPopoverItem::Row(row)),
        }
    }
    let trimmed = query.trim();
    let can_create = valid_branch_name(trimmed)
        && !branches.iter().any(|branch| {
            branch.kind == BranchKind::Local && branch.name.as_slice() == trimmed.as_bytes()
        });
    let mut items = Vec::new();
    if can_create {
        items.push(CommandPopoverItem::section("Create Branch"));
        let mut row = CommandPopoverRow::new("create-branch", format!("Create {trimmed}"));
        row.leading = Some(CommandPopoverLeading::Icon(Icon::Plus));
        row.disabled = mutation_busy;
        items.push(CommandPopoverItem::Row(row));
    }
    if !local.is_empty() {
        items.push(CommandPopoverItem::section("Local Branches"));
        items.extend(local);
    }
    if !remote.is_empty() {
        items.push(CommandPopoverItem::section("Remote Branches"));
        items.extend(remote);
    }
    let status = if items.is_empty() {
        CommandPopoverStatus::Empty(if query.is_empty() {
            "No branches".into()
        } else {
            "No matching branches".into()
        })
    } else {
        CommandPopoverStatus::Ready
    };
    (items, status)
}

fn stash_items(
    stashes: &LoadState<Vec<StashEntry>>,
    query: &str,
    mutation_busy: bool,
) -> (Vec<CommandPopoverItem>, CommandPopoverStatus) {
    let LoadState::Ready(stashes) = stashes else {
        return (Vec::new(), load_status(stashes, "Loading stashes…"));
    };
    let mut stash_rows = stashes
        .iter()
        .filter_map(|stash| {
            let haystack = format!(
                "{} {}",
                stash.branch.as_deref().unwrap_or_default(),
                stash.message
            )
            .to_lowercase();
            (query.is_empty() || haystack.contains(query)).then(|| {
                let mut row = CommandPopoverRow::new(stash_row_id(stash), stash.message.clone());
                row.leading = Some(CommandPopoverLeading::Icon(Icon::Archive));
                row.subtitle = Some(
                    format!(
                        "{} · {}",
                        stash.branch.as_deref().unwrap_or("Detached"),
                        relative_time(stash.timestamp)
                    )
                    .into(),
                );
                row.disabled = mutation_busy;
                if !mutation_busy {
                    row.actions = vec![
                        CommandPopoverAction::new("preview", "Preview stash")
                            .icon(CommandPopoverLeading::Icon(Icon::Eye)),
                        CommandPopoverAction::new("apply", "Apply stash"),
                        CommandPopoverAction::new("pop", "Pop stash"),
                        CommandPopoverAction::new("drop", "Drop stash")
                            .icon(CommandPopoverLeading::Icon(Icon::Trash))
                            .destructive(true),
                    ];
                }
                CommandPopoverItem::Row(row)
            })
        })
        .collect::<Vec<_>>();
    let mut rows = vec![CommandPopoverItem::section("Working Tree")];
    let mut create = CommandPopoverRow::new("create-stash", "Stash current changes");
    create.leading = Some(CommandPopoverLeading::Icon(Icon::Plus));
    create.subtitle = Some("Include tracked and untracked changes".into());
    create.disabled = mutation_busy;
    rows.push(CommandPopoverItem::Row(create));
    if !stash_rows.is_empty() {
        rows.push(CommandPopoverItem::section("Stashes"));
        rows.append(&mut stash_rows);
    }
    let status = if rows.len() == 2 && !query.is_empty() {
        CommandPopoverStatus::Empty(if query.is_empty() {
            "No stashes".into()
        } else {
            "No matching stashes".into()
        })
    } else {
        CommandPopoverStatus::Ready
    };
    (rows, status)
}

fn load_status<T>(state: &LoadState<T>, loading: &'static str) -> CommandPopoverStatus {
    match state {
        LoadState::Idle | LoadState::Loading => CommandPopoverStatus::Loading(loading.into()),
        LoadState::Error(error) => CommandPopoverStatus::Error(error.clone().into()),
        LoadState::Ready(_) => CommandPopoverStatus::Ready,
    }
}

fn branch_subtitle(branch: &BranchEntry) -> String {
    let author = if branch.author.is_empty() {
        "Unknown"
    } else {
        &branch.author
    };
    let subject = if branch.subject.is_empty() {
        "No commits"
    } else {
        &branch.subject
    };
    format!("{author} · {} · {subject}", relative_time(branch.timestamp))
}

fn relative_time(timestamp: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64);
    let seconds = now.saturating_sub(timestamp).max(0);
    match seconds {
        0..=59 => "now".to_owned(),
        60..=3_599 => format!("{} minutes ago", seconds / 60),
        3_600..=86_399 => format!("{} hours ago", seconds / 3_600),
        86_400..=604_799 => format!("{} days ago", seconds / 86_400),
        _ => format!("{} weeks ago", seconds / 604_800),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_operations::ProjectOperationKind;
    use crate::repository::{LoadState, RepositoryKey, RepositoryReadKind};
    use std::path::PathBuf;

    fn key(path: &str) -> RepositoryKey {
        RepositoryKey {
            project_id: "project".to_owned(),
            worktree_id: "worktree".to_owned(),
            normalized_path: PathBuf::from(path),
        }
    }

    fn branches(names: &[&str]) -> LoadState<Vec<Vec<u8>>> {
        LoadState::Ready(names.iter().map(|name| name.as_bytes().to_vec()).collect())
    }

    #[test]
    fn branch_presentation_naturally_sorts_filters_and_selects_current_branch() {
        let presented = present_branches(
            &branches(&["feature10", "main", "feature2", "release"]),
            Some("main"),
            "FEATURE",
            "",
            false,
        );
        assert_eq!(presented.state, BranchListState::Ready);
        assert_eq!(
            presented
                .rows
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["feature2", "feature10"]
        );
        assert!(presented.rows.iter().all(|row| !row.current));

        let current = present_branches(&branches(&["topic", "main"]), Some("main"), "", "", false);
        assert!(
            current
                .rows
                .iter()
                .any(|row| row.name == "main" && row.current)
        );
        assert!(
            current
                .rows
                .iter()
                .any(|row| row.name == "topic" && row.delete_enabled)
        );
        assert!(
            current
                .rows
                .iter()
                .any(|row| row.name == "main" && !row.delete_enabled)
        );
    }

    #[test]
    fn branch_presentation_models_loading_empty_error_creation_and_disabled_reasons() {
        assert_eq!(
            present_branches(&LoadState::Loading, None, "", "", false).state,
            BranchListState::Loading
        );
        assert_eq!(
            present_branches(&branches(&[]), None, "", "", false).state,
            BranchListState::Empty
        );
        assert_eq!(
            present_branches(
                &LoadState::Error("branch read failed".to_owned()),
                None,
                "",
                "",
                false,
            )
            .state,
            BranchListState::Error("branch read failed".to_owned())
        );

        let available = branches(&["main", "topic"]);
        assert_eq!(
            present_branches(&available, Some("main"), "", "topic", false).creation_error,
            Some(BranchCreationError::Duplicate)
        );
        for invalid in ["", "   ", "-topic", "bad name", "topic..next"] {
            assert_eq!(
                present_branches(&available, Some("main"), "", invalid, false).creation_error,
                Some(BranchCreationError::Invalid)
            );
        }
        assert_eq!(
            present_branches(&available, Some("main"), "", "new-topic", false).creation_error,
            None
        );
        let busy = present_branches(&available, Some("main"), "", "new-topic", true);
        assert_eq!(
            busy.actions_disabled_reason.as_deref(),
            Some("Another project mutation is running")
        );
        assert!(
            busy.rows
                .iter()
                .all(|row| !row.switch_enabled && !row.delete_enabled)
        );
        assert!(!busy.create_enabled);
    }

    #[test]
    fn inline_deletion_cancels_and_revalidates_every_identity_boundary() {
        let repository = key("/repo");
        let mut deletion = BranchDeletionState::default();
        assert_eq!(
            deletion.request(repository.clone(), b"main".to_vec(), Some("main")),
            Err(BranchDeletionRefusal::CurrentBranch)
        );
        deletion
            .request(repository.clone(), b"topic".to_vec(), Some("main"))
            .unwrap();
        assert_eq!(deletion.escape(), BranchEscapeAction::CancelDeletion);
        assert_eq!(deletion.escape(), BranchEscapeAction::DismissPopover);

        deletion
            .request(repository.clone(), b"topic".to_vec(), Some("main"))
            .unwrap();
        deletion.search_changed();
        assert!(deletion.pending().is_none());

        deletion
            .request(repository.clone(), b"topic".to_vec(), Some("main"))
            .unwrap();
        deletion.repository_changed(&key("/other"));
        assert!(deletion.pending().is_none());

        deletion
            .request(repository.clone(), b"topic".to_vec(), Some("main"))
            .unwrap();
        deletion.retain_branches(&[b"main".to_vec()]);
        assert!(deletion.pending().is_none());
    }

    #[test]
    fn inline_deletion_success_and_failure_clear_or_preserve_actionable_state() {
        let repository = key("/repo");
        let mut deletion = BranchDeletionState::default();
        deletion
            .request(repository.clone(), b"topic".to_vec(), Some("main"))
            .unwrap();
        deletion.finish(&repository, b"topic", Ok(()));
        assert!(deletion.pending().is_none());
        assert!(deletion.error().is_none());

        deletion
            .request(repository.clone(), b"topic".to_vec(), Some("main"))
            .unwrap();
        deletion.finish(&repository, b"topic", Err("branch changed".to_owned()));
        assert!(deletion.pending().is_none());
        assert_eq!(deletion.error(), Some("branch changed"));
    }

    #[test]
    fn branch_intents_use_one_background_mutation_lane_and_exact_refreshes() {
        for kind in [
            BranchMutationKind::Switch(b"topic".to_vec()),
            BranchMutationKind::SwitchRemote(b"origin/topic".to_vec()),
            BranchMutationKind::Create("new-topic".to_owned()),
            BranchMutationKind::Delete(b"old-topic".to_vec()),
        ] {
            let plan = branch_mutation_plan(key("/repo"), Some("main".to_owned()), kind);
            assert_eq!(
                plan.operation_kind,
                ProjectOperationKind::RepositoryMutation
            );
            assert!(plan.background);
            assert!(plan.revalidate_key);
            assert!(plan.revalidate_current_branch);
            assert!(plan.refresh.contains(RepositoryReadKind::Summary));
            assert!(plan.refresh.contains(RepositoryReadKind::Branches));
            assert!(plan.refresh.contains(RepositoryReadKind::PullRequest));
            assert!(!plan.refresh.contains(RepositoryReadKind::Changes));
        }
    }

    #[test]
    fn repository_branch_overlay_has_one_clamped_focus_restoring_surface() {
        let policy = branch_overlay_policy();
        assert!(policy.initial_focus_search);
        assert!(policy.restore_workspace_focus);
        assert!(policy.dismiss_on_outside_click);
        assert!(policy.clear_input_on_repository_change);
        assert_eq!(policy.target_width, 520.0);
        assert_eq!(policy.target_height, 400.0);
        assert_eq!(clamp_axis(760.0, 520.0, 800.0, 8.0), 272.0);
        assert_eq!(clamp_axis(-10.0, 520.0, 800.0, 8.0), 8.0);
    }

    #[test]
    fn repository_picker_builds_rich_local_remote_and_create_rows() {
        let entries = LoadState::Ready(vec![
            BranchEntry {
                name: b"main".to_vec(),
                oid: vec![b'a'; 40],
                kind: BranchKind::Local,
                current: true,
                upstream: Some(b"origin/main".to_vec()),
                author: "Muxy".to_owned(),
                subject: "Current work".to_owned(),
                timestamp: 1,
            },
            BranchEntry {
                name: b"topic".to_vec(),
                oid: vec![b'b'; 40],
                kind: BranchKind::Local,
                current: false,
                upstream: None,
                author: "Muxy".to_owned(),
                subject: "Topic work".to_owned(),
                timestamp: 2,
            },
            BranchEntry {
                name: b"origin/release".to_vec(),
                oid: vec![b'c'; 40],
                kind: BranchKind::Remote,
                current: false,
                upstream: None,
                author: "Muxy".to_owned(),
                subject: "Release work".to_owned(),
                timestamp: 3,
            },
        ]);
        let (items, status) = branch_items(&entries, "", false);
        assert_eq!(status, CommandPopoverStatus::Ready);
        let rows = items
            .iter()
            .filter_map(|item| match item {
                CommandPopoverItem::Row(row) => Some(row),
                CommandPopoverItem::Section(_) => None,
            })
            .collect::<Vec<_>>();
        assert!(rows.iter().any(|row| row.title == "main" && row.current));
        assert!(rows.iter().any(|row| {
            row.title == "topic" && row.actions.iter().any(|action| action.id == "delete")
        }));
        assert!(rows.iter().any(|row| row.title == "origin/release"));

        let (create, _) = branch_items(&entries, "new-topic", false);
        assert!(create.iter().any(|item| {
            matches!(item, CommandPopoverItem::Row(row) if row.id == "create-branch")
        }));
    }

    #[test]
    fn repository_picker_builds_complete_stash_actions_and_search_state() {
        let entry = StashEntry {
            index: 0,
            reference: "stash@{0}".to_owned(),
            oid: vec![b'a'; 40],
            branch: Some("main".to_owned()),
            message: "Saved work".to_owned(),
            timestamp: 1,
        };
        let expected_id = stash_row_id(&entry);
        let stashes = LoadState::Ready(vec![entry]);
        let (items, status) = stash_items(&stashes, "saved", false);
        assert_eq!(status, CommandPopoverStatus::Ready);
        let stash = items
            .iter()
            .find_map(|item| match item {
                CommandPopoverItem::Row(row) if row.id == expected_id => Some(row),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            stash
                .actions
                .iter()
                .map(|action| action.id.as_ref())
                .collect::<Vec<_>>(),
            ["preview", "apply", "pop", "drop"]
        );

        let (_, status) = stash_items(&stashes, "missing", false);
        assert_eq!(
            status,
            CommandPopoverStatus::Empty("No matching stashes".into())
        );
    }
}
