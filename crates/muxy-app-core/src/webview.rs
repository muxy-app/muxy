use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AppError, AppState, PaneContent, PaneId, ProjectId, Tab, TabId};

pub const MAX_RESULT_BYTES: usize = 256 * 1024;
pub const ACKNOWLEDGEMENT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WebviewDescriptor {
    pub owner: String,
    pub kind: String,
    pub data: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModalOptions {
    pub width: f64,
    pub height: f64,
    pub dismiss_on_outside_click: bool,
}

impl Default for ModalOptions {
    fn default() -> Self {
        Self {
            width: 480.0,
            height: 320.0,
            dismiss_on_outside_click: true,
        }
    }
}

impl ModalOptions {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.width = finite_size(self.width, 480.0, 900.0);
        self.height = finite_size(self.height, 320.0, 760.0);
        self
    }
}

fn finite_size(value: f64, default: f64, maximum: f64) -> f64 {
    if value.is_finite() {
        value.clamp(120.0, maximum)
    } else {
        default
    }
}

#[derive(Debug, Default)]
pub struct ModalResult {
    completed: bool,
}

impl ModalResult {
    pub fn complete(&mut self, value: Value) -> Option<Value> {
        if std::mem::replace(&mut self.completed, true) {
            return None;
        }
        Some(
            if serde_json::to_vec(&value).is_ok_and(|bytes| bytes.len() <= MAX_RESULT_BYTES) {
                value
            } else {
                Value::Null
            },
        )
    }
}

#[derive(Debug, Default)]
pub struct CloseRequests {
    next: u64,
    pending: BTreeMap<u64, Option<Instant>>,
}

impl CloseRequests {
    pub fn begin(&mut self, now: Instant) -> u64 {
        self.next += 1;
        self.pending
            .insert(self.next, Some(now + ACKNOWLEDGEMENT_TIMEOUT));
        self.next
    }

    pub fn acknowledge(&mut self, id: u64) {
        if let Some(deadline) = self.pending.get_mut(&id) {
            *deadline = None;
        }
    }

    pub fn resolve(&mut self, id: u64) -> bool {
        self.pending.remove(&id).is_some()
    }

    pub fn expire(&mut self, now: Instant) -> Vec<u64> {
        let expired: Vec<_> = self
            .pending
            .iter()
            .filter_map(|(id, deadline)| {
                deadline
                    .is_some_and(|deadline| now >= deadline)
                    .then_some(*id)
            })
            .collect();
        for id in &expired {
            self.pending.remove(id);
        }
        expired
    }

    pub fn drain(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.pending).into_keys().collect()
    }
}

impl AppState {
    pub fn open_webview(
        &mut self,
        project: ProjectId,
        descriptor: WebviewDescriptor,
        title: &str,
        singleton: bool,
    ) -> Result<(TabId, PaneId), AppError> {
        if singleton {
            let existing = self.project(project).and_then(|project| {
                project.tabs.iter().find_map(|tab| {
                    tab.panes.iter().find_map(|pane| match &pane.content {
                        PaneContent::Webview(current)
                            if current.owner == descriptor.owner
                                && current.kind == descriptor.kind =>
                        {
                            Some((tab.id, pane.id))
                        }
                        _ => None,
                    })
                })
            });
            if let Some((tab, pane)) = existing {
                self.set_webview_data(pane, descriptor.data)?;
                self.select_project(project)?;
                self.select_tab(project, tab)?;
                self.focus_pane(pane)?;
                return Ok((tab, pane));
            }
        }
        let tab = Tab::with_content(PaneContent::Webview(descriptor), title);
        let id = tab.id;
        let pane = tab.panes[0].id;
        self.project_mut(project)?.tabs.push(tab);
        self.select_project(project)?;
        self.select_tab(project, id)?;
        Ok((id, pane))
    }

    pub fn set_webview_data(&mut self, pane: PaneId, data: Value) -> Result<(), AppError> {
        match &mut self.pane_mut(pane)?.content {
            PaneContent::Webview(descriptor) => {
                descriptor.data = data;
                Ok(())
            }
            _ => Err(AppError::InvalidState("pane is not a webview".into())),
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "Exact clamped pixel dimensions")]
mod tests {
    use super::*;

    #[test]
    fn modal_sizes_results_and_once_only_completion() {
        let options = ModalOptions {
            width: f64::NAN,
            height: 1000.0,
            ..Default::default()
        }
        .normalized();
        assert_eq!(options.width, 480.0);
        assert_eq!(options.height, 760.0);
        assert_eq!(
            ModalOptions {
                width: 1.0,
                ..Default::default()
            }
            .normalized()
            .width,
            120.0
        );
        let mut result = ModalResult::default();
        assert_eq!(result.complete(Value::Bool(true)), Some(Value::Bool(true)));
        assert_eq!(result.complete(Value::Null), None);
        assert_eq!(
            ModalResult::default().complete(Value::String("x".repeat(MAX_RESULT_BYTES))),
            Some(Value::Null)
        );
    }

    #[test]
    fn acknowledged_close_waits_and_stale_replies_are_ignored() {
        let now = Instant::now();
        let mut requests = CloseRequests::default();
        let first = requests.begin(now);
        let second = requests.begin(now);
        requests.acknowledge(second);
        assert_eq!(requests.expire(now + ACKNOWLEDGEMENT_TIMEOUT), vec![first]);
        assert!(!requests.resolve(first));
        assert!(requests.resolve(second));
        assert!(!requests.resolve(second));
        let third = requests.begin(now);
        assert_eq!(requests.drain(), vec![third]);
        assert!(!requests.resolve(third));
    }
}
