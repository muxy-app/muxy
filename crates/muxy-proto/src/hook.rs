use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const AGENT_HOOK_VERSION: i64 = 3;
pub const AGENT_HOOK_EVENT_KIND: &str = "agent_event";
pub const AGENT_HOOK_ACKNOWLEDGEMENT_KIND: &str = "ack";
pub const AGENT_HOOK_DEDUP_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentHookPhase {
    Working,
    Waiting,
    Finished,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentHookEvent {
    pub v: i64,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub provider: String,
    #[serde(rename = "paneID", skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    pub phase: AgentHookPhase,
    pub title: String,
    pub body: String,
    pub pids: Vec<i32>,
    pub ts: i64,
    #[serde(
        default,
        deserialize_with = "deserialize_default_bool",
        skip_serializing_if = "is_false"
    )]
    pub test: bool,
}

fn deserialize_default_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or(false))
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentHookAcknowledgement {
    pub kind: String,
    pub ok: bool,
    pub v: i64,
}

impl AgentHookAcknowledgement {
    pub fn success() -> Self {
        Self {
            kind: AGENT_HOOK_ACKNOWLEDGEMENT_KIND.to_owned(),
            ok: true,
            v: AGENT_HOOK_VERSION,
        }
    }
}

#[derive(Debug, Error)]
pub enum AgentHookParseError {
    #[error("invalid agent hook JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported agent hook version")]
    UnsupportedVersion,
    #[error("invalid agent hook kind")]
    InvalidKind,
    #[error("agent hook provider is empty")]
    EmptyProvider,
    #[error("invalid canonical pane ID")]
    InvalidPaneId,
}

pub fn parse_agent_hook_event(input: &str) -> Result<AgentHookEvent, AgentHookParseError> {
    let mut event = serde_json::from_str::<AgentHookEvent>(input.trim())?;
    if event.v != AGENT_HOOK_VERSION {
        return Err(AgentHookParseError::UnsupportedVersion);
    }
    if event.kind != AGENT_HOOK_EVENT_KIND {
        return Err(AgentHookParseError::InvalidKind);
    }
    if event.provider.is_empty() {
        return Err(AgentHookParseError::EmptyProvider);
    }
    if let Some(pane_id) = event.pane_id.as_deref() {
        event.pane_id = Some(normalize_canonical_uuid(pane_id)?);
    }
    Ok(event)
}

pub fn encode_agent_hook_event(event: &AgentHookEvent) -> Result<Vec<u8>, serde_json::Error> {
    encode_line(event)
}

pub fn parse_agent_hook_acknowledgement(
    input: &str,
) -> Result<AgentHookAcknowledgement, AgentHookParseError> {
    let acknowledgement = serde_json::from_str::<AgentHookAcknowledgement>(input.trim())?;
    if acknowledgement.v != AGENT_HOOK_VERSION {
        return Err(AgentHookParseError::UnsupportedVersion);
    }
    if acknowledgement.kind != AGENT_HOOK_ACKNOWLEDGEMENT_KIND {
        return Err(AgentHookParseError::InvalidKind);
    }
    Ok(acknowledgement)
}

pub fn encode_agent_hook_acknowledgement(
    acknowledgement: &AgentHookAcknowledgement,
) -> Result<Vec<u8>, serde_json::Error> {
    encode_line(acknowledgement)
}

fn encode_line<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut value = serde_json::to_value(value)?;
    sort_json_value(&mut value);
    let mut bytes = serde_json::to_vec(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sort_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries = std::mem::take(map).into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, mut value) in entries {
                sort_json_value(&mut value);
                map.insert(key, value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                sort_json_value(value);
            }
        }
        _ => {}
    }
}

fn normalize_canonical_uuid(value: &str) -> Result<String, AgentHookParseError> {
    let bytes = value.as_bytes();
    let canonical = bytes.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|index| bytes[index] == b'-');
    if !canonical {
        return Err(AgentHookParseError::InvalidPaneId);
    }
    let uuid = Uuid::parse_str(value).map_err(|_| AgentHookParseError::InvalidPaneId)?;
    Ok(uuid.hyphenated().to_string().to_uppercase())
}

#[derive(Clone, Debug)]
pub struct RecentAgentHookEventIds {
    capacity: usize,
    identifiers: HashSet<String>,
    insertion_order: VecDeque<String>,
}

impl Default for RecentAgentHookEventIds {
    fn default() -> Self {
        Self::new(AGENT_HOOK_DEDUP_CAPACITY)
    }
}

