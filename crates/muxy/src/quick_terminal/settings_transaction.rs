use super::QuickTerminalApplicationService;
use muxy_core::prefs::settings::{SettingsError, SettingsProposal};
use muxy_core::quick_terminal::{ConflictCandidate, QuickTerminalShortcut};
use muxy_core::shortcuts::{KeyCombo, ShortcutMap, modelled_actions};
use muxy_core::store::CommandShortcuts;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuickTerminalSettingsError {
    #[error("Quick Terminal shortcut conflicts with {0}")]
    Conflict(String),
    #[error("{0}")]
    Shortcut(String),
    #[error("{0}")]
    Persistence(String),
}

pub fn conflict_candidates(
    shortcuts: &ShortcutMap,
    commands: &CommandShortcuts,
) -> Vec<ConflictCandidate> {
    let mut candidates: Vec<ConflictCandidate> = modelled_actions()
        .into_iter()
        .filter_map(|action| {
            let combo = shortcuts.combo(action).clone();
            combo.is_assigned().then(|| ConflictCandidate {
                label: action.display_name().to_owned(),
                combo,
            })
        })
        .collect();
    if commands.prefix_combo.is_assigned() {
        candidates.push(ConflictCandidate {
            label: "Command Prefix".to_owned(),
            combo: commands.prefix_combo.clone(),
        });
    }
    candidates.extend(
        commands
            .shortcuts
            .iter()
            .filter(|command| command.combo.is_assigned())
            .map(|command| ConflictCandidate {
                label: command.display_name(),
                combo: command.combo.clone(),
            }),
    );
    candidates
}

pub fn validate_quick_terminal_candidate(
    shortcut: &QuickTerminalShortcut,
    candidates: &[ConflictCandidate],
    key_resolver: impl FnMut(u16) -> Option<String>,
) -> Result<(), QuickTerminalSettingsError> {
    match shortcut.find_conflict(candidates, key_resolver) {
        Some(conflict) => Err(QuickTerminalSettingsError::Conflict(conflict.label)),
        None => Ok(()),
    }
}

pub fn validate_reverse_conflict(
    combo: &KeyCombo,
    shortcut: &QuickTerminalShortcut,
    key_resolver: impl FnMut(u16) -> Option<String>,
) -> Result<(), QuickTerminalSettingsError> {
    let Some(shortcut) = shortcut.canonicalized(key_resolver) else {
        return Ok(());
    };
    if shortcut
        .key_combo()
        .is_some_and(|active| active.conflicts_with(combo))
    {
        Err(QuickTerminalSettingsError::Conflict(
            "Quick Terminal".to_owned(),
        ))
    } else {
        Ok(())
    }
}

pub fn validate_proposal(
    proposal: &SettingsProposal,
    current_shortcuts: &ShortcutMap,
    current_commands: &CommandShortcuts,
    current_quick_terminal: &QuickTerminalShortcut,
    key_resolver: impl FnMut(u16) -> Option<String>,
) -> Result<(), QuickTerminalSettingsError> {
    let shortcuts = proposal.app_shortcuts.as_ref().unwrap_or(current_shortcuts);
    let commands = proposal
        .custom_commands
        .as_ref()
        .unwrap_or(current_commands);
    let quick_terminal = proposal
        .quick_terminal_shortcut
        .as_ref()
        .unwrap_or(current_quick_terminal);
    validate_quick_terminal_candidate(
        quick_terminal,
        &conflict_candidates(shortcuts, commands),
        key_resolver,
    )
}

pub fn apply_proposal(
    service: &mut QuickTerminalApplicationService,
    proposal: SettingsProposal,
    current_shortcuts: &ShortcutMap,
    current_commands: &CommandShortcuts,
    enabled: bool,
) -> Result<(), QuickTerminalSettingsError> {
    apply_proposal_with(
        service,
        proposal,
        current_shortcuts,
        current_commands,
        enabled,
        muxy_core::prefs::settings::commit_proposal,
    )
}

fn apply_proposal_with(
    service: &mut QuickTerminalApplicationService,
    proposal: SettingsProposal,
    current_shortcuts: &ShortcutMap,
    current_commands: &CommandShortcuts,
    enabled: bool,
    commit: impl FnOnce(SettingsProposal) -> Result<(), SettingsError>,
) -> Result<(), QuickTerminalSettingsError> {
    validate_proposal(
        &proposal,
        current_shortcuts,
        current_commands,
        service.shortcut(),
        super::platform::resolve_key,
    )?;
    let shortcut = proposal
        .quick_terminal_shortcut
        .clone()
        .unwrap_or_else(|| service.shortcut().clone());
    let prepared = service
        .prepare_shortcut_for_enabled(
            shortcut,
            &conflict_candidates(
                proposal.app_shortcuts.as_ref().unwrap_or(current_shortcuts),
                proposal
                    .custom_commands
                    .as_ref()
                    .unwrap_or(current_commands),
            ),
            enabled,
        )
        .map_err(|error| QuickTerminalSettingsError::Shortcut(error.to_string()))?;
    if let Err(error) = commit(proposal) {
        service.cancel_shortcut(prepared);
        return Err(settings_error(error));
    }
    service.commit_shortcut(prepared);
    Ok(())
}

