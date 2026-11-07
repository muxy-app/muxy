//! The thread that reads each agent's tabs while terminals run it.

use std::collections::{HashMap, HashSet};
use std::sync::Weak;
use std::time::{Duration, Instant};

use super::Report;
use crate::activity::Activity;

const INTERVAL: Duration = Duration::from_millis(1500);
/// The least time between polls, so a burst of terminal changes costs one.
const SETTLE: Duration = Duration::from_millis(200);
/// How long a report's busy sessions last without a successful poll. A
/// failed poll keeps the last tabs, so they don't blink out, but not work
/// that may have ended since.
const STALE: Duration = Duration::from_secs(10);

/// Polls every source while any terminal runs its agent, every `INTERVAL`
/// or soon after a terminal starts, stops, or switches tabs, until
/// `activity` is dropped.
pub(crate) fn poll(activity: &Weak<Activity>) {
    let mut sources = super::sources();
    let mut last_success: HashMap<muxy_protocol::AgentProvider, Instant> = HashMap::new();
    loop {
        let Some(shared) = activity.upgrade() else {
            return;
        };
        let panes = shared.wait_for_agent_tabs(INTERVAL);
        for source in &mut sources {
            let provider = source.provider();
            let directories: HashSet<String> = panes
                .values()
                .filter(|pane| pane.provider == provider)
                .map(|pane| pane.directory.clone())
                .collect();
            if directories.is_empty() {
                last_success.remove(&provider);
                shared.set_agent_tabs_report(provider, Report::default());
            } else if let Some(report) = source.report(&directories) {
                last_success.insert(provider, Instant::now());
                shared.set_agent_tabs_report(provider, report);
            } else if last_success
                .get(&provider)
                .is_none_or(|success| success.elapsed() >= STALE)
            {
                shared.clear_agent_tabs_busy(provider);
            }
        }
        drop(shared);
        std::thread::sleep(SETTLE);
    }
}
