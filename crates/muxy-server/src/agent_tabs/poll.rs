use std::collections::{HashMap, HashSet};
use std::sync::Weak;
use std::time::{Duration, Instant};

use super::Report;
use crate::activity::Activity;

const INTERVAL: Duration = Duration::from_millis(1500);
const SETTLE: Duration = Duration::from_millis(200);
const STALE: Duration = Duration::from_secs(10);

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
