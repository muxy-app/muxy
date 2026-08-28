mod coalescing;
mod store;

pub use coalescing::{DesktopPairCoalescer, DesktopPairEvent, PairOrigin};
pub use store::NotificationStore;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NotificationSource {
    Osc,
    AiProvider {
        #[serde(rename = "providerID")]
        provider_id: String,
    },
    Socket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationTarget {
    pub pane_id: String,
    pub project_id: String,
    pub worktree_id: String,
    pub area_id: String,
    pub tab_id: String,
    pub worktree_path: String,
}

impl NotificationTarget {
    pub fn new(
        pane_id: impl Into<String>,
        project_id: impl Into<String>,
        worktree_id: impl Into<String>,
        area_id: impl Into<String>,
        tab_id: impl Into<String>,
        worktree_path: impl Into<String>,
    ) -> Option<Self> {
        Some(Self {
            pane_id: canonical_uuid(&pane_id.into())?,
            project_id: canonical_uuid(&project_id.into())?,
            worktree_id: canonical_uuid(&worktree_id.into())?,
            area_id: canonical_uuid(&area_id.into())?,
            tab_id: canonical_uuid(&tab_id.into())?,
            worktree_path: worktree_path.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRecord {
    pub id: String,
    #[serde(rename = "paneID")]
    pub pane_id: String,
    #[serde(rename = "projectID")]
    pub project_id: String,
    #[serde(rename = "worktreeID")]
    pub worktree_id: String,
    #[serde(rename = "areaID")]
    pub area_id: String,
    #[serde(rename = "tabID")]
    pub tab_id: String,
    pub worktree_path: String,
    pub source: NotificationSource,
    pub title: String,
    pub body: String,
    pub timestamp: f64,
    #[serde(default, rename = "isRead")]
    pub is_read: bool,
}

impl NotificationRecord {
    pub fn new(
        target: NotificationTarget,
        source: NotificationSource,
        title: impl Into<String>,
        body: impl Into<String>,
        timestamp: f64,
    ) -> Option<Self> {
        Self::with_id(
            crate::store::new_uuid(),
            target,
            source,
            title,
            body,
            timestamp,
            false,
        )
    }

    pub fn with_id(
        id: impl Into<String>,
        target: NotificationTarget,
        source: NotificationSource,
        title: impl Into<String>,
        body: impl Into<String>,
        timestamp: f64,
        is_read: bool,
    ) -> Option<Self> {
        let id = canonical_uuid(&id.into())?;
        if !timestamp.is_finite() {
            return None;
        }
        Some(Self {
            id,
            pane_id: target.pane_id,
            project_id: target.project_id,
            worktree_id: target.worktree_id,
            area_id: target.area_id,
            tab_id: target.tab_id,
            worktree_path: target.worktree_path,
            source,
            title: title.into(),
            body: body.into(),
            timestamp,
            is_read,
        })
    }

    pub fn target(&self) -> NotificationTarget {
        NotificationTarget {
            pane_id: self.pane_id.clone(),
            project_id: self.project_id.clone(),
            worktree_id: self.worktree_id.clone(),
            area_id: self.area_id.clone(),
            tab_id: self.tab_id.clone(),
            worktree_path: self.worktree_path.clone(),
        }
    }
}

pub fn canonical_uuid(value: &str) -> Option<String> {
    (value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        }))
    .then(|| value.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PANE: &str = "11111111-2222-4333-8444-555555555555";
    const PROJECT: &str = "22222222-3333-4444-8555-666666666666";
    const WORKTREE: &str = "33333333-4444-4555-8666-777777777777";
    const AREA: &str = "44444444-5555-4666-8777-888888888888";
    const TAB: &str = "55555555-6666-4777-8888-999999999999";

    fn target() -> NotificationTarget {
        NotificationTarget::new(PANE, PROJECT, WORKTREE, AREA, TAB, "/tmp/worktree")
            .expect("target")
    }

    #[test]
    fn notifications_source_json_shapes_are_explicit() {
        assert_eq!(
            serde_json::to_value(NotificationSource::Osc).expect("OSC source"),
            serde_json::json!({"type": "osc"})
        );
        assert_eq!(
            serde_json::to_value(NotificationSource::Socket).expect("socket source"),
            serde_json::json!({"type": "socket"})
        );
        assert_eq!(
            serde_json::to_value(NotificationSource::AiProvider {
                provider_id: "codex".to_owned()
            })
            .expect("provider source"),
            serde_json::json!({"type": "aiProvider", "providerID": "codex"})
        );
    }

    #[test]
    fn notifications_records_generate_and_normalize_uppercase_ids() {
        let generated = NotificationRecord::new(
            target(),
            NotificationSource::Osc,
            "Title",
            "Body",
            crate::store::reference_now(),
        )
        .expect("record");
        assert_eq!(
            canonical_uuid(&generated.id).as_deref(),
            Some(generated.id.as_str())
        );

        let normalized = NotificationRecord::with_id(
            "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
            NotificationTarget::new(
                PANE.to_ascii_lowercase(),
                PROJECT.to_ascii_lowercase(),
                WORKTREE.to_ascii_lowercase(),
                AREA.to_ascii_lowercase(),
                TAB.to_ascii_lowercase(),
                "/tmp/worktree",
            )
            .expect("target"),
            NotificationSource::Socket,
            "",
            "",
            1.0,
            false,
        )
        .expect("record");
        assert_eq!(normalized.id, "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE");
        assert_eq!(normalized.pane_id, PANE);
        assert_eq!(normalized.project_id, PROJECT);
        assert_eq!(normalized.worktree_id, WORKTREE);
        assert_eq!(normalized.area_id, AREA);
        assert_eq!(normalized.tab_id, TAB);
    }

    #[test]
    fn notifications_reject_malformed_ids_and_non_finite_timestamps() {
        assert!(NotificationTarget::new("bad", PROJECT, WORKTREE, AREA, TAB, "/tmp").is_none());
        assert!(
            NotificationRecord::with_id(
                "bad",
                target(),
                NotificationSource::Osc,
                "",
                "",
                1.0,
                false,
            )
            .is_none()
        );
        assert!(
            NotificationRecord::new(target(), NotificationSource::Osc, "", "", f64::NAN,).is_none()
        );
        assert!(crate::store::reference_now().is_finite());
    }
}