impl RecentAgentHookEventIds {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            identifiers: HashSet::new(),
            insertion_order: VecDeque::new(),
        }
    }

    pub fn register_and_check_is_first_delivery(&mut self, identifier: Option<&str>) -> bool {
        let Some(identifier) = identifier.filter(|identifier| !identifier.is_empty()) else {
            return true;
        };
        if !self.identifiers.insert(identifier.to_owned()) {
            return false;
        }
        self.insertion_order.push_back(identifier.to_owned());
        if self.insertion_order.len() > self.capacity
            && let Some(removed) = self.insertion_order.pop_front()
        {
            self.identifiers.remove(&removed);
        }
        true
    }

    pub fn len(&self) -> usize {
        self.identifiers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.identifiers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn valid_event() -> Value {
        json!({
            "v": 3,
            "kind": "agent_event",
            "provider": "sample",
            "phase": "finished",
            "title": "Done",
            "body": "Ready",
            "pids": [42, -7],
            "ts": 1720000000000_i64
        })
    }

    #[test]
    fn acknowledgement_bytes_are_exact_and_round_trip() {
        let acknowledgement = AgentHookAcknowledgement::success();
        let bytes = encode_agent_hook_acknowledgement(&acknowledgement).unwrap();
        assert_eq!(bytes, b"{\"kind\":\"ack\",\"ok\":true,\"v\":3}\n");
        assert_eq!(
            parse_agent_hook_acknowledgement(std::str::from_utf8(&bytes).unwrap()).unwrap(),
            acknowledgement
        );
    }

    #[test]
    fn optional_fields_default_and_unknown_keys_are_ignored() {
        let mut value = valid_event();
        value["id"] = Value::Null;
        value["paneID"] = Value::Null;
        value["test"] = Value::Null;
        value["unknown"] = json!({"nested": true});
        let event = parse_agent_hook_event(&value.to_string()).unwrap();
        assert_eq!(event.id, None);
        assert_eq!(event.pane_id, None);
        assert!(!event.test);
        assert_eq!(event.phase, AgentHookPhase::Finished);
    }

    #[test]
    fn optional_fields_are_strict_and_pane_ids_are_normalized() {
        let mut value = valid_event();
        value["id"] = json!("delivery-1");
        value["paneID"] = json!("123e4567-e89b-12d3-a456-426614174000");
        value["test"] = json!(true);
        let event = parse_agent_hook_event(&value.to_string()).unwrap();
        assert_eq!(event.id.as_deref(), Some("delivery-1"));
        assert_eq!(
            event.pane_id.as_deref(),
            Some("123E4567-E89B-12D3-A456-426614174000")
        );
        assert!(event.test);

        for (field, wrong) in [("id", json!(1)), ("paneID", json!(1)), ("test", json!(1))] {
            let mut value = valid_event();
            value[field] = wrong;
            assert!(parse_agent_hook_event(&value.to_string()).is_err());
        }
    }

    #[test]
    fn every_required_payload_field_is_required_and_strictly_typed() {
        let cases = [
            ("title", json!(1)),
            ("body", json!(false)),
            ("pids", json!("42")),
            ("pids", json!([2_147_483_648_i64])),
            ("ts", json!("1720000000000")),
        ];
        for field in ["title", "body", "pids", "ts"] {
            let mut value = valid_event();
            value.as_object_mut().unwrap().remove(field);
            assert!(
                parse_agent_hook_event(&value.to_string()).is_err(),
                "{field}"
            );
        }
        for (field, wrong) in cases {
            let mut value = valid_event();
            value[field] = wrong;
            assert!(
                parse_agent_hook_event(&value.to_string()).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn envelope_fields_are_required_and_strictly_typed() {
        for field in ["v", "kind", "provider", "phase"] {
            let mut value = valid_event();
            value.as_object_mut().unwrap().remove(field);
            assert!(
                parse_agent_hook_event(&value.to_string()).is_err(),
                "{field}"
            );
        }
        for (field, wrong) in [
            ("v", json!("3")),
            ("kind", json!(true)),
            ("provider", json!(1)),
            ("phase", json!(1)),
        ] {
            let mut value = valid_event();
            value[field] = wrong;
            assert!(
                parse_agent_hook_event(&value.to_string()).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn envelope_validation_rejects_unsupported_values() {
        let cases = [
            ("v", json!(2)),
            ("kind", json!("ack")),
            ("provider", json!("")),
            ("phase", json!("unknown")),
        ];
        for (field, wrong) in cases {
            let mut value = valid_event();
            value[field] = wrong;
            assert!(
                parse_agent_hook_event(&value.to_string()).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn pane_id_requires_the_canonical_hyphenated_shape() {
        for pane_id in [
            "123e4567e89b12d3a456426614174000",
            "{123e4567-e89b-12d3-a456-426614174000}",
            "123e4567-e89b-12d3-a456-42661417400z",
        ] {
            let mut value = valid_event();
            value["paneID"] = json!(pane_id);
            assert!(parse_agent_hook_event(&value.to_string()).is_err());
        }
    }

    #[test]
    fn event_encoding_is_sorted_newline_terminated_and_omits_default_options() {
        let event = parse_agent_hook_event(&valid_event().to_string()).unwrap();
        let encoded = encode_agent_hook_event(&event).unwrap();
        assert_eq!(
            encoded,
            b"{\"body\":\"Ready\",\"kind\":\"agent_event\",\"phase\":\"finished\",\"pids\":[42,-7],\"provider\":\"sample\",\"title\":\"Done\",\"ts\":1720000000000,\"v\":3}\n"
        );
        assert_eq!(
            parse_agent_hook_event(std::str::from_utf8(&encoded).unwrap()).unwrap(),
            event
        );
    }

    #[test]
    fn recent_ids_deduplicate_and_evict_in_insertion_order() {
        let mut recent = RecentAgentHookEventIds::new(256);
        assert!(recent.register_and_check_is_first_delivery(None));
        assert!(recent.register_and_check_is_first_delivery(Some("")));
        for index in 0..256 {
            assert!(recent.register_and_check_is_first_delivery(Some(&index.to_string())));
        }
        assert_eq!(recent.len(), 256);
        assert!(!recent.register_and_check_is_first_delivery(Some("0")));
        assert!(recent.register_and_check_is_first_delivery(Some("256")));
        assert_eq!(recent.len(), 256);
        assert!(recent.register_and_check_is_first_delivery(Some("0")));
        assert!(!recent.register_and_check_is_first_delivery(Some("256")));
    }
}
