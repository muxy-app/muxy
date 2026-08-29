use async_channel::{Receiver, Sender};
use muxy_core::quick_terminal::{ConflictCandidate, QuickTerminalShortcut};
use std::cell::Cell;
use std::rc::Rc;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonitoringState {
    Stopped,
    Unavailable,
    LocalOnly,
    SystemWide,
    CarbonHotKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventTapRecovery {
    Reenable,
    Downgrade,
}

pub fn event_tap_recovery(authorized: bool, reenable_succeeded: bool) -> EventTapRecovery {
    if authorized && reenable_succeeded {
        EventTapRecovery::Reenable
    } else {
        EventTapRecovery::Downgrade
    }
}

pub trait ShortcutBackend {
    fn start(&mut self, trigger: Rc<dyn Fn()>) -> Result<(), String>;
    fn stop(&mut self);
    fn monitoring_state(&self) -> MonitoringState;
    fn refresh_system_wide_monitoring(&mut self) -> bool {
        false
    }
}

pub trait ShortcutBackendFactory {
    fn create(&mut self, shortcut: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>>;
    fn request_input_monitoring_access(&mut self) -> bool;
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

#[derive(Debug, Error)]
pub enum ShortcutServiceError {
    #[error("invalid Quick Terminal shortcut")]
    InvalidShortcut,
    #[error("Quick Terminal shortcut conflicts with {0}")]
    Conflict(String),
    #[error("{0}")]
    Backend(String),
    #[error("failed to persist Quick Terminal shortcut: {0}")]
    Persistence(#[source] std::io::Error),
}

pub struct QuickTerminalShortcutService {
    shortcut: QuickTerminalShortcut,
    enabled: bool,
    monitoring_requested: bool,
    monitoring_state: MonitoringState,
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
            monitoring_requested: false,
            monitoring_state: MonitoringState::Stopped,
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
        self.monitoring_requested = true;
        if !self.enabled || self.active_backend.is_some() {
            return Ok(());
        }
        let Some(shortcut) = self.canonicalize(self.shortcut.clone()) else {
            return self.fail(ShortcutServiceError::InvalidShortcut);
        };
        self.shortcut = shortcut.clone();
        let Some(mut backend) = self.factory.create(&shortcut) else {
            self.monitoring_state = MonitoringState::Stopped;
            self.error_message = None;
            return Ok(());
        };
        let generation = self.next_generation();
        if let Err(error) = backend.start(self.trigger(generation)) {
            return self.fail(ShortcutServiceError::Backend(error));
        }
        self.active_generation.set(Some(generation));
        self.monitoring_state = backend.monitoring_state();
        self.active_backend = Some(backend);
        self.error_message = None;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.monitoring_requested = false;
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

    pub fn prepare_shortcut_for_enabled(
        &mut self,
        shortcut: QuickTerminalShortcut,
        conflicts: &[ConflictCandidate],
        enabled: bool,
    ) -> Result<PreparedShortcutUpdate, ShortcutServiceError> {
        let Some(shortcut) = self.canonicalize(shortcut) else {
            return self.fail(ShortcutServiceError::InvalidShortcut);
        };
        if let Some(conflict) = shortcut.find_conflict(conflicts, |code| (self.key_resolver)(code))
        {
            return self.fail(ShortcutServiceError::Conflict(conflict.label));
        }
        let persist = shortcut != self.shortcut;
        if !self.monitoring_requested {
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
                self.monitoring_state = backend
                    .as_ref()
                    .map_or(MonitoringState::Stopped, |backend| {
                        backend.monitoring_state()
                    });
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
        if self.monitoring_requested
            && let Err(error) = self.start()
        {
            self.enabled = false;
            return Err(error);
        }
        Ok(())
    }

    pub fn request_input_monitoring_access(&mut self) -> bool {
        if !self.monitoring_requested
            || !self.enabled
            || !matches!(self.shortcut, QuickTerminalShortcut::DoubleShift)
        {
            return false;
        }
        if !self.factory.request_input_monitoring_access() {
            return false;
        }
        self.refresh_input_monitoring_access()
    }

    pub fn refresh_input_monitoring_access(&mut self) -> bool {
        if !self.monitoring_requested
            || !self.enabled
            || !matches!(self.shortcut, QuickTerminalShortcut::DoubleShift)
        {
            return false;
        }
        let Some(backend) = self.active_backend.as_mut() else {
            return false;
        };
        let enabled = backend.refresh_system_wide_monitoring();
        self.monitoring_state = backend.monitoring_state();
        if enabled {
            self.error_message = None;
        }
        enabled
    }

    pub fn shortcut(&self) -> &QuickTerminalShortcut {
        &self.shortcut
    }

    pub fn monitoring_state(&self) -> MonitoringState {
        self.monitoring_state
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

    fn canonicalize(&mut self, shortcut: QuickTerminalShortcut) -> Option<QuickTerminalShortcut> {
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
        self.monitoring_state = MonitoringState::Stopped;
    }

    fn fail<T>(&mut self, error: ShortcutServiceError) -> Result<T, ShortcutServiceError> {
        self.error_message = Some(error.to_string());
        Err(error)
    }
}

fn same_registration(left: &QuickTerminalShortcut, right: &QuickTerminalShortcut) -> bool {
    match (left, right) {
        (QuickTerminalShortcut::Unassigned, QuickTerminalShortcut::Unassigned)
        | (QuickTerminalShortcut::DoubleShift, QuickTerminalShortcut::DoubleShift) => true,
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

#[cfg(test)]
mod tests {
    use super::{
        EventTapRecovery, MonitoringState, QuickTerminalShortcutService, ShortcutBackend,
        ShortcutBackendFactory, ShortcutServiceError, event_tap_recovery,
    };
    use muxy_core::quick_terminal::{ConflictCandidate, QuickTerminalShortcut};
    use muxy_core::shortcuts::{COMMAND, KeyCombo};
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::io;
    use std::rc::Rc;

    #[derive(Default)]
    struct BackendRecord {
        starts: usize,
        stops: usize,
        refreshes: usize,
        trigger: Option<Rc<dyn Fn()>>,
        events: Vec<String>,
    }

    struct TestBackend {
        name: &'static str,
        state: MonitoringState,
        refreshed_state: MonitoringState,
        start_error: Option<&'static str>,
        record: Rc<RefCell<BackendRecord>>,
    }

    impl ShortcutBackend for TestBackend {
        fn start(&mut self, trigger: Rc<dyn Fn()>) -> Result<(), String> {
            let mut record = self.record.borrow_mut();
            record.starts += 1;
            record.events.push(format!("start {}", self.name));
            if let Some(error) = self.start_error {
                return Err(error.to_owned());
            }
            record.trigger = Some(trigger);
            Ok(())
        }

        fn stop(&mut self) {
            let mut record = self.record.borrow_mut();
            record.stops += 1;
            record.events.push(format!("stop {}", self.name));
            self.state = MonitoringState::Stopped;
        }

        fn monitoring_state(&self) -> MonitoringState {
            self.state
        }

        fn refresh_system_wide_monitoring(&mut self) -> bool {
            self.record.borrow_mut().refreshes += 1;
            self.state = self.refreshed_state;
            self.state == MonitoringState::SystemWide
        }
    }

    struct TestFactory {
        double_shift: VecDeque<TestBackend>,
        carbon: VecDeque<TestBackend>,
        requests: Rc<Cell<usize>>,
        grant: bool,
    }

    impl ShortcutBackendFactory for TestFactory {
        fn create(&mut self, shortcut: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>> {
            match shortcut {
                QuickTerminalShortcut::Unassigned => None,
                QuickTerminalShortcut::DoubleShift => self
                    .double_shift
                    .pop_front()
                    .map(|backend| Box::new(backend) as Box<dyn ShortcutBackend>),
                QuickTerminalShortcut::KeyCombo { .. } => self
                    .carbon
                    .pop_front()
                    .map(|backend| Box::new(backend) as Box<dyn ShortcutBackend>),
            }
        }

        fn request_input_monitoring_access(&mut self) -> bool {
            self.requests.set(self.requests.get() + 1);
            self.grant
        }
    }

    fn backend(
        name: &'static str,
        state: MonitoringState,
        refreshed_state: MonitoringState,
        start_error: Option<&'static str>,
    ) -> (TestBackend, Rc<RefCell<BackendRecord>>) {
        let record = Rc::new(RefCell::new(BackendRecord::default()));
        (
            TestBackend {
                name,
                state,
                refreshed_state,
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

    fn service(
        shortcut: QuickTerminalShortcut,
        enabled: bool,
        double_shift: Vec<TestBackend>,
        carbon: Vec<TestBackend>,
        grant: bool,
        persist: impl FnMut(&QuickTerminalShortcut) -> io::Result<()> + 'static,
    ) -> (QuickTerminalShortcutService, Rc<Cell<usize>>) {
        let requests = Rc::new(Cell::new(0));
        let factory = TestFactory {
            double_shift: double_shift.into(),
            carbon: carbon.into(),
            requests: requests.clone(),
            grant,
        };
        (
            QuickTerminalShortcutService::new(
                shortcut,
                enabled,
                Box::new(factory),
                Box::new(persist),
                Box::new(|code| match code {
                    0 => Some("a".to_owned()),
                    49 => Some("space".to_owned()),
                    _ => None,
                }),
            ),
            requests,
        )
    }

    #[test]
    fn quick_terminal_shortcut_start_stop_and_stale_generation() {
        let (first, first_record) = backend(
            "first",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (second, second_record) = backend(
            "second",
            MonitoringState::CarbonHotKey,
            MonitoringState::CarbonHotKey,
            None,
        );
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            true,
            vec![first],
            vec![second],
            false,
            |_| Ok(()),
        );
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
        assert_eq!(service.monitoring_state(), MonitoringState::Stopped);
    }

    #[test]
    fn quick_terminal_shortcut_replacement_is_transactional() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let (first, first_record) = backend(
            "first",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (second, second_record) = backend(
            "second",
            MonitoringState::CarbonHotKey,
            MonitoringState::CarbonHotKey,
            None,
        );
        let persisted = events.clone();
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            true,
            vec![first],
            vec![second],
            false,
            move |_| {
                persisted.borrow_mut().push("persist".to_owned());
                Ok(())
            },
        );
        service.start().unwrap();
        service.update_shortcut(key_combo(), &[]).unwrap();
        assert_eq!(first_record.borrow().events, ["start first", "stop first"]);
        assert_eq!(second_record.borrow().events, ["start second"]);
        assert_eq!(events.borrow().as_slice(), ["persist"]);
        assert_eq!(service.shortcut(), &key_combo());
    }

    #[test]
    fn quick_terminal_shortcut_registration_failure_keeps_previous() {
        let (first, first_record) = backend(
            "first",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (second, second_record) = backend(
            "second",
            MonitoringState::CarbonHotKey,
            MonitoringState::CarbonHotKey,
            Some("registration failed"),
        );
        let saves = Rc::new(Cell::new(0));
        let observed_saves = saves.clone();
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            true,
            vec![first],
            vec![second],
            false,
            move |_| {
                observed_saves.set(observed_saves.get() + 1);
                Ok(())
            },
        );
        service.start().unwrap();
        assert!(matches!(
            service.update_shortcut(key_combo(), &[]),
            Err(ShortcutServiceError::Backend(_))
        ));
        assert_eq!(first_record.borrow().stops, 0);
        assert_eq!(second_record.borrow().starts, 1);
        assert_eq!(saves.get(), 0);
        assert_eq!(service.shortcut(), &QuickTerminalShortcut::DoubleShift);
        assert_eq!(service.monitoring_state(), MonitoringState::LocalOnly);
    }

    #[test]
    fn quick_terminal_shortcut_persistence_failure_rolls_back_candidate() {
        let (first, first_record) = backend(
            "first",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (second, second_record) = backend(
            "second",
            MonitoringState::CarbonHotKey,
            MonitoringState::CarbonHotKey,
            None,
        );
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            true,
            vec![first],
            vec![second],
            false,
            |_| Err(io::Error::other("disk full")),
        );
        service.start().unwrap();
        assert!(matches!(
            service.update_shortcut(key_combo(), &[]),
            Err(ShortcutServiceError::Persistence(_))
        ));
        assert_eq!(first_record.borrow().stops, 0);
        assert_eq!(second_record.borrow().stops, 1);
        assert_eq!(service.shortcut(), &QuickTerminalShortcut::DoubleShift);
        assert_eq!(service.monitoring_state(), MonitoringState::LocalOnly);
    }

    #[test]
    fn quick_terminal_shortcut_unassignment_persists_before_stop() {
        let ordering = Rc::new(RefCell::new(Vec::new()));
        let record = Rc::new(RefCell::new(BackendRecord::default()));
        struct OrderedBackend {
            ordering: Rc<RefCell<Vec<&'static str>>>,
            record: Rc<RefCell<BackendRecord>>,
        }
        impl ShortcutBackend for OrderedBackend {
            fn start(&mut self, trigger: Rc<dyn Fn()>) -> Result<(), String> {
                self.record.borrow_mut().trigger = Some(trigger);
                Ok(())
            }
            fn stop(&mut self) {
                self.ordering.borrow_mut().push("stop");
            }
            fn monitoring_state(&self) -> MonitoringState {
                MonitoringState::LocalOnly
            }
        }
        let requests = Rc::new(Cell::new(0));
        struct OrderedFactory {
            backend: Option<OrderedBackend>,
            requests: Rc<Cell<usize>>,
        }
        impl ShortcutBackendFactory for OrderedFactory {
            fn create(
                &mut self,
                shortcut: &QuickTerminalShortcut,
            ) -> Option<Box<dyn ShortcutBackend>> {
                matches!(shortcut, QuickTerminalShortcut::DoubleShift)
                    .then(|| Box::new(self.backend.take().unwrap()) as Box<dyn ShortcutBackend>)
            }
            fn request_input_monitoring_access(&mut self) -> bool {
                self.requests.set(self.requests.get() + 1);
                false
            }
        }
        let persisted = ordering.clone();
        let mut service = QuickTerminalShortcutService::new(
            QuickTerminalShortcut::DoubleShift,
            true,
            Box::new(OrderedFactory {
                backend: Some(OrderedBackend {
                    ordering: ordering.clone(),
                    record,
                }),
                requests,
            }),
            Box::new(move |_| {
                persisted.borrow_mut().push("persist");
                Ok(())
            }),
            Box::new(|_| Some("space".to_owned())),
        );
        service.start().unwrap();
        service
            .update_shortcut(QuickTerminalShortcut::Unassigned, &[])
            .unwrap();
        assert_eq!(ordering.borrow().as_slice(), ["persist", "stop"]);
    }

    #[test]
    fn quick_terminal_shortcut_explicit_grant_and_passive_refresh_are_distinct() {
        let (first, first_record) = backend(
            "first",
            MonitoringState::LocalOnly,
            MonitoringState::SystemWide,
            None,
        );
        let (mut service, requests) = service(
            QuickTerminalShortcut::DoubleShift,
            true,
            vec![first],
            vec![],
            true,
            |_| Ok(()),
        );
        service.start().unwrap();
        assert!(service.refresh_input_monitoring_access());
        assert_eq!(requests.get(), 0);
        assert!(service.request_input_monitoring_access());
        assert_eq!(requests.get(), 1);
        assert_eq!(first_record.borrow().refreshes, 2);
        assert_eq!(service.monitoring_state(), MonitoringState::SystemWide);
    }

    #[test]
    fn quick_terminal_shortcut_revocation_downgrades_monitoring() {
        let (first, _) = backend(
            "first",
            MonitoringState::SystemWide,
            MonitoringState::LocalOnly,
            None,
        );
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            true,
            vec![first],
            vec![],
            false,
            |_| Ok(()),
        );
        service.start().unwrap();
        assert!(!service.refresh_input_monitoring_access());
        assert_eq!(service.monitoring_state(), MonitoringState::LocalOnly);
    }

    #[test]
    fn quick_terminal_shortcut_event_tap_reenables_or_downgrades() {
        assert_eq!(event_tap_recovery(true, true), EventTapRecovery::Reenable);
        assert_eq!(event_tap_recovery(true, false), EventTapRecovery::Downgrade);
        assert_eq!(event_tap_recovery(false, true), EventTapRecovery::Downgrade);
    }

    #[test]
    fn quick_terminal_shortcut_disabled_runtime_does_not_install_backend() {
        let (first, first_record) = backend(
            "first",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            false,
            vec![first],
            vec![],
            false,
            |_| Ok(()),
        );
        service.start().unwrap();
        assert_eq!(first_record.borrow().starts, 0);
        assert_eq!(service.monitoring_state(), MonitoringState::Stopped);
    }

    #[test]
    fn quick_terminal_shortcut_prepares_enable_before_runtime_publish() {
        let (candidate, record) = backend(
            "candidate",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            false,
            vec![candidate],
            vec![],
            false,
            |_| Ok(()),
        );
        service.start().unwrap();
        let prepared = service
            .prepare_shortcut_for_enabled(QuickTerminalShortcut::DoubleShift, &[], true)
            .unwrap();
        assert_eq!(record.borrow().starts, 1);
        assert_eq!(service.monitoring_state(), MonitoringState::Stopped);
        service.commit_prepared(prepared);
        assert_eq!(service.monitoring_state(), MonitoringState::LocalOnly);
        service.stop();
        assert_eq!(record.borrow().stops, 1);
    }

    #[test]
    fn quick_terminal_shortcut_cancelled_enable_stops_only_the_candidate() {
        let (candidate, record) = backend(
            "candidate",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            false,
            vec![candidate],
            vec![],
            false,
            |_| Ok(()),
        );
        service.start().unwrap();
        let prepared = service
            .prepare_shortcut_for_enabled(QuickTerminalShortcut::DoubleShift, &[], true)
            .unwrap();
        service.cancel_prepared(prepared);
        assert_eq!(record.borrow().stops, 1);
        assert_eq!(service.monitoring_state(), MonitoringState::Stopped);
    }

    #[test]
    fn quick_terminal_shortcut_failed_unassignment_keeps_active_backend() {
        let (first, first_record) = backend(
            "first",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            true,
            vec![first],
            vec![],
            false,
            |_| Err(io::Error::other("disk full")),
        );
        service.start().unwrap();
        assert!(matches!(
            service.update_shortcut(QuickTerminalShortcut::Unassigned, &[]),
            Err(ShortcutServiceError::Persistence(_))
        ));
        assert_eq!(first_record.borrow().stops, 0);
        assert_eq!(service.shortcut(), &QuickTerminalShortcut::DoubleShift);
        assert_eq!(service.monitoring_state(), MonitoringState::LocalOnly);
    }

    #[test]
    fn quick_terminal_shortcut_disable_and_reenable_replaces_monitoring() {
        let (first, first_record) = backend(
            "first",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (second, second_record) = backend(
            "second",
            MonitoringState::LocalOnly,
            MonitoringState::LocalOnly,
            None,
        );
        let (mut service, _) = service(
            QuickTerminalShortcut::DoubleShift,
            true,
            vec![first, second],
            vec![],
            false,
            |_| Ok(()),
        );
        service.start().unwrap();
        service.set_enabled(false).unwrap();
        assert_eq!(first_record.borrow().stops, 1);
        assert_eq!(service.monitoring_state(), MonitoringState::Stopped);
        service.set_enabled(true).unwrap();
        assert_eq!(second_record.borrow().starts, 1);
        assert_eq!(service.monitoring_state(), MonitoringState::LocalOnly);
    }

    #[test]
    fn quick_terminal_shortcut_layout_refresh_preserves_registration() {
        let (first, first_record) = backend(
            "first",
            MonitoringState::CarbonHotKey,
            MonitoringState::CarbonHotKey,
            None,
        );
        let saved = Rc::new(RefCell::new(None));
        let observed = saved.clone();
        let initial = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("a", COMMAND),
            virtual_key_code: 0,
        };
        let (mut service, _) =
            service(initial, true, vec![], vec![first], false, move |shortcut| {
                *observed.borrow_mut() = Some(shortcut.clone());
                Ok(())
            });
        service.start().unwrap();
        service.key_resolver = Box::new(|code| (code == 0).then(|| "q".to_owned()));
        let refreshed = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("a", COMMAND),
            virtual_key_code: 0,
        };
        service.update_shortcut(refreshed, &[]).unwrap();
        assert_eq!(first_record.borrow().starts, 1);
        assert_eq!(first_record.borrow().stops, 0);
        assert_eq!(
            service.shortcut().key_combo().unwrap(),
            &KeyCombo::new("q", COMMAND)
        );
        assert_eq!(saved.borrow().as_ref(), Some(service.shortcut()));
    }

    #[test]
    fn quick_terminal_shortcut_conflict_is_rejected_before_registration() {
        let (first, first_record) = backend(
            "first",
            MonitoringState::CarbonHotKey,
            MonitoringState::CarbonHotKey,
            None,
        );
        let (mut service, _) = service(
            QuickTerminalShortcut::Unassigned,
            true,
            vec![],
            vec![first],
            false,
            |_| Ok(()),
        );
        service.start().unwrap();
        let conflicts = [ConflictCandidate {
            label: "Open Project".to_owned(),
            combo: KeyCombo::new("space", COMMAND),
        }];
        assert!(matches!(
            service.update_shortcut(key_combo(), &conflicts),
            Err(ShortcutServiceError::Conflict(label)) if label == "Open Project"
        ));
        assert_eq!(first_record.borrow().starts, 0);
    }
}
