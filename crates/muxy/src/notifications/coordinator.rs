use super::desktop::{AuthorizationResult, DesktopNotificationService, DesktopRequest};
use super::sound::NotificationSoundPlayer;
use crate::socket::ingress::{AgentHookRecord, AgentHookResolution, LegacyNotificationRecord};
use gpui::Task;
use muxy_core::notifications::{NotificationRecord, NotificationSource, NotificationTarget};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationOrigin {
    TerminalOsc,
    AgentHook,
    AgentHookTest,
    LegacySocket,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedNotificationEvent {
    pub target: Option<NotificationTarget>,
    pub source: NotificationSource,
    pub origin: NotificationOrigin,
    pub title: String,
    pub body: String,
    pub timestamp: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryInputs {
    pub focused_osc: bool,
    pub toast_enabled: bool,
    pub desktop_enabled: bool,
    pub sound: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToastDelivery {
    pub notification_id: Option<String>,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeliveryEffects {
    pub record: Option<NotificationRecord>,
    pub schedule_save: bool,
    pub toast: Option<ToastDelivery>,
    pub desktop_notification_id: Option<String>,
    pub sound: Option<String>,
    pub notify: bool,
}

pub(crate) fn resolve_legacy_notification(
    record: &LegacyNotificationRecord,
    timestamp: f64,
    resolve_pane: impl Fn(&str) -> Option<NotificationTarget>,
    mut active_fallback: impl FnMut() -> Option<NotificationTarget>,
) -> Option<ResolvedNotificationEvent> {
    let target = match record
        .raw_pane_id
        .as_deref()
        .and_then(muxy_core::notifications::canonical_uuid)
    {
        Some(pane_id) => resolve_pane(&pane_id),
        None => active_fallback(),
    }?;
    let source =
        muxy_core::repository_ai::provider_for_notification_socket_key(&record.notification_type)
            .map_or(NotificationSource::Socket, |provider| {
                NotificationSource::AiProvider {
                    provider_id: provider.id.to_owned(),
                }
            });
    Some(ResolvedNotificationEvent {
        target: Some(target),
        source,
        origin: NotificationOrigin::LegacySocket,
        title: record.title.clone(),
        body: record.body.clone(),
        timestamp,
    })
}

pub(crate) fn resolve_agent_hook_notification(
    record: &AgentHookRecord,
    timestamp: f64,
    resolve_pane: impl Fn(&str) -> Option<NotificationTarget>,
    mut active_fallback: impl FnMut() -> Option<NotificationTarget>,
) -> Option<ResolvedNotificationEvent> {
    let provider = muxy_core::repository_ai::provider(&record.provider)?;
    let (target, origin) = if record.test {
        let target = match record
            .pane_id
            .as_deref()
            .and_then(muxy_core::notifications::canonical_uuid)
        {
            Some(pane_id) => resolve_pane(&pane_id),
            None => active_fallback(),
        }?;
        (target, NotificationOrigin::AgentHookTest)
    } else {
        if record.title.is_empty() && record.body.is_empty() {
            return None;
        }
        let pane_id = match &record.resolution {
            AgentHookResolution::ExplicitPane(pane_id)
            | AgentHookResolution::ProcessMatch { pane_id, .. } => pane_id,
            AgentHookResolution::Test | AgentHookResolution::Unresolved => return None,
        };
        (resolve_pane(pane_id)?, NotificationOrigin::AgentHook)
    };
    let title = if record.title.is_empty() {
        if record.test {
            "Notifications"
        } else {
            "Task completed!"
        }
    } else {
        &record.title
    };
    Some(ResolvedNotificationEvent {
        target: Some(target),
        source: NotificationSource::AiProvider {
            provider_id: provider.id.to_owned(),
        },
        origin,
        title: title.to_owned(),
        body: record.body.clone(),
        timestamp,
    })
}

#[derive(Default)]
struct DeliveryPolicy {
    desktop_pairs: muxy_core::notifications::DesktopPairCoalescer,
}

impl DeliveryPolicy {
    fn decide(
        &mut self,
        event: &ResolvedNotificationEvent,
        inputs: DeliveryInputs,
    ) -> DeliveryEffects {
        if event.origin == NotificationOrigin::TerminalOsc && inputs.focused_osc {
            return DeliveryEffects {
                sound: Some(inputs.sound),
                ..DeliveryEffects::default()
            };
        }
        let record = event.target.clone().and_then(|target| {
            NotificationRecord::new(
                target,
                event.source.clone(),
                event.title.clone(),
                event.body.clone(),
                event.timestamp,
            )
        });
        let notification_id = record
            .as_ref()
            .map(|record| record.id.clone())
            .or_else(|| inputs.desktop_enabled.then(muxy_core::store::new_uuid));
        let toast = inputs.toast_enabled.then(|| ToastDelivery {
            notification_id: record.as_ref().map(|record| record.id.clone()),
            title: event.title.clone(),
            body: event.body.clone(),
        });
        let desktop_allowed = event.target.as_ref().is_none_or(|target| {
            let pair_origin = match event.origin {
                NotificationOrigin::TerminalOsc => {
                    muxy_core::notifications::PairOrigin::TerminalOsc
                }
                NotificationOrigin::AgentHook => muxy_core::notifications::PairOrigin::AgentHook,
                NotificationOrigin::AgentHookTest | NotificationOrigin::LegacySocket => {
                    muxy_core::notifications::PairOrigin::Other
                }
            };
            self.desktop_pairs
                .allow_desktop(muxy_core::notifications::DesktopPairEvent {
                    origin: pair_origin,
                    project_id: &target.project_id,
                    worktree_id: &target.worktree_id,
                    area_id: &target.area_id,
                    tab_id: &target.tab_id,
                    title: &event.title,
                    body: &event.body,
                    timestamp: event.timestamp,
                })
        });
        DeliveryEffects {
            schedule_save: record.is_some(),
            record,
            toast,
            desktop_notification_id: (inputs.desktop_enabled && desktop_allowed)
                .then_some(notification_id)
                .flatten(),
            sound: Some(inputs.sound),
            notify: true,
        }
    }
}

#[derive(Default)]
struct SaveTaskState {
    generation: u64,
}

impl SaveTaskState {
    fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation == generation
    }
}

#[derive(Default)]
struct AuthorizationRequestState {
    generation: u64,
    pending: bool,
}

impl AuthorizationRequestState {
    fn begin(&mut self) -> Option<u64> {
        if self.pending {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.pending = true;
        Some(self.generation)
    }

    fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = false;
    }

    fn complete(&mut self, generation: u64, result: AuthorizationResult) -> Option<bool> {
        if !self.pending || generation != self.generation {
            return None;
        }
        self.pending = false;
        Some(matches!(result, AuthorizationResult::Allowed))
    }
}

pub struct NotificationCoordinator {
    desktop: DesktopNotificationService,
    sound: NotificationSoundPlayer,
    policy: DeliveryPolicy,
    authorization: AuthorizationRequestState,
    authorization_task: Option<Task<()>>,
    save_state: SaveTaskState,
    save_task: Option<Task<()>>,
}

impl NotificationCoordinator {
    pub fn new(desktop: DesktopNotificationService) -> Self {
        Self {
            desktop,
            sound: NotificationSoundPlayer::new(),
            policy: DeliveryPolicy::default(),
            authorization: AuthorizationRequestState::default(),
            authorization_task: None,
            save_state: SaveTaskState::default(),
            save_task: None,
        }
    }

    pub fn decide(
        &mut self,
        event: &ResolvedNotificationEvent,
        inputs: DeliveryInputs,
    ) -> DeliveryEffects {
        self.policy.decide(event, inputs)
    }

    pub fn next_save_generation(&mut self) -> u64 {
        self.save_state.next_generation()
    }

    pub fn save_generation_is_current(&self, generation: u64) -> bool {
        self.save_state.is_current(generation)
    }

    pub fn set_save_task(&mut self, task: Task<()>) {
        self.save_task = Some(task);
    }

    pub fn query_desktop_authorization(
        &self,
    ) -> async_channel::Receiver<super::desktop::AuthorizationStatus> {
        self.desktop.query_authorization()
    }

    pub fn begin_desktop_authorization(
        &mut self,
    ) -> Option<(u64, async_channel::Receiver<AuthorizationResult>)> {
        let generation = self.authorization.begin()?;
        Some((generation, self.desktop.request_authorization()))
    }

    pub fn set_authorization_task(&mut self, task: Task<()>) {
        self.authorization_task = Some(task);
    }

    pub fn cancel_desktop_authorization(&mut self) {
        self.authorization.cancel();
        self.authorization_task = None;
    }

    pub fn complete_desktop_authorization(
        &mut self,
        generation: u64,
        result: AuthorizationResult,
    ) -> Option<bool> {
        self.authorization.complete(generation, result)
    }

    pub fn schedule_desktop(&self, request: DesktopRequest) {
        self.desktop.schedule(request);
    }

    pub fn play_sound(&mut self, name: &str) -> bool {
        self.sound.play(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PANE: &str = "11111111-2222-4333-8444-555555555555";
    const PROJECT: &str = "22222222-3333-4444-8555-666666666666";
    const WORKTREE: &str = "33333333-4444-4555-8666-777777777777";
    const AREA: &str = "44444444-5555-4666-8777-888888888888";
    const TAB: &str = "55555555-6666-4777-8888-999999999999";

    fn event(origin: NotificationOrigin, timestamp: f64) -> ResolvedNotificationEvent {
        ResolvedNotificationEvent {
            target: NotificationTarget::new(PANE, PROJECT, WORKTREE, AREA, TAB, "/tmp/worktree"),
            source: if origin == NotificationOrigin::TerminalOsc {
                NotificationSource::Osc
            } else {
                NotificationSource::AiProvider {
                    provider_id: "codex".to_owned(),
                }
            },
            origin,
            title: "Task completed!".to_owned(),
            body: "Finished".to_owned(),
            timestamp,
        }
    }

    fn inputs() -> DeliveryInputs {
        DeliveryInputs {
            focused_osc: false,
            toast_enabled: true,
            desktop_enabled: true,
            sound: "Funk".to_owned(),
        }
    }

    fn target() -> NotificationTarget {
        event(NotificationOrigin::LegacySocket, 1.0).target.unwrap()
    }

    fn legacy(pane_id: Option<&str>, notification_type: &str) -> LegacyNotificationRecord {
        LegacyNotificationRecord {
            notification_type: notification_type.to_owned(),
            raw_pane_id: pane_id.map(str::to_owned),
            sender_extension_id: Some("sample.extension".to_owned()),
            title: "Task completed!".to_owned(),
            body: "Finished".to_owned(),
        }
    }

    fn hook(
        provider: &str,
        resolution: AgentHookResolution,
        test: bool,
        title: &str,
        body: &str,
    ) -> AgentHookRecord {
        AgentHookRecord {
            id: Some("event".to_owned()),
            provider: provider.to_owned(),
            pane_id: None,
            phase: muxy_proto::hook::AgentHookPhase::Finished,
            title: title.to_owned(),
            body: body.to_owned(),
            pids: vec![42],
            timestamp: 123,
            test,
            resolution,
        }
    }

    #[test]
    fn notifications_legacy_conversion_preserves_payload_and_maps_provider_source() {
        let resolved = resolve_legacy_notification(
            &legacy(Some(PANE), "codex_hook"),
            44.0,
            |pane| (pane == PANE).then(target),
            || None,
        )
        .unwrap();
        assert_eq!(resolved.target, Some(target()));
        assert_eq!(
            resolved.source,
            NotificationSource::AiProvider {
                provider_id: "codex".to_owned()
            }
        );
        assert_eq!(resolved.origin, NotificationOrigin::LegacySocket);
        assert_eq!(resolved.title, "Task completed!");
        assert_eq!(resolved.body, "Finished");
        assert_eq!(resolved.timestamp, 44.0);
    }

    #[test]
    fn notifications_legacy_conversion_uses_only_invalid_or_missing_pane_fallback() {
        let mut fallback_calls = 0;
        let stale = resolve_legacy_notification(
            &legacy(Some(PANE), "unknown"),
            1.0,
            |_| None,
            || {
                fallback_calls += 1;
                Some(target())
            },
        );
        assert!(stale.is_none());
        assert_eq!(fallback_calls, 0);

        for pane in [None, Some("invalid")] {
            let resolved = resolve_legacy_notification(
                &legacy(pane, "unknown"),
                2.0,
                |_| None,
                || Some(target()),
            )
            .unwrap();
            assert_eq!(resolved.source, NotificationSource::Socket);
        }
        assert!(
            resolve_legacy_notification(&legacy(None, "unknown"), 2.0, |_| None, || None).is_none()
        );
    }

    #[test]
    fn notifications_hook_conversion_requires_known_provider_and_resolved_normal_target() {
        let explicit = hook(
            "codex",
            AgentHookResolution::ExplicitPane(PANE.to_owned()),
            false,
            "",
            "Finished",
        );
        let resolved = resolve_agent_hook_notification(
            &explicit,
            88.0,
            |pane| (pane == PANE).then(target),
            || None,
        )
        .unwrap();
        assert_eq!(resolved.title, "Task completed!");
        assert_eq!(resolved.body, "Finished");
        assert_eq!(resolved.origin, NotificationOrigin::AgentHook);
        assert_eq!(resolved.timestamp, 88.0);

        let unknown = hook(
            "unknown",
            AgentHookResolution::ExplicitPane(PANE.to_owned()),
            false,
            "Title",
            "Body",
        );
        assert!(
            resolve_agent_hook_notification(&unknown, 1.0, |_| Some(target()), || Some(target()))
                .is_none()
        );
        let unresolved = hook(
            "codex",
            AgentHookResolution::Unresolved,
            false,
            "Title",
            "Body",
        );
        assert!(
            resolve_agent_hook_notification(
                &unresolved,
                1.0,
                |_| Some(target()),
                || Some(target())
            )
            .is_none()
        );
    }

    #[test]
    fn notifications_hook_conversion_test_fallback_and_stale_rules_are_exact() {
        let test = hook("xal", AgentHookResolution::Test, true, "", "Test body");
        let resolved =
            resolve_agent_hook_notification(&test, 9.0, |_| None, || Some(target())).unwrap();
        assert_eq!(resolved.title, "Notifications");
        assert_eq!(resolved.body, "Test body");
        assert_eq!(resolved.origin, NotificationOrigin::AgentHookTest);

        let mut explicit_test = test.clone();
        explicit_test.pane_id = Some(PANE.to_owned());
        let mut fallback_calls = 0;
        assert!(
            resolve_agent_hook_notification(
                &explicit_test,
                9.0,
                |_| None,
                || {
                    fallback_calls += 1;
                    Some(target())
                }
            )
            .is_none()
        );
        assert_eq!(fallback_calls, 0);

        let mut invalid_test = test;
        invalid_test.pane_id = Some("invalid".to_owned());
        assert!(
            resolve_agent_hook_notification(&invalid_test, 9.0, |_| None, || Some(target()))
                .is_some()
        );
    }

    #[test]
    fn notifications_hook_conversion_drops_empty_normal_text_but_all_phases_can_deliver() {
        let empty = hook(
            "claude",
            AgentHookResolution::ProcessMatch {
                pane_id: PANE.to_owned(),
                pid: 42,
            },
            false,
            "",
            "",
        );
        assert!(
            resolve_agent_hook_notification(&empty, 1.0, |_| Some(target()), || None).is_none()
        );

        for phase in [
            muxy_proto::hook::AgentHookPhase::Working,
            muxy_proto::hook::AgentHookPhase::Waiting,
            muxy_proto::hook::AgentHookPhase::Finished,
        ] {
            let mut eligible = empty.clone();
            eligible.phase = phase;
            eligible.body = "status".to_owned();
            assert!(
                resolve_agent_hook_notification(&eligible, 1.0, |_| Some(target()), || None)
                    .is_some()
            );
        }
    }

    #[test]
    fn notifications_delivery_normal_event_requests_every_independent_effect() {
        let mut coordinator = DeliveryPolicy::default();
        let effects = coordinator.decide(&event(NotificationOrigin::LegacySocket, 10.0), inputs());
        let record = effects.record.as_ref().unwrap();
        assert!(!record.is_read);
        assert_eq!(record.title, "Task completed!");
        assert!(effects.schedule_save);
        assert_eq!(
            effects
                .toast
                .as_ref()
                .and_then(|toast| toast.notification_id.as_deref()),
            Some(record.id.as_str())
        );
        assert_eq!(
            effects.desktop_notification_id.as_deref(),
            Some(record.id.as_str())
        );
        assert_eq!(effects.sound.as_deref(), Some("Funk"));
        assert!(effects.notify);
    }

    #[test]
    fn quick_terminal_osc_notification_has_effects_without_workspace_navigation() {
        let mut coordinator = DeliveryPolicy::default();
        let mut notification = event(NotificationOrigin::TerminalOsc, 10.0);
        notification.target = None;
        let effects = coordinator.decide(&notification, inputs());
        assert!(effects.record.is_none());
        assert!(!effects.schedule_save);
        assert!(
            effects
                .toast
                .as_ref()
                .is_some_and(|toast| toast.notification_id.is_none())
        );
        assert!(effects.desktop_notification_id.is_some());
        assert_eq!(effects.sound.as_deref(), Some("Funk"));
        assert!(effects.notify);
    }

    #[test]
    fn notifications_delivery_focused_osc_is_sound_only() {
        let mut coordinator = DeliveryPolicy::default();
        let mut inputs = inputs();
        inputs.focused_osc = true;
        let effects = coordinator.decide(&event(NotificationOrigin::TerminalOsc, 10.0), inputs);
        assert!(effects.record.is_none());
        assert!(!effects.schedule_save);
        assert!(effects.toast.is_none());
        assert!(effects.desktop_notification_id.is_none());
        assert_eq!(effects.sound.as_deref(), Some("Funk"));
        assert!(!effects.notify);
    }

    #[test]
    fn notifications_delivery_settings_are_independent() {
        let mut coordinator = DeliveryPolicy::default();
        let mut inputs = inputs();
        inputs.toast_enabled = false;
        inputs.desktop_enabled = false;
        inputs.sound = "None".to_owned();
        let effects = coordinator.decide(&event(NotificationOrigin::LegacySocket, 10.0), inputs);
        assert!(effects.record.is_some());
        assert!(effects.toast.is_none());
        assert!(effects.desktop_notification_id.is_none());
        assert_eq!(effects.sound.as_deref(), Some("None"));
    }

    #[test]
    fn desktop_notification_osc_hook_pair_suppresses_only_second_native_request() {
        let mut coordinator = DeliveryPolicy::default();
        let osc = coordinator.decide(&event(NotificationOrigin::TerminalOsc, 10.0), inputs());
        let hook = coordinator.decide(&event(NotificationOrigin::AgentHook, 12.0), inputs());
        assert!(osc.record.is_some());
        assert!(hook.record.is_some());
        assert!(osc.toast.is_some());
        assert!(hook.toast.is_some());
        assert!(osc.desktop_notification_id.is_some());
        assert!(hook.desktop_notification_id.is_none());
        assert!(osc.sound.is_some());
        assert!(hook.sound.is_some());
    }

    #[test]
    fn notifications_save_generation_replaces_stale_tasks() {
        let mut state = SaveTaskState::default();
        let first = state.next_generation();
        let second = state.next_generation();
        assert_ne!(first, second);
        assert!(!state.is_current(first));
        assert!(state.is_current(second));
    }

    #[test]
    fn settings_desktop_authorization_gate_rejects_repeats_and_stale_results() {
        let mut state = AuthorizationRequestState::default();
        let first = state.begin().unwrap();
        assert_eq!(state.begin(), None);
        state.cancel();
        assert_eq!(state.complete(first, AuthorizationResult::Allowed), None);

        let second = state.begin().unwrap();
        assert_ne!(first, second);
        assert_eq!(
            state.complete(second, AuthorizationResult::Allowed),
            Some(true)
        );
    }

    #[test]
    fn settings_desktop_authorization_persists_false_for_non_allowed_results() {
        for result in [
            AuthorizationResult::Denied,
            AuthorizationResult::Unavailable,
            AuthorizationResult::Failed,
        ] {
            let mut state = AuthorizationRequestState::default();
            let generation = state.begin().unwrap();
            assert_eq!(state.complete(generation, result), Some(false));
        }
    }
}
