use crate::sessions::{
    ManagedSession, ManagedSessionState, SessionCoordinator, SessionReattachOutcome,
};
use muxy_core::workspace_store::WorkspaceStore;
use muxy_proto::session::{SessionId, WorkspacePlacement};
use muxy_ui::command_popover::{
    CommandPopover, CommandPopoverAction, CommandPopoverConfig, CommandPopoverDensity,
    CommandPopoverItem, CommandPopoverPresentation, CommandPopoverRow, CommandPopoverStatus,
    CommandPopoverTab,
};

pub const PANEL_WIDTH: f32 = 620.0;
pub const PANEL_HEIGHT: f32 = 480.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionManagerActionKind {
    Focus,
    Reattach,
    End,
    StartNew,
    Remove,
}

impl SessionManagerActionKind {
    fn id(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Reattach => "reattach",
            Self::End => "end",
            Self::StartNew => "start-new",
            Self::Remove => "remove",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Focus => "Focus",
            Self::Reattach => "Reattach",
            Self::End => "End",
            Self::StartNew => "Start New",
            Self::Remove => "Remove",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionManagerAction {
    pub kind: SessionManagerActionKind,
    pub session_id: SessionId,
}

impl SessionManagerAction {
    pub fn id(&self) -> String {
        format!("{}:{}", self.kind.id(), self.session_id)
    }

    pub fn parse(value: &str) -> Option<Self> {
        let (kind, session_id) = value.split_once(':')?;
        let kind = match kind {
            "focus" => SessionManagerActionKind::Focus,
            "reattach" => SessionManagerActionKind::Reattach,
            "end" => SessionManagerActionKind::End,
            "start-new" => SessionManagerActionKind::StartNew,
            "remove" => SessionManagerActionKind::Remove,
            _ => return None,
        };
        Some(Self {
            kind,
            session_id: SessionId::parse(session_id).ok()?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionManagerRowAction {
    pub action: SessionManagerAction,
    pub label: &'static str,
    pub destructive: bool,
    pub disabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionManagerRow {
    pub section: &'static str,
    pub session_id: SessionId,
    pub title: String,
    pub subtitle: String,
    pub state: &'static str,
    pub actions: Vec<SessionManagerRowAction>,
    pub accessibility_label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionManagerModel {
    pub rows: Vec<SessionManagerRow>,
    pub active_count: usize,
    pub accessibility_summary: String,
}

pub fn model(sessions: &[ManagedSession], query: &str) -> SessionManagerModel {
    let query = query.trim().to_ascii_lowercase();
    let mut rows = sessions
        .iter()
        .filter(|session| {
            query.is_empty()
                || session.title.to_ascii_lowercase().contains(&query)
                || session
                    .working_directory
                    .to_ascii_lowercase()
                    .contains(&query)
                || session
                    .owner
                    .project_id
                    .to_ascii_lowercase()
                    .contains(&query)
                || session
                    .owner
                    .worktree_id
                    .to_ascii_lowercase()
                    .contains(&query)
                || session
                    .session_id
                    .to_string()
                    .to_ascii_lowercase()
                    .contains(&query)
        })
        .map(row_model)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (section_order(row.section), row.session_id));
    let active_count = sessions
        .iter()
        .filter(|session| {
            matches!(
                session.state,
                ManagedSessionState::Workspace
                    | ManagedSessionState::Background
                    | ManagedSessionState::AttachmentFailed
            )
        })
        .count();
    let accessibility_summary = format!(
        "Terminal Sessions, {} total, {} active",
        sessions.len(),
        active_count
    );
    SessionManagerModel {
        rows,
        active_count,
        accessibility_summary,
    }
}

fn row_model(session: &ManagedSession) -> SessionManagerRow {
    let (section, state, actions) = match session.state {
        ManagedSessionState::Workspace => (
            "Workspace Sessions",
            "Workspace",
            vec![
                row_action(
                    SessionManagerActionKind::Focus,
                    session.session_id,
                    false,
                    false,
                ),
                row_action(
                    SessionManagerActionKind::End,
                    session.session_id,
                    true,
                    false,
                ),
            ],
        ),
        ManagedSessionState::Background => (
            "Background Sessions",
            "Background",
            vec![
                row_action(
                    SessionManagerActionKind::Reattach,
                    session.session_id,
                    false,
                    false,
                ),
                row_action(
                    SessionManagerActionKind::End,
                    session.session_id,
                    true,
                    false,
                ),
            ],
        ),
        ManagedSessionState::AttachmentFailed => (
            "Missing/Ended",
            "Owner Changed",
            vec![
                row_action(
                    SessionManagerActionKind::Reattach,
                    session.session_id,
                    false,
                    true,
                ),
                row_action(
                    SessionManagerActionKind::End,
                    session.session_id,
                    true,
                    false,
                ),
            ],
        ),
        ManagedSessionState::Missing => (
            "Missing/Ended",
            "Missing",
            vec![
                row_action(
                    SessionManagerActionKind::StartNew,
                    session.session_id,
                    false,
                    session.placement.is_none(),
                ),
                row_action(
                    SessionManagerActionKind::Remove,
                    session.session_id,
                    true,
                    session.placement.is_none(),
                ),
            ],
        ),
        ManagedSessionState::Ended => (
            "Missing/Ended",
            "Ended",
            vec![
                row_action(
                    SessionManagerActionKind::StartNew,
                    session.session_id,
                    false,
                    session.placement.is_none(),
                ),
                row_action(
                    SessionManagerActionKind::Remove,
                    session.session_id,
                    true,
                    session.placement.is_none(),
                ),
            ],
        ),
    };
    let title = if session.title.trim().is_empty() {
        "Terminal".to_owned()
    } else {
        session.title.clone()
    };
    let subtitle = format!(
        "{} / {} · {}",
        session.owner.project_id, session.owner.worktree_id, session.working_directory
    );
    let action_labels = actions
        .iter()
        .map(|action| {
            if action.disabled {
                format!("{} unavailable", action.label)
            } else {
                action.label.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let accessibility_label = format!(
        "{title}, {state}, project {}, worktree {}, {}, actions {action_labels}",
        session.owner.project_id, session.owner.worktree_id, session.working_directory
    );
    SessionManagerRow {
        section,
        session_id: session.session_id,
        title,
        subtitle,
        state,
        actions,
        accessibility_label,
    }
}

fn row_action(
    kind: SessionManagerActionKind,
    session_id: SessionId,
    destructive: bool,
    disabled: bool,
) -> SessionManagerRowAction {
    SessionManagerRowAction {
        action: SessionManagerAction { kind, session_id },
        label: kind.label(),
        destructive,
        disabled,
    }
}

fn section_order(section: &str) -> u8 {
    match section {
        "Workspace Sessions" => 0,
        "Background Sessions" => 1,
        "Missing/Ended" => 2,
        _ => 3,
    }
}

pub fn end_confirmation(title: &str) -> String {
    format!("End {title} and every process running inside it?")
}

pub fn end_all_confirmation(count: usize) -> String {
    let (noun, pronoun) = if count == 1 {
        ("session", "it")
    } else {
        ("sessions", "them")
    };
    format!("End {count} active {noun} and every process running inside {pronoun}?")
}

pub fn remove_confirmation(title: &str) -> String {
    format!("Remove {title} from its workspace?")
}

pub fn picker(
    theme: muxy_ui::theme::Theme,
    metrics: muxy_ui::theme::Metrics,
    cx: &mut gpui::Context<CommandPopover>,
) -> CommandPopover {
    CommandPopover::new(
        CommandPopoverConfig {
            id: "terminal-session-manager".into(),
            presentation: CommandPopoverPresentation::Popover,
            density: CommandPopoverDensity::Comfortable,
            tabs: vec![CommandPopoverTab::new("sessions", "Sessions")],
            placeholder: "Search sessions…".into(),
            footer_actions: vec![
                CommandPopoverAction::new("end-all", "End All Sessions").destructive(true),
                CommandPopoverAction::new("terminal-settings", "Terminal Settings"),
            ],
            footer_hints: Vec::new(),
            width: Some(PANEL_WIDTH),
            height: Some(PANEL_HEIGHT),
            max_height: None,
            completion_on_tab: false,
            confirm_on_click: false,
        },
        theme,
        metrics,
        cx,
    )
}

pub fn sync_picker(
    picker: &gpui::Entity<CommandPopover>,
    sessions: Result<Vec<ManagedSession>, String>,
    query: &str,
    cx: &mut gpui::Context<crate::views::window::MainWindow>,
) -> Option<SessionManagerModel> {
    match sessions {
        Ok(sessions) => {
            let model = model(&sessions, query);
            let items = command_items(&model);
            let status = if items.is_empty() {
                CommandPopoverStatus::Empty("No terminal sessions".into())
            } else {
                CommandPopoverStatus::Ready
            };
            picker.update(cx, |picker, cx| {
                picker.set_items(items, cx);
                picker.set_status(status, cx);
                picker.set_header_detail(Some(model.accessibility_summary.clone()), cx);
                picker.set_footer_actions(
                    vec![
                        CommandPopoverAction::new("end-all", "End All Sessions")
                            .destructive(true)
                            .disabled(model.active_count == 0),
                        CommandPopoverAction::new("terminal-settings", "Terminal Settings"),
                    ],
                    cx,
                );
            });
            Some(model)
        }
        Err(error) => {
            picker.update(cx, |picker, cx| {
                picker.set_items(Vec::new(), cx);
                picker.set_status(CommandPopoverStatus::Error(error.into()), cx);
                picker.set_footer_actions(
                    vec![CommandPopoverAction::new(
                        "terminal-settings",
                        "Terminal Settings",
                    )],
                    cx,
                );
            });
            None
        }
    }
}

fn command_items(model: &SessionManagerModel) -> Vec<CommandPopoverItem> {
    let mut items = Vec::new();
    let mut section = None;
    for row in &model.rows {
        if section != Some(row.section) {
            section = Some(row.section);
            items.push(CommandPopoverItem::section(row.section));
        }
        let mut item =
            CommandPopoverRow::new(format!("session:{}", row.session_id), row.title.clone());
        item.subtitle = Some(row.subtitle.clone().into());
        item.accessibility_label = Some(row.accessibility_label.clone().into());
        item.trailing = Some(row.state.into());
        item.actions = row
            .actions
            .iter()
            .map(|action| {
                CommandPopoverAction::new(action.action.id(), action.label)
                    .destructive(action.destructive)
                    .disabled(action.disabled)
            })
            .collect();
        items.push(CommandPopoverItem::Row(item));
    }
    items
}

pub fn execute(
    action: &SessionManagerAction,
    session: &ManagedSession,
    coordinator: &mut SessionCoordinator,
    store: &mut WorkspaceStore,
) -> Result<Option<WorkspacePlacement>, String> {
    if action.session_id != session.session_id {
        return Err("session manager selection changed".to_owned());
    }
    match action.kind {
        SessionManagerActionKind::Focus | SessionManagerActionKind::Reattach => {
            let outcome = coordinator.reattach(session.session_id, store)?;
            let placement = match outcome {
                SessionReattachOutcome::Focused(placement)
                | SessionReattachOutcome::Reattached(placement) => placement,
            };
            Ok(Some(placement))
        }
        SessionManagerActionKind::End => {
            coordinator.end_session(session.session_id, &session.owner, store)?;
            Ok(None)
        }
        SessionManagerActionKind::StartNew => {
            let tab_id = session
                .placement
                .as_ref()
                .map(|placement| placement.tab_id.as_str())
                .ok_or_else(|| "workspace session placement is unavailable".to_owned())?;
            coordinator.start_new_terminal(tab_id, session.session_id, store)?;
            Ok(None)
        }
        SessionManagerActionKind::Remove => {
            let tab_id = session
                .placement
                .as_ref()
                .map(|placement| placement.tab_id.as_str())
                .ok_or_else(|| "workspace session placement is unavailable".to_owned())?;
            coordinator.remove_stale_session(tab_id, session.session_id, &session.owner, store)?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_proto::session::{ProcessIdentity, SessionOwner, WorkspacePlacement};

    fn managed(value: &str, state: ManagedSessionState) -> ManagedSession {
        let session_id = SessionId::parse(value).unwrap();
        ManagedSession {
            session_id,
            owner: SessionOwner {
                project_id: "PROJECT".into(),
                worktree_id: "WORKTREE".into(),
                original_tab_id: "TAB".into(),
            },
            placement: Some(WorkspacePlacement {
                project_id: "PROJECT".into(),
                worktree_id: "WORKTREE".into(),
                tab_id: "TAB".into(),
                area_id: "AREA".into(),
            }),
            shell: Some(ProcessIdentity {
                process_id: 10,
                start_identity: 20,
            }),
            title: "Shell".into(),
            working_directory: "/tmp/project".into(),
            state,
        }
    }

    #[test]
    fn session_manager_sections_actions_and_attached_semantics_are_deterministic() {
        let sessions = vec![
            managed(
                "123E4567-E89B-12D3-A456-426614174000",
                ManagedSessionState::Workspace,
            ),
            managed(
                "223E4567-E89B-12D3-A456-426614174000",
                ManagedSessionState::Background,
            ),
            managed(
                "323E4567-E89B-12D3-A456-426614174000",
                ManagedSessionState::Ended,
            ),
        ];
        let model = model(&sessions, "");
        assert_eq!(model.active_count, 2);
        assert_eq!(
            model.rows.iter().map(|row| row.section).collect::<Vec<_>>(),
            ["Workspace Sessions", "Background Sessions", "Missing/Ended"]
        );
        assert_eq!(
            model.rows[0]
                .actions
                .iter()
                .map(|action| action.label)
                .collect::<Vec<_>>(),
            ["Focus", "End"]
        );
        assert_eq!(
            model.rows[1]
                .actions
                .iter()
                .map(|action| action.label)
                .collect::<Vec<_>>(),
            ["Reattach", "End"]
        );
        assert_eq!(
            model.rows[2]
                .actions
                .iter()
                .map(|action| action.label)
                .collect::<Vec<_>>(),
            ["Start New", "Remove"]
        );
    }

    #[test]
    fn session_manager_owner_invalidation_disables_reattach_and_labels_accessibly() {
        let sessions = vec![managed(
            "423E4567-E89B-12D3-A456-426614174000",
            ManagedSessionState::AttachmentFailed,
        )];
        let model = model(&sessions, "");
        assert_eq!(model.rows[0].state, "Owner Changed");
        assert_eq!(model.rows[0].actions[0].label, "Reattach");
        assert!(model.rows[0].actions[0].disabled);
        assert_eq!(model.rows[0].actions[1].label, "End");
        assert!(
            model.rows[0]
                .accessibility_label
                .contains("Reattach unavailable, End")
        );
        assert_eq!(
            model.accessibility_summary,
            "Terminal Sessions, 1 total, 1 active"
        );
        let item = command_items(&model).pop().unwrap().row_data().unwrap();
        assert_eq!(
            item.accessibility_label
                .as_ref()
                .map(|label| label.as_ref()),
            Some(model.rows[0].accessibility_label.as_str())
        );
    }

    #[test]
    fn session_manager_confirmations_and_action_ids_are_exact() {
        assert_eq!(
            end_confirmation("Shell"),
            "End Shell and every process running inside it?"
        );
        assert_eq!(
            end_all_confirmation(1),
            "End 1 active session and every process running inside it?"
        );
        assert_eq!(
            remove_confirmation("Shell"),
            "Remove Shell from its workspace?"
        );
        assert_eq!(
            end_all_confirmation(3),
            "End 3 active sessions and every process running inside them?"
        );
        let action = SessionManagerAction {
            kind: SessionManagerActionKind::Remove,
            session_id: SessionId::parse("523E4567-E89B-12D3-A456-426614174000").unwrap(),
        };
        assert_eq!(SessionManagerAction::parse(&action.id()), Some(action));
    }
}
