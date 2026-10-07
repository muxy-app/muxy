use async_channel::{Receiver, Sender};
use muxy_core::quick_terminal::{ConflictCandidate, QuickTerminalShortcut};
use std::cell::Cell;
use std::rc::Rc;

use crate::tr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutState {
    Stopped,
    Unavailable,
    Registered,
}

pub trait ShortcutBackend {
    fn start(&mut self, trigger: Rc<dyn Fn()>) -> Result<(), String>;
    fn stop(&mut self);
    fn state(&self) -> ShortcutState;
}

pub trait ShortcutBackendFactory {
    fn create(&mut self, shortcut: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>>;
}

pub type ShortcutPersistence = dyn FnMut(&QuickTerminalShortcut) -> std::io::Result<()>;

enum PreparedBackend {
    Keep,
    Replace {
        backend: Option<Box<dyn ShortcutBackend>>,
        generation: Option<u64>,
    },
}

pub struct PreparedShortcutUpdate {
    shortcut: QuickTerminalShortcut,
    enabled: bool,
    persist: bool,
    backend: PreparedBackend,
}

#[derive(Debug)]
pub enum ShortcutServiceError {
    InvalidShortcut,
    Conflict(String),
    Backend(String),
    Persistence(std::io::Error),
}

pub struct QuickTerminalShortcutService {
    shortcut: QuickTerminalShortcut,
    enabled: bool,
    started: bool,
    state: ShortcutState,
    error_message: Option<String>,
    active_backend: Option<Box<dyn ShortcutBackend>>,
    generation: u64,
    active_generation: Rc<Cell<Option<u64>>>,
    trigger_count: Rc<Cell<u64>>,
    trigger_sender: Sender<()>,
    trigger_receiver: Receiver<()>,
    factory: Box<dyn ShortcutBackendFactory>,
    persist: Box<ShortcutPersistence>,
    key_resolver: Box<dyn FnMut(u16) -> Option<String>>,
}

impl QuickTerminalShortcutService {
    pub fn new(
        shortcut: QuickTerminalShortcut,
        enabled: bool,
        factory: Box<dyn ShortcutBackendFactory>,
        persist: Box<ShortcutPersistence>,
        key_resolver: Box<dyn FnMut(u16) -> Option<String>>,
    ) -> Self {
        let (trigger_sender, trigger_receiver) = async_channel::unbounded();
        Self {
            shortcut,
            enabled,
            started: false,
            state: ShortcutState::Stopped,
            error_message: None,
            active_backend: None,
            generation: 0,
            active_generation: Rc::new(Cell::new(None)),
            trigger_count: Rc::new(Cell::new(0)),
            trigger_sender,
            trigger_receiver,
            factory,
            persist,
            key_resolver,
        }
    }

    pub fn start(&mut self) -> Result<(), ShortcutServiceError> {
        self.started = true;
        if !self.enabled || self.active_backend.is_some() {
            return Ok(());
        }
        let Some(shortcut) = self.canonicalize(&self.shortcut.clone()) else {
            return self.fail(ShortcutServiceError::InvalidShortcut);
        };
        self.shortcut = shortcut.clone();
        let Some(mut backend) = self.factory.create(&shortcut) else {
            self.state = ShortcutState::Stopped;
            self.error_message = None;
            return Ok(());
        };
        let generation = self.next_generation();
        if let Err(error) = backend.start(self.trigger(generation)) {
            return self.fail(ShortcutServiceError::Backend(error));
        }
        self.active_generation.set(Some(generation));
        self.state = backend.state();
        self.active_backend = Some(backend);
        self.error_message = None;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.started = false;
        self.stop_active_backend();
        self.error_message = None;
    }

    pub fn update_shortcut(
        &mut self,
        shortcut: QuickTerminalShortcut,
        conflicts: &[ConflictCandidate],
    ) -> Result<(), ShortcutServiceError> {
        let prepared = self.prepare_shortcut(shortcut, conflicts)?;
        if prepared.persist
            && let Err(error) = (self.persist)(&prepared.shortcut)
        {
            self.cancel_prepared(prepared);
            return self.fail(ShortcutServiceError::Persistence(error));
        }
        self.commit_prepared(prepared);
        Ok(())
    }

