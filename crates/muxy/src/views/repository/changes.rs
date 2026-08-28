use crate::project_operations::ProjectOperationKind;
use crate::repository::{LoadState, RepositoryKey, RepositoryRefreshSet};
use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, Styled};
use muxy_api::repository::{
    ChangedFile, ChangedFileId, ChangedFiles, LineStat, UntrackedLineCount,
};
use muxy_ui::command_popover::{
    CommandPopover, CommandPopoverAction, CommandPopoverItem, CommandPopoverLeading,
    CommandPopoverRow, CommandPopoverStatus,
};
use muxy_ui::icon::Icon;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ChangeSide {
    Conflict,
    Staged,
    Unstaged,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ChangeRowKey {
    pub(crate) file: ChangedFileId,
    pub(crate) side: ChangeSide,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresentedChangeRow {
    pub(crate) key: ChangeRowKey,
    pub(crate) file: ChangedFile,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) trailing: String,
    pub(crate) can_stage: bool,
    pub(crate) can_unstage: bool,
    pub(crate) can_discard: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresentedChangeSection {
    pub(crate) label: &'static str,
    pub(crate) rows: Vec<PresentedChangeRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChangesListState {
    Loading,
    Ready,
    Empty,
    Error(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChangesPresentation {
    pub(crate) state: ChangesListState,
    pub(crate) summary: String,
    pub(crate) sections: Vec<PresentedChangeSection>,
}

pub(crate) fn present_changes(
    changes: &LoadState<ChangedFiles>,
    query: &str,
    line_counts: &HashMap<ChangedFileId, UntrackedLineCount>,
) -> ChangesPresentation {
    let LoadState::Ready(changes) = changes else {
        return ChangesPresentation {
            state: match changes {
                LoadState::Idle | LoadState::Loading => ChangesListState::Loading,
                LoadState::Error(error) => ChangesListState::Error(error.clone()),
                LoadState::Ready(_) => unreachable!(),
            },
            summary: String::new(),
            sections: Vec::new(),
        };
    };
    let query = query.trim().to_lowercase();
    let mut conflicts = Vec::new();
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    for file in &changes.files {
        let path = file.display_path().into_owned();
        let old_path = file.display_old_path().map(|path| path.into_owned());
        let haystack = old_path
            .as_ref()
            .map_or_else(|| path.clone(), |old| format!("{old} {path}"))
            .to_lowercase();
        if !query.is_empty() && !haystack.contains(&query) {
            continue;
        }
        if file.is_conflicted {
            conflicts.push(present_row(
                file,
                ChangeSide::Conflict,
                line_counts.get(&file.stable_id()).copied(),
            ));
            continue;
        }
        if file.is_staged {
            staged.push(present_row(file, ChangeSide::Staged, None));
        }
        if file.is_unstaged {
            unstaged.push(present_row(
                file,
                ChangeSide::Unstaged,
                line_counts.get(&file.stable_id()).copied(),
            ));
        }
    }
    let mut sections = Vec::new();
    if !conflicts.is_empty() {
        sections.push(PresentedChangeSection {
            label: "Conflicts",
            rows: conflicts,
        });
    }
    if !staged.is_empty() {
        sections.push(PresentedChangeSection {
            label: "Staged Changes",
            rows: staged,
        });
    }
    if !unstaged.is_empty() {
        sections.push(PresentedChangeSection {
            label: "Changes",
            rows: unstaged,
        });
    }
    ChangesPresentation {
        state: if sections.is_empty() {
            ChangesListState::Empty
        } else {
            ChangesListState::Ready
        },
        summary: summary_label(changes),
        sections,
    }
}

fn present_row(
    file: &ChangedFile,
    side: ChangeSide,
    untracked_lines: Option<UntrackedLineCount>,
) -> PresentedChangeRow {
    let path = file.display_path().into_owned();
    let subtitle = match file.display_old_path() {
        Some(old) if old.as_ref() != path => {
            format!("{} → {} · {}", old, path, status_label(file, side))
        }
        _ => status_label(file, side).to_owned(),
    };
    PresentedChangeRow {
        key: ChangeRowKey {
            file: file.stable_id(),
            side,
        },
        file: file.clone(),
        title: path,
        subtitle,
        trailing: stat_label(file, side, untracked_lines),
        can_stage: matches!(side, ChangeSide::Conflict | ChangeSide::Unstaged),
        can_unstage: side == ChangeSide::Staged,
        can_discard: side == ChangeSide::Unstaged && !file.is_conflicted,
    }
}

fn status_label(file: &ChangedFile, side: ChangeSide) -> &'static str {
    if file.is_conflicted || side == ChangeSide::Conflict {
        return "UU";
    }
    if file.is_untracked {
        return "??";
    }
    let status = match side {
        ChangeSide::Staged => file.x_status,
        ChangeSide::Unstaged => file.y_status,
        ChangeSide::Conflict => b'U',
    };
    match status {
        b'A' => "A",
        b'D' => "D",
        b'R' => "R",
        b'C' => "C",
        b'T' => "T",
        b'M' => "M",
        _ => "?",
    }
}

fn summary_label(changes: &ChangedFiles) -> String {
    let count = changes.files.len();
    let files = if count == 1 { "file" } else { "files" };
    let totals = changes.total_lines;
    let mut label = format!(
        "{count} {files} · +{} −{}",
        totals.additions, totals.deletions
    );
    if totals.binary_files > 0 {
        write!(&mut label, " · {} binary", totals.binary_files)
            .expect("writing to String cannot fail");
    }
    if totals.unknown_files > 0 {
        write!(&mut label, " · {} unknown", totals.unknown_files)
            .expect("writing to String cannot fail");
    }
    label
}

fn stat_label(
    file: &ChangedFile,
    side: ChangeSide,
    untracked_lines: Option<UntrackedLineCount>,
) -> String {
    if file.is_untracked {
        return match untracked_lines {
            Some(UntrackedLineCount::Known(lines)) => format!("+{lines} −0"),
            Some(UntrackedLineCount::Unknown) => "Unknown".to_owned(),
            None => "Count lines".to_owned(),
        };
    }
    let stat = match side {
        ChangeSide::Conflict => file.combined_stat,
        ChangeSide::Staged => file.staged_stat,
        ChangeSide::Unstaged => file.unstaged_stat,
    };
    match stat {
        Some(LineStat { binary: true, .. }) => "Binary".to_owned(),
        Some(LineStat {
            additions: Some(additions),
            deletions: Some(deletions),
            binary: false,
        }) => format!("+{additions} −{deletions}"),
        _ => "Unknown".to_owned(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChangeSelectionGesture {
    Plain,
    Toggle,
    Range,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ChangeSelectionState {
    selected: HashSet<ChangeRowKey>,
    anchor: Option<ChangeRowKey>,
}

impl ChangeSelectionState {
    pub(crate) fn apply(
        &mut self,
        row: ChangeRowKey,
        gesture: ChangeSelectionGesture,
        ordered: &[ChangeRowKey],
    ) {
        match gesture {
            ChangeSelectionGesture::Plain => {
                self.selected.clear();
                self.selected.insert(row.clone());
                self.anchor = Some(row);
            }
            ChangeSelectionGesture::Toggle => {
                if !self.selected.remove(&row) {
                    self.selected.insert(row.clone());
                }
                self.anchor = Some(row);
            }
            ChangeSelectionGesture::Range => {
                let Some(anchor) = self.anchor.as_ref() else {
                    self.apply(row, ChangeSelectionGesture::Plain, ordered);
                    return;
                };
                let Some(start) = ordered.iter().position(|candidate| candidate == anchor) else {
                    self.apply(row, ChangeSelectionGesture::Plain, ordered);
                    return;
                };
                let Some(end) = ordered.iter().position(|candidate| candidate == &row) else {
                    return;
                };
                self.selected.clear();
                let (start, end) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };
                self.selected.extend(ordered[start..=end].iter().cloned());
            }
        }
    }

    pub(crate) fn prune(&mut self, ordered: &[ChangeRowKey]) {
        let available = ordered.iter().collect::<HashSet<_>>();
        self.selected.retain(|row| available.contains(row));
        if self
            .anchor
            .as_ref()
            .is_some_and(|anchor| !available.contains(anchor))
        {
            self.anchor = None;
        }
    }

    pub(crate) fn contains(&self, row: &ChangeRowKey) -> bool {
        self.selected.contains(row)
    }

    pub(crate) fn selected(&self) -> &HashSet<ChangeRowKey> {
        &self.selected
    }

    pub(crate) fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingDiscard {
    pub(crate) key: RepositoryKey,
    pub(crate) files: Vec<ChangedFileId>,
    pub(crate) permanently_deletes_untracked: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DiscardConfirmationState {
    pending: Option<PendingDiscard>,
}

impl DiscardConfirmationState {
    pub(crate) fn request(&mut self, key: RepositoryKey, files: &[ChangedFile]) -> bool {
        if files.is_empty() || files.iter().any(|file| file.is_conflicted) {
            return false;
        }
        self.pending = Some(PendingDiscard {
            key,
            files: files.iter().map(ChangedFile::stable_id).collect(),
            permanently_deletes_untracked: files.iter().any(|file| file.is_untracked),
        });
        true
    }

    pub(crate) fn take(
        &mut self,
        key: &RepositoryKey,
        current: &[ChangedFile],
    ) -> Option<Vec<ChangedFile>> {
        let pending = self.pending.take()?;
        if &pending.key != key {
            return None;
        }
        let files = pending
            .files
            .iter()
            .map(|id| current.iter().find(|file| file.stable_id() == *id).cloned())
            .collect::<Option<Vec<_>>>()?;
        (!files.is_empty() && files.iter().all(|file| !file.is_conflicted)).then_some(files)
    }

    pub(crate) fn cancel(&mut self) {
        self.pending = None;
    }

    pub(crate) fn repository_changed(&mut self, key: &RepositoryKey) {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| &pending.key != key)
        {
            self.pending = None;
        }
    }

    pub(crate) fn retain(&mut self, files: &[ChangedFile]) {
        let current = files
            .iter()
            .map(ChangedFile::stable_id)
            .collect::<HashSet<_>>();
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.files.iter().any(|file| !current.contains(file)))
        {
            self.pending = None;
        }
    }
}

pub(crate) fn discard_warning(files: &[ChangedFile]) -> String {
    match files {
        [file] if file.is_untracked => {
            format!("Delete '{}' permanently?", file.display_path())
        }
        [file] => format!(
            "Discard changes to '{}'? This cannot be undone.",
            file.display_path()
        ),
        files if files.iter().any(|file| file.is_untracked) => format!(
            "Discard {} selected files? Untracked files are permanently deleted.",
            files.len()
        ),
        files => format!(
            "Discard changes in {} selected files? This cannot be undone.",
            files.len()
        ),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChangesMutationKind {
    Stage(Vec<ChangedFile>),
    StageAll,
    Unstage(Vec<ChangedFile>),
    UnstageAll,
    Discard(Vec<ChangedFile>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChangesMutationPlan {
    pub(crate) key: RepositoryKey,
    pub(crate) kind: ChangesMutationKind,
    pub(crate) operation_kind: ProjectOperationKind,
    pub(crate) refresh: RepositoryRefreshSet,
    pub(crate) background: bool,
    pub(crate) revalidate_key: bool,
}

pub(crate) fn changes_mutation_plan(
    key: RepositoryKey,
    kind: ChangesMutationKind,
) -> ChangesMutationPlan {
    ChangesMutationPlan {
        key,
        kind,
        operation_kind: ProjectOperationKind::RepositoryMutation,
        refresh: RepositoryRefreshSet::summary_and_changes(),
        background: true,
        revalidate_key: true,
    }
}

pub(crate) struct ChangesPopover {
    pub(crate) key: RepositoryKey,
    pub(crate) picker: Entity<CommandPopover>,
    pub(crate) selection: ChangeSelectionState,
    pub(crate) discard: DiscardConfirmationState,
    pub(crate) operation_error: Option<String>,
    pub(crate) line_counts: HashMap<ChangedFileId, UntrackedLineCount>,
    pub(crate) removable_worktree: Option<(String, String)>,
    pub(crate) discard_in_flight: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ChangesOverlayPolicy {
    pub(crate) target_width: f32,
    pub(crate) target_height: f32,
}

pub(crate) fn changes_overlay_policy() -> ChangesOverlayPolicy {
    ChangesOverlayPolicy {
        target_width: 360.0,
        target_height: 380.0,
    }
}

pub(crate) fn render(popover: &ChangesPopover, anchor: gpui::Bounds<gpui::Pixels>) -> AnyElement {
    gpui::div()
        .absolute()
        .left(anchor.origin.x)
        .top(anchor.origin.y)
        .child(popover.picker.clone())
        .into_any_element()
}

pub(crate) fn sync_picker(
    popover: &mut ChangesPopover,
    changes: &LoadState<ChangedFiles>,
    mutation_busy: bool,
    cx: &mut Context<crate::views::window::MainWindow>,
) {
    let query = popover.picker.read(cx).query().to_owned();
    let presentation = present_changes(changes, &query, &popover.line_counts);
    let complete = present_changes(changes, "", &popover.line_counts);
    let ordered = complete
        .sections
        .iter()
        .flat_map(|section| section.rows.iter().map(|row| row.key.clone()))
        .collect::<Vec<_>>();
    popover.selection.prune(&ordered);
    if let LoadState::Ready(changes) = changes {
        popover.discard.repository_changed(&popover.key);
        popover.discard.retain(&changes.files);
    }
    let mut items = Vec::new();
    if !presentation.summary.is_empty() {
        items.push(CommandPopoverItem::section(presentation.summary.clone()));
    }
    for section in &presentation.sections {
        items.push(CommandPopoverItem::section(section.label));
        for presented in &section.rows {
            let mut row = CommandPopoverRow::new(row_id(&presented.key), presented.title.clone());
            row.subtitle = Some(presented.subtitle.clone().into());
            row.trailing = Some(presented.trailing.clone().into());
            row.leading = Some(CommandPopoverLeading::Icon(Icon::Code));
            row.selected = popover.selection.contains(&presented.key);
            row.disabled = mutation_busy;
            if !mutation_busy {
                if presented.can_stage {
                    row.actions.push(CommandPopoverAction::new(
                        "stage",
                        if presented.key.side == ChangeSide::Conflict {
                            "Mark Resolved"
                        } else {
                            "Stage"
                        },
                    ));
                }
                if presented.can_unstage {
                    row.actions
                        .push(CommandPopoverAction::new("unstage", "Unstage"));
                }
                if presented.file.is_untracked
                    && !popover
                        .line_counts
                        .contains_key(&presented.file.stable_id())
                {
                    row.actions
                        .push(CommandPopoverAction::new("stats", "Count lines"));
                }
                if presented.can_discard {
                    row.actions.push(
                        CommandPopoverAction::new(
                            "discard",
                            if presented.file.is_untracked {
                                "Delete Permanently"
                            } else {
                                "Discard Changes"
                            },
                        )
                        .icon(CommandPopoverLeading::Icon(Icon::Trash))
                        .destructive(true),
                    );
                }
            }
            items.push(CommandPopoverItem::Row(row));
        }
    }
    let selected = selected_rows(&complete, popover.selection.selected());
    let mut footer = Vec::new();
    let has_error = popover.operation_error.is_some()
        || matches!(presentation.state, ChangesListState::Error(_));
    if has_error {
        footer.push(CommandPopoverAction::new("retry", "Retry"));
    } else if selected.is_empty() {
        if presentation
            .sections
            .iter()
            .any(|section| section.rows.iter().any(|row| row.can_stage))
        {
            footer.push(CommandPopoverAction::new("stage-all", "Stage All"));
        }
        if presentation
            .sections
            .iter()
            .any(|section| section.rows.iter().any(|row| row.can_unstage))
        {
            footer.push(CommandPopoverAction::new("unstage-all", "Unstage All"));
        }
    } else {
        if selected.iter().any(|row| row.can_stage) {
            footer.push(CommandPopoverAction::new(
                "stage-selected",
                "Stage Selected",
            ));
        }
        if selected.iter().any(|row| row.can_unstage) {
            footer.push(CommandPopoverAction::new(
                "unstage-selected",
                "Unstage Selected",
            ));
        }
        if selected.iter().all(|row| row.can_discard) {
            footer.push(
                CommandPopoverAction::new("discard-selected", "Discard Selected").destructive(true),
            );
        }
    }
    if popover.removable_worktree.is_some() {
        footer.push(
            CommandPopoverAction::new("remove-worktree", "Remove Worktree").destructive(true),
        );
    }
    for action in &mut footer {
        action.disabled = mutation_busy;
    }
    let status = popover
        .operation_error
        .as_ref()
        .map(|error| CommandPopoverStatus::Error(error.clone().into()))
        .unwrap_or_else(|| match presentation.state {
            ChangesListState::Loading => CommandPopoverStatus::Loading("Loading changes…".into()),
            ChangesListState::Ready => CommandPopoverStatus::Ready,
            ChangesListState::Empty => CommandPopoverStatus::Empty(if query.trim().is_empty() {
                "Working tree clean".into()
            } else {
                "No matching changes".into()
            }),
            ChangesListState::Error(error) => CommandPopoverStatus::Error(error.into()),
        });
    popover.picker.update(cx, |picker, cx| {
        picker.set_items(items, cx);
        picker.set_footer_actions(footer, cx);
        picker.set_status(status, cx);
    });
}

pub(crate) fn ordered_keys(
    changes: &LoadState<ChangedFiles>,
    query: &str,
    line_counts: &HashMap<ChangedFileId, UntrackedLineCount>,
) -> Vec<ChangeRowKey> {
    present_changes(changes, query, line_counts)
        .sections
        .into_iter()
        .flat_map(|section| section.rows.into_iter().map(|row| row.key))
        .collect()
}

pub(crate) fn find_row(
    changes: &LoadState<ChangedFiles>,
    query: &str,
    line_counts: &HashMap<ChangedFileId, UntrackedLineCount>,
    id: &str,
) -> Option<PresentedChangeRow> {
    present_changes(changes, query, line_counts)
        .sections
        .into_iter()
        .flat_map(|section| section.rows)
        .find(|row| row_id(&row.key) == id)
}

pub(crate) fn selected_files(
    changes: &LoadState<ChangedFiles>,
    query: &str,
    line_counts: &HashMap<ChangedFileId, UntrackedLineCount>,
    selected: &HashSet<ChangeRowKey>,
    predicate: impl Fn(&PresentedChangeRow) -> bool,
) -> Vec<ChangedFile> {
    let presentation = present_changes(changes, query, line_counts);
    let mut seen = HashSet::new();
    presentation
        .sections
        .iter()
        .flat_map(|section| &section.rows)
        .filter(|row| selected.contains(&row.key) && predicate(row))
        .filter_map(|row| {
            seen.insert(row.file.stable_id())
                .then_some(row.file.clone())
        })
        .collect()
}

fn selected_rows<'a>(
    presentation: &'a ChangesPresentation,
    selected: &HashSet<ChangeRowKey>,
) -> Vec<&'a PresentedChangeRow> {
    presentation
        .sections
        .iter()
        .flat_map(|section| &section.rows)
        .filter(|row| selected.contains(&row.key))
        .collect()
}

pub(crate) fn row_id(key: &ChangeRowKey) -> String {
    let prefix = match key.side {
        ChangeSide::Conflict => "conflict",
        ChangeSide::Staged => "staged",
        ChangeSide::Unstaged => "unstaged",
    };
    let mut id = format!("change-{prefix}-");
    for byte in &key.file.path {
        write!(&mut id, "{byte:02x}").expect("writing to String cannot fail");
    }
    id.push('-');
    if let Some(old_path) = &key.file.old_path {
        for byte in old_path {
            write!(&mut id, "{byte:02x}").expect("writing to String cannot fail");
        }
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn key(path: &str) -> RepositoryKey {
        RepositoryKey {
            project_id: "project".to_owned(),
            worktree_id: "worktree".to_owned(),
            normalized_path: PathBuf::from(path),
        }
    }

    fn file(path: &str, x: u8, y: u8) -> ChangedFile {
        ChangedFile {
            path: path.as_bytes().to_vec(),
            old_path: None,
            x_status: x,
            y_status: y,
            is_staged: !matches!(x, b' ' | b'?'),
            is_unstaged: !matches!(y, b' '),
            is_untracked: x == b'?' && y == b'?',
            is_conflicted: x == b'U' || y == b'U',
            is_binary: false,
            combined_stat: Some(LineStat {
                additions: Some(3),
                deletions: Some(1),
                binary: false,
            }),
            staged_stat: Some(LineStat {
                additions: Some(2),
                deletions: Some(0),
                binary: false,
            }),
            unstaged_stat: Some(LineStat {
                additions: Some(1),
                deletions: Some(1),
                binary: false,
            }),
        }
    }

    fn snapshot() -> LoadState<ChangedFiles> {
        let mut conflicted = file("conflict.rs", b'U', b'U');
        conflicted.is_conflicted = true;
        let mut untracked = file("new.txt", b'?', b'?');
        untracked.staged_stat = None;
        untracked.unstaged_stat = None;
        LoadState::Ready(ChangedFiles {
            files: vec![
                conflicted,
                file("both.rs", b'M', b'M'),
                untracked,
                file("staged.rs", b'A', b' '),
            ],
            ..ChangedFiles::default()
        })
    }

    #[test]
    fn changes_presentation_sections_every_side_and_preserves_dual_rows() {
        let presentation = present_changes(&snapshot(), "", &HashMap::new());
        assert_eq!(presentation.state, ChangesListState::Ready);
        assert_eq!(
            presentation
                .sections
                .iter()
                .map(|section| section.label)
                .collect::<Vec<_>>(),
            ["Conflicts", "Staged Changes", "Changes"]
        );
        assert_eq!(presentation.sections[0].rows.len(), 1);
        assert_eq!(presentation.sections[1].rows.len(), 2);
        assert_eq!(presentation.sections[2].rows.len(), 2);
        let both = presentation
            .sections
            .iter()
            .flat_map(|section| &section.rows)
            .filter(|row| row.title == "both.rs")
            .collect::<Vec<_>>();
        assert_eq!(both.len(), 2);
        assert!(both.iter().any(|row| row.can_stage && row.can_discard));
        assert!(both.iter().any(|row| row.can_unstage && !row.can_discard));
        assert!(!presentation.sections[0].rows[0].can_discard);
        assert!(presentation.sections[0].rows[0].can_stage);
        assert!(!presentation.sections[0].rows[0].can_unstage);
        assert!(presentation.sections[0].rows[0].subtitle.contains("UU"));
        assert!(presentation.summary.starts_with("4 files ·"));
    }

    #[test]
    fn changes_presentation_handles_filter_binary_rename_and_lazy_untracked_stats() {
        let mut renamed = file("new name.rs", b'R', b' ');
        renamed.old_path = Some(b"old name.rs".to_vec());
        renamed.is_binary = true;
        renamed.staged_stat = Some(LineStat {
            additions: None,
            deletions: None,
            binary: true,
        });
        let mut untracked = file("notes.txt", b'?', b'?');
        untracked.unstaged_stat = None;
        let changes = LoadState::Ready(ChangedFiles {
            files: vec![renamed, untracked.clone()],
            ..ChangedFiles::default()
        });
        let filtered = present_changes(&changes, "old name", &HashMap::new());
        assert_eq!(filtered.sections[0].rows[0].trailing, "Binary");
        assert!(filtered.sections[0].rows[0].subtitle.contains('→'));
        let mut counts = HashMap::new();
        counts.insert(untracked.stable_id(), UntrackedLineCount::Known(12));
        let counted = present_changes(&changes, "notes", &counts);
        assert_eq!(counted.sections[0].rows[0].trailing, "+12 −0");
    }

    #[test]
    fn changes_presentation_preserves_copy_type_and_unknown_status_details() {
        let mut copied = file("copy.rs", b'C', b' ');
        copied.old_path = Some(b"source.rs".to_vec());
        let typed = file("typed.rs", b' ', b'T');
        let mut unknown = file("unknown.rs", b' ', b'M');
        unknown.unstaged_stat = None;
        let changes = LoadState::Ready(ChangedFiles {
            files: vec![copied, typed, unknown],
            ..ChangedFiles::default()
        });
        let presentation = present_changes(&changes, "", &HashMap::new());
        let rows = presentation
            .sections
            .iter()
            .flat_map(|section| &section.rows)
            .collect::<Vec<_>>();
        let copied = rows.iter().find(|row| row.title == "copy.rs").unwrap();
        assert!(copied.subtitle.contains("source.rs → copy.rs · C"));
        let typed = rows.iter().find(|row| row.title == "typed.rs").unwrap();
        assert_eq!(typed.subtitle, "T");
        let unknown = rows.iter().find(|row| row.title == "unknown.rs").unwrap();
        assert_eq!(unknown.trailing, "Unknown");
    }

    #[test]
    fn change_selection_supports_plain_toggle_range_and_pruning() {
        let rows = ordered_keys(&snapshot(), "", &HashMap::new());
        let mut selection = ChangeSelectionState::default();
        selection.apply(rows[1].clone(), ChangeSelectionGesture::Plain, &rows);
        selection.apply(rows[3].clone(), ChangeSelectionGesture::Range, &rows);
        assert_eq!(selection.selected().len(), 3);
        selection.apply(rows[2].clone(), ChangeSelectionGesture::Toggle, &rows);
        assert_eq!(selection.selected().len(), 2);
        selection.prune(&rows[..1]);
        assert!(selection.selected().is_empty());
    }

    #[test]
    fn discard_confirmation_rejects_conflicts_and_tracks_untracked_loss() {
        let LoadState::Ready(changes) = snapshot() else {
            unreachable!()
        };
        let mut state = DiscardConfirmationState::default();
        assert!(!state.request(key("/repo"), &changes.files[..1]));
        assert!(state.request(key("/repo"), &changes.files[2..3]));
        assert!(
            state
                .pending
                .as_ref()
                .unwrap()
                .permanently_deletes_untracked
        );
        assert!(discard_warning(&changes.files[2..3]).contains("permanently"));
        let confirmed = state.take(&key("/repo"), &changes.files).unwrap();
        assert_eq!(confirmed.len(), 1);
        assert!(state.take(&key("/repo"), &changes.files).is_none());
        assert!(state.request(key("/repo"), &changes.files[1..2]));
        assert!(discard_warning(&changes.files[1..2]).contains("cannot be undone"));
        state.cancel();
        assert!(state.pending.is_none());
        let mut renamed = changes.files[1].clone();
        renamed.path = b"new.rs".to_vec();
        renamed.old_path = Some(b"old.rs".to_vec());
        assert!(state.request(key("/repo"), std::slice::from_ref(&renamed)));
        assert_eq!(
            state
                .take(&key("/repo"), std::slice::from_ref(&renamed))
                .unwrap()[0]
                .old_path,
            Some(b"old.rs".to_vec())
        );
        assert!(state.request(key("/repo"), &changes.files[2..3]));
        state.repository_changed(&key("/other"));
        assert!(state.pending.is_none());
    }

    #[test]
    fn filtering_hides_selection_without_dropping_it() {
        let complete = ordered_keys(&snapshot(), "", &HashMap::new());
        let filtered = ordered_keys(&snapshot(), "new.txt", &HashMap::new());
        let hidden = complete
            .iter()
            .find(|key| !filtered.contains(key))
            .unwrap()
            .clone();
        let mut selection = ChangeSelectionState::default();
        selection.apply(hidden.clone(), ChangeSelectionGesture::Plain, &complete);
        selection.prune(&complete);
        assert!(selection.contains(&hidden));
        assert!(!filtered.contains(&hidden));
    }

    #[test]
    fn selected_actions_use_current_snapshot_files_and_reject_stale_rows() {
        let mut renamed = file("renamed.rs", b' ', b'M');
        renamed.old_path = Some(b"original.rs".to_vec());
        let changes = LoadState::Ready(ChangedFiles {
            files: vec![renamed],
            ..ChangedFiles::default()
        });
        let rows = present_changes(&changes, "", &HashMap::new())
            .sections
            .into_iter()
            .flat_map(|section| section.rows)
            .collect::<Vec<_>>();
        let rename_source = rows
            .iter()
            .find(|row| row.title == "renamed.rs" && row.can_discard)
            .unwrap();
        let selected = HashSet::from([rename_source.key.clone()]);
        let files = selected_files(&changes, "", &HashMap::new(), &selected, |row| {
            row.can_discard
        });
        assert_eq!(files, vec![rename_source.file.clone()]);
        assert_eq!(files[0].old_path, Some(b"original.rs".to_vec()));
        let stale = ChangeRowKey {
            file: ChangedFileId {
                path: b"gone.rs".to_vec(),
                old_path: None,
            },
            side: ChangeSide::Unstaged,
        };
        let stale_selected = HashSet::from([stale]);
        assert!(
            selected_files(&changes, "", &HashMap::new(), &stale_selected, |_| true).is_empty()
        );
        assert!(find_row(&changes, "", &HashMap::new(), "change-unstaged-dead").is_none());
    }

    #[test]
    fn changes_overlay_uses_the_compact_phase_seven_size() {
        assert_eq!(
            changes_overlay_policy(),
            ChangesOverlayPolicy {
                target_width: 360.0,
                target_height: 380.0,
            }
        );
    }

    #[test]
    fn changes_mutations_use_the_shared_lane_and_refresh_summary_and_changes() {
        let plan = changes_mutation_plan(key("/repo"), ChangesMutationKind::StageAll);
        assert_eq!(
            plan.operation_kind,
            ProjectOperationKind::RepositoryMutation
        );
        assert!(plan.background && plan.revalidate_key);
        assert!(
            plan.refresh
                .contains(crate::repository::RepositoryReadKind::Summary)
        );
        assert!(
            plan.refresh
                .contains(crate::repository::RepositoryReadKind::Changes)
        );
        assert!(
            !plan
                .refresh
                .contains(crate::repository::RepositoryReadKind::Branches)
        );
    }
}
