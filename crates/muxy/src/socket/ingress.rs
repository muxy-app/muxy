use std::collections::VecDeque;

use muxy_proto::extension::ExtensionLocalEvent;
use muxy_proto::hook::{AgentHookEvent, AgentHookPhase};
use muxy_proto::server::{ExtensionLocalEventIngress, LegacyNotificationIngress};

pub const INGRESS_CAPACITY: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyNotificationRecord {
    pub notification_type: String,
    pub raw_pane_id: Option<String>,
    pub sender_extension_id: Option<String>,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentHookResolution {
    ExplicitPane(String),
    ProcessMatch { pane_id: String, pid: u64 },
    Test,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentHookRecord {
    pub id: Option<String>,
    pub provider: String,
    pub pane_id: Option<String>,
    pub phase: AgentHookPhase,
    pub title: String,
    pub body: String,
    pub pids: Vec<i32>,
    pub timestamp: i64,
    pub test: bool,
    pub resolution: AgentHookResolution,
}

impl AgentHookRecord {
    pub fn new(event: AgentHookEvent, resolution: AgentHookResolution) -> Self {
        Self {
            id: event.id,
            provider: event.provider,
            pane_id: event.pane_id,
            phase: event.phase,
            title: event.title,
            body: event.body,
            pids: event.pids,
            timestamp: event.ts,
            test: event.test,
            resolution,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionLocalEventRecord {
    pub sender_extension_id: String,
    pub event: ExtensionLocalEvent,
}

impl From<ExtensionLocalEventIngress> for ExtensionLocalEventRecord {
    fn from(value: ExtensionLocalEventIngress) -> Self {
        Self {
            sender_extension_id: value.extension_id,
            event: value.event,
        }
    }
}

impl From<LegacyNotificationIngress> for LegacyNotificationRecord {
    fn from(value: LegacyNotificationIngress) -> Self {
        Self {
            notification_type: value.notification_type,
            raw_pane_id: value.pane_id,
            sender_extension_id: value.sender_extension_id,
            title: value.title,
            body: value.body,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BoundedIngress<T> {
    capacity: usize,
    records: VecDeque<T>,
}

impl<T> BoundedIngress<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            records: VecDeque::new(),
        }
    }

    pub fn push(&mut self, record: T) {
        if self.capacity == 0 {
            return;
        }
        if self.records.len() == self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }
}

#[derive(Clone, Debug)]
pub struct IngressQueues {
    pub legacy_notifications: BoundedIngress<LegacyNotificationRecord>,
    pub agent_hooks: BoundedIngress<AgentHookRecord>,
    pub extension_events: BoundedIngress<ExtensionLocalEventRecord>,
}

impl Default for IngressQueues {
    fn default() -> Self {
        Self {
            legacy_notifications: BoundedIngress::new(INGRESS_CAPACITY),
            agent_hooks: BoundedIngress::new(INGRESS_CAPACITY),
            extension_events: BoundedIngress::new(INGRESS_CAPACITY),
        }
    }
}

impl IngressQueues {
    pub fn push_legacy(&mut self, ingress: LegacyNotificationIngress) {
        self.legacy_notifications.push(ingress.into());
    }

    pub fn push_agent_hook(&mut self, event: AgentHookEvent, resolution: AgentHookResolution) {
        self.agent_hooks
            .push(AgentHookRecord::new(event, resolution));
    }

    pub fn push_extension_event(&mut self, event: ExtensionLocalEventIngress) {
        self.extension_events.push(event.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook(index: usize) -> AgentHookEvent {
        AgentHookEvent {
            v: 3,
            kind: "agent_event".to_owned(),
            id: Some(format!("event-{index}")),
            provider: "sample".to_owned(),
            pane_id: None,
            phase: AgentHookPhase::Waiting,
            title: format!("Title {index}"),
            body: format!("Body {index}"),
            pids: vec![index as i32],
            ts: index as i64,
            test: false,
        }
    }

    fn notification(index: usize) -> LegacyNotificationIngress {
        LegacyNotificationIngress {
            notification_type: "finished".to_owned(),
            pane_id: Some(format!("PANE-{index}")),
            title: format!("Title {index}"),
            body: format!("Body {index}"),
            sender_extension_id: None,
        }
    }

    #[test]
    fn legacy_records_preserve_every_typed_field() {
        let mut queues = IngressQueues::default();
        queues.push_legacy(notification(7));
        let record = queues.legacy_notifications.records.front().unwrap();
        assert_eq!(record.notification_type, "finished");
        assert_eq!(record.raw_pane_id.as_deref(), Some("PANE-7"));
        assert_eq!(record.sender_extension_id, None);
        assert_eq!(record.title, "Title 7");
        assert_eq!(record.body, "Body 7");
    }

    #[test]
    fn overflow_evicts_the_oldest_and_retains_the_newest() {
        let mut queues = IngressQueues::default();
        for index in 0..=INGRESS_CAPACITY {
            queues.push_legacy(notification(index));
        }
        let records = &queues.legacy_notifications.records;
        assert_eq!(records.len(), INGRESS_CAPACITY);
        assert_eq!(
            records.front().unwrap().raw_pane_id.as_deref(),
            Some("PANE-1")
        );
        assert_eq!(
            records.back().unwrap().raw_pane_id.as_deref(),
            Some("PANE-256")
        );
    }

    #[test]
    fn hook_records_retain_payload_and_typed_resolution() {
        let mut queues = IngressQueues::default();
        queues.push_agent_hook(
            hook(42),
            AgentHookResolution::ProcessMatch {
                pane_id: "PANE".to_owned(),
                pid: 42,
            },
        );
        let record = queues.agent_hooks.records.front().unwrap();
        assert_eq!(record.id.as_deref(), Some("event-42"));
        assert_eq!(record.phase, AgentHookPhase::Waiting);
        assert_eq!(record.pids, [42]);
        assert_eq!(
            record.resolution,
            AgentHookResolution::ProcessMatch {
                pane_id: "PANE".to_owned(),
                pid: 42
            }
        );
    }

    #[test]
    fn every_ingress_kind_evicts_oldest_and_retains_newest() {
        let mut hooks = BoundedIngress::new(2);
        hooks.push(AgentHookRecord::new(
            hook(0),
            AgentHookResolution::Unresolved,
        ));
        hooks.push(AgentHookRecord::new(hook(1), AgentHookResolution::Test));
        hooks.push(AgentHookRecord::new(
            hook(2),
            AgentHookResolution::Unresolved,
        ));
        assert_eq!(
            hooks.records.front().unwrap().id.as_deref(),
            Some("event-1")
        );
        assert_eq!(hooks.records.back().unwrap().id.as_deref(), Some("event-2"));

        let mut events = BoundedIngress::new(1);
        for name in ["extension.one", "extension.two"] {
            events.push(ExtensionLocalEventRecord {
                sender_extension_id: "sample".to_owned(),
                event: ExtensionLocalEvent {
                    name: name.to_owned(),
                    payload: Vec::new(),
                },
            });
        }
        assert_eq!(events.records.len(), 1);
        assert_eq!(events.records.front().unwrap().event.name, "extension.two");
    }

    #[test]
    fn zero_capacity_drops_records_without_growing() {
        let mut ingress = BoundedIngress::new(0);
        ingress.push(notification(1));
        assert!(ingress.records.is_empty());
    }
}