    pub fn prepare_shortcut(
        &mut self,
        shortcut: QuickTerminalShortcut,
        conflicts: &[ConflictCandidate],
    ) -> Result<PreparedShortcutUpdate, ShortcutServiceError> {
        self.prepare_shortcut_for_enabled(shortcut, conflicts, self.enabled)
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "Preparation accepts an owned candidate before canonicalizing its physical key"
    )]
    pub fn prepare_shortcut_for_enabled(
        &mut self,
        shortcut: QuickTerminalShortcut,
        conflicts: &[ConflictCandidate],
        enabled: bool,
    ) -> Result<PreparedShortcutUpdate, ShortcutServiceError> {
        let Some(shortcut) = self.canonicalize(&shortcut) else {
            return self.fail(ShortcutServiceError::InvalidShortcut);
        };
        if let Some(conflict) = shortcut.find_conflict(conflicts, |code| (self.key_resolver)(code))
        {
            return self.fail(ShortcutServiceError::Conflict(conflict.label));
        }
        let persist = shortcut != self.shortcut;
        if !self.started {
            return Ok(PreparedShortcutUpdate {
                shortcut,
                enabled,
                persist,
                backend: PreparedBackend::Keep,
            });
        }
        if !enabled {
            return Ok(PreparedShortcutUpdate {
                shortcut,
                enabled,
                persist,
                backend: if self.active_backend.is_some() {
                    PreparedBackend::Replace {
                        backend: None,
                        generation: None,
                    }
                } else {
                    PreparedBackend::Keep
                },
            });
        }
        if self.active_backend.is_some()
            && (shortcut == self.shortcut || same_registration(&shortcut, &self.shortcut))
        {
            return Ok(PreparedShortcutUpdate {
                shortcut,
                enabled,
                persist,
                backend: PreparedBackend::Keep,
            });
        }
        let Some(mut backend) = self.factory.create(&shortcut) else {
            return Ok(PreparedShortcutUpdate {
                shortcut,
                enabled,
                persist,
                backend: PreparedBackend::Replace {
                    backend: None,
                    generation: None,
                },
            });
        };
        let generation = self.next_generation();
        if let Err(error) = backend.start(self.trigger(generation)) {
            return self.fail(ShortcutServiceError::Backend(error));
        }
        Ok(PreparedShortcutUpdate {
            shortcut,
            enabled,
            persist,
            backend: PreparedBackend::Replace {
                backend: Some(backend),
                generation: Some(generation),
            },
        })
    }

    pub fn commit_prepared(&mut self, prepared: PreparedShortcutUpdate) {
        self.shortcut = prepared.shortcut;
        self.enabled = prepared.enabled;
        match prepared.backend {
            PreparedBackend::Keep => {}
            PreparedBackend::Replace {
                mut backend,
                generation,
            } => {
                self.active_generation.set(generation);
                self.state = backend
                    .as_ref()
                    .map_or(ShortcutState::Stopped, |backend| backend.state());
                if let Some(mut previous) = self.active_backend.take() {
                    previous.stop();
                }
                self.active_backend = backend.take();
            }
        }
        self.error_message = None;
    }