fn settings_error(error: SettingsError) -> QuickTerminalSettingsError {
    QuickTerminalSettingsError::Persistence(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quick_terminal::shortcut_service::{
        MonitoringState, QuickTerminalShortcutService, ShortcutBackend, ShortcutBackendFactory,
    };
    use muxy_core::shortcuts::{COMMAND, CONTROL, OPTION};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    struct TransactionBackend {
        stops: Rc<Cell<usize>>,
    }

    impl ShortcutBackend for TransactionBackend {
        fn start(&mut self, _: Rc<dyn Fn()>) -> Result<(), String> {
            Ok(())
        }

        fn stop(&mut self) {
            self.stops.set(self.stops.get() + 1);
        }

        fn monitoring_state(&self) -> MonitoringState {
            MonitoringState::CarbonHotKey
        }
    }

    struct TransactionFactory {
        stops: Rc<RefCell<Vec<Rc<Cell<usize>>>>>,
    }

    impl ShortcutBackendFactory for TransactionFactory {
        fn create(&mut self, _: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>> {
            let stops = Rc::new(Cell::new(0));
            self.stops.borrow_mut().push(stops.clone());
            Some(Box::new(TransactionBackend { stops }))
        }

        fn request_input_monitoring_access(&mut self) -> bool {
            false
        }
    }

    fn resolver(code: u16) -> Option<String> {
        (code == 49).then(|| "space".to_owned())
    }

    fn shortcut() -> QuickTerminalShortcut {
        QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("space", COMMAND | CONTROL),
            virtual_key_code: 49,
        }
    }

    #[test]
    fn quick_terminal_settings_candidate_conflicts_name_the_owner() {
        let candidates = [ConflictCandidate {
            label: "Open Project".to_owned(),
            combo: KeyCombo::new("space", COMMAND | CONTROL),
        }];
        assert_eq!(
            validate_quick_terminal_candidate(&shortcut(), &candidates, resolver),
            Err(QuickTerminalSettingsError::Conflict(
                "Open Project".to_owned()
            ))
        );
    }

    #[test]
    fn quick_terminal_settings_reverse_conflicts_preserve_the_active_shortcut() {
        assert_eq!(
            validate_reverse_conflict(
                &KeyCombo::new("space", COMMAND | CONTROL),
                &shortcut(),
                resolver,
            ),
            Err(QuickTerminalSettingsError::Conflict(
                "Quick Terminal".to_owned()
            ))
        );
        assert!(
            validate_reverse_conflict(&KeyCombo::new("x", COMMAND), &shortcut(), resolver).is_ok()
        );
    }

    #[test]
    fn quick_terminal_settings_conflict_candidates_include_prefix_and_commands() {
        let mut shortcuts = ShortcutMap::from_mirror_object(&serde_json::Map::from_iter([(
            "openProject".to_owned(),
            serde_json::json!({ "key": "o", "modifiers": COMMAND }),
        )]))
        .unwrap();
        for action in modelled_actions() {
            if action.display_name() != "Open Project" {
                shortcuts.set(action, KeyCombo::new("", 0));
            }
        }
        let commands = CommandShortcuts::from_mirror_value(&serde_json::json!({
            "prefixCombo": { "key": "g", "modifiers": COMMAND },
            "shortcuts": [{
                "id": "build",
                "name": "Build",
                "command": "cargo build",
                "combo": { "key": "b", "modifiers": COMMAND }
            }]
        }))
        .unwrap();
        let candidates = conflict_candidates(&shortcuts, &commands);
        assert!(candidates.iter().any(|entry| entry.label == "Open Project"));
        assert!(
            candidates
                .iter()
                .any(|entry| entry.label == "Command Prefix")
        );
        assert!(candidates.iter().any(|entry| entry.label == "Build"));
    }

    #[test]
    fn quick_terminal_settings_commit_failure_stops_only_the_prepared_backend() {
        let initial = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("a", COMMAND | CONTROL | OPTION),
            virtual_key_code: 0,
        };
        let stops = Rc::new(RefCell::new(Vec::new()));
        let mut service =
            QuickTerminalApplicationService::from_service(QuickTerminalShortcutService::new(
                initial.clone(),
                true,
                Box::new(TransactionFactory {
                    stops: stops.clone(),
                }),
                Box::new(|_| Ok(())),
                Box::new(|code| (code == 0).then(|| "a".to_owned())),
            ));
        service.start().unwrap();
        let proposal = SettingsProposal {
            document: serde_json::Map::new(),
            settings: Vec::new(),
            app_shortcuts: None,
            custom_commands: None,
            quick_terminal_shortcut: Some(QuickTerminalShortcut::DoubleShift),
        };
        let error = apply_proposal_with(
            &mut service,
            proposal,
            &ShortcutMap::load(),
            &CommandShortcuts::default(),
            true,
            |_| Err(SettingsError::Persistence("blocked".to_owned())),
        )
        .unwrap_err();
        assert_eq!(
            error,
            QuickTerminalSettingsError::Persistence(
                "Failed to persist settings: blocked".to_owned()
            )
        );
        assert_eq!(service.shortcut(), &initial);
        let stops = stops.borrow();
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[0].get(), 0);
        assert_eq!(stops[1].get(), 1);
    }
}