    pub fn cancel_prepared(&mut self, prepared: PreparedShortcutUpdate) {
        if let PreparedBackend::Replace {
            backend: Some(mut backend),
            ..
        } = prepared.backend
        {
            backend.stop();
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<(), ShortcutServiceError> {
        if enabled == self.enabled {
            return Ok(());
        }
        self.enabled = enabled;
        if !enabled {
            self.stop_active_backend();
            self.error_message = None;
            return Ok(());
        }
        if self.started
            && let Err(error) = self.start()
        {
            self.enabled = false;
            return Err(error);
        }
        Ok(())
    }

    pub fn shortcut(&self) -> &QuickTerminalShortcut {
        &self.shortcut
    }

    pub fn state(&self) -> ShortcutState {
        self.state
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn trigger_count(&self) -> u64 {
        self.trigger_count.get()
    }

    pub fn try_receive_trigger(&self) -> bool {
        self.trigger_receiver.try_recv().is_ok()
    }

    pub fn trigger_receiver(&self) -> Receiver<()> {
        self.trigger_receiver.clone()
    }

    fn canonicalize(&mut self, shortcut: &QuickTerminalShortcut) -> Option<QuickTerminalShortcut> {
        shortcut.canonicalized(|code| (self.key_resolver)(code))
    }

    fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    fn trigger(&self, generation: u64) -> Rc<dyn Fn()> {
        let active_generation = self.active_generation.clone();
        let trigger_count = self.trigger_count.clone();
        let trigger_sender = self.trigger_sender.clone();
        Rc::new(move || {
            if active_generation.get() != Some(generation) {
                return;
            }
            trigger_count.set(trigger_count.get().wrapping_add(1));
            let _ = trigger_sender.try_send(());
        })
    }

    fn stop_active_backend(&mut self) {
        self.active_generation.set(None);
        if let Some(mut backend) = self.active_backend.take() {
            backend.stop();
        }
        self.state = ShortcutState::Stopped;
    }

    fn fail<T>(&mut self, error: ShortcutServiceError) -> Result<T, ShortcutServiceError> {
        self.error_message = Some(error.to_string());
        Err(error)
    }
}

fn same_registration(left: &QuickTerminalShortcut, right: &QuickTerminalShortcut) -> bool {
    match (left, right) {
        (QuickTerminalShortcut::Unassigned, QuickTerminalShortcut::Unassigned) => true,
        (QuickTerminalShortcut::KeyCombo { .. }, QuickTerminalShortcut::KeyCombo { .. }) => {
            left.registration_identity() == right.registration_identity()
        }
        _ => false,
    }
}

impl Drop for QuickTerminalShortcutService {
    fn drop(&mut self) {
        self.stop();
    }
}

impl std::fmt::Display for ShortcutServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidShortcut => f.write_str(&tr!("Invalid Quick Terminal shortcut")),
            Self::Conflict(label) => {
                f.write_str(&tr!("Quick Terminal shortcut conflicts with %@", label))
            }
            Self::Backend(error) => f.write_str(error),
            Self::Persistence(error) => f.write_str(&tr!(
                "Could not save Quick Terminal shortcut: %@",
                error.to_string()
            )),
        }
    }
}

impl std::fmt::Debug for PreparedShortcutUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedShortcutUpdate")
            .field("shortcut", &self.shortcut)
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for QuickTerminalShortcutService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuickTerminalShortcutService")
            .field("shortcut", &self.shortcut)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{
        QuickTerminalShortcutService, ShortcutBackend, ShortcutBackendFactory,
        ShortcutServiceError, ShortcutState,
    };
    use muxy_core::quick_terminal::QuickTerminalShortcut;
    use muxy_core::quick_terminal::keys::{COMMAND, CONTROL, KeyCombo};
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::io;
    use std::rc::Rc;

    #[derive(Default)]
    struct BackendRecord {
        starts: usize,
        stops: usize,
        trigger: Option<Rc<dyn Fn()>>,
    }

    struct TestBackend {
        state: ShortcutState,
        start_error: Option<&'static str>,
        record: Rc<RefCell<BackendRecord>>,
    }

    impl ShortcutBackend for TestBackend {
        fn start(&mut self, trigger: Rc<dyn Fn()>) -> Result<(), String> {
            let mut record = self.record.borrow_mut();
            record.starts += 1;
            if let Some(error) = self.start_error {
                return Err(error.to_owned());
            }
            record.trigger = Some(trigger);
            self.state = ShortcutState::Registered;
            Ok(())
        }

        fn stop(&mut self) {
            let mut record = self.record.borrow_mut();
            record.stops += 1;
            self.state = ShortcutState::Stopped;
        }

        fn state(&self) -> ShortcutState {
            self.state
        }
    }

    struct TestFactory {
        backends: VecDeque<TestBackend>,
    }

    impl ShortcutBackendFactory for TestFactory {
        fn create(&mut self, shortcut: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>> {
            match shortcut {
                QuickTerminalShortcut::Unassigned => None,
                QuickTerminalShortcut::KeyCombo { .. } => self
                    .backends
                    .pop_front()
                    .map(|backend| Box::new(backend) as Box<dyn ShortcutBackend>),
            }
        }
    }

    fn backend(start_error: Option<&'static str>) -> (TestBackend, Rc<RefCell<BackendRecord>>) {
        let record = Rc::new(RefCell::new(BackendRecord::default()));
        (
            TestBackend {
                state: ShortcutState::Stopped,
                start_error,
                record: record.clone(),
            },
            record,
        )
    }

    fn key_combo() -> QuickTerminalShortcut {
        QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("space", COMMAND),
            virtual_key_code: 49,
        }
    }

    fn other_key_combo() -> QuickTerminalShortcut {
        QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("space", CONTROL),
            virtual_key_code: 49,
        }
    }

    fn service(
        shortcut: QuickTerminalShortcut,
        enabled: bool,
        backends: Vec<TestBackend>,
        persist: impl FnMut(&QuickTerminalShortcut) -> io::Result<()> + 'static,
    ) -> QuickTerminalShortcutService {
        QuickTerminalShortcutService::new(
            shortcut,
            enabled,
            Box::new(TestFactory {
                backends: backends.into(),
            }),
            Box::new(persist),
            Box::new(|code| match code {
                0 => Some("a".to_owned()),
                49 => Some("space".to_owned()),
                _ => None,
            }),
        )
    }

    #[test]
    fn quick_terminal_shortcut_start_stop_and_stale_generation() {
        let (first, first_record) = backend(None);
        let (second, second_record) = backend(None);
        let mut service = service(other_key_combo(), true, vec![first, second], |_| Ok(()));
        service.start().unwrap();
        first_record.borrow().trigger.as_ref().unwrap()();
        assert_eq!(service.trigger_count(), 1);
        service.update_shortcut(key_combo(), &[]).unwrap();
        first_record.borrow().trigger.as_ref().unwrap()();
        assert_eq!(service.trigger_count(), 1);
        second_record.borrow().trigger.as_ref().unwrap()();
        assert_eq!(service.trigger_count(), 2);
        service.stop();
        assert_eq!(second_record.borrow().stops, 1);
        assert_eq!(service.state(), ShortcutState::Stopped);
    }

    #[test]
    fn quick_terminal_shortcut_registration_failure_keeps_previous() {
        let (first, first_record) = backend(None);
        let (second, second_record) = backend(Some("registration failed"));
        let saves = Rc::new(Cell::new(0));
        let observed_saves = saves.clone();
        let mut service = service(other_key_combo(), true, vec![first, second], move |_| {
            observed_saves.set(observed_saves.get() + 1);
            Ok(())
        });
        service.start().unwrap();
        assert!(matches!(
            service.update_shortcut(key_combo(), &[]),
            Err(ShortcutServiceError::Backend(_))
        ));
        assert_eq!(first_record.borrow().stops, 0);
        assert_eq!(second_record.borrow().starts, 1);
        assert_eq!(saves.get(), 0);
        assert_eq!(service.shortcut(), &other_key_combo());
        assert_eq!(service.state(), ShortcutState::Registered);
    }

    #[test]
    fn quick_terminal_shortcut_persistence_failure_rolls_back_candidate() {
        let (first, first_record) = backend(None);
        let (second, second_record) = backend(None);
        let mut service = service(other_key_combo(), true, vec![first, second], |_| {
            Err(io::Error::other("disk full"))
        });
        service.start().unwrap();
        assert!(matches!(
            service.update_shortcut(key_combo(), &[]),
            Err(ShortcutServiceError::Persistence(_))
        ));
        assert_eq!(first_record.borrow().stops, 0);
        assert_eq!(second_record.borrow().stops, 1);
        assert_eq!(service.shortcut(), &other_key_combo());
        assert_eq!(service.state(), ShortcutState::Registered);
    }
}
