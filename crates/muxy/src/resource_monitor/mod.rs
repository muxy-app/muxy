#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(target_os = "macos"))]
use unsupported as platform;

use muxy_core::resources::{
    ProcessIdentity, ProcessResourceRecord, ProcessResourceSample, ResourceAvailability,
    ResourceSnapshot, ResourceTotals, aggregate_resources, process_tree_resources,
};
use std::time::{Duration, Instant};

pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceMonitorSnapshot {
    pub total: ResourceSnapshot,
    pub app: Option<ResourceTotals>,
    pub sessions: Option<ResourceTotals>,
    pub session_roots: Vec<ProcessIdentity>,
    pub session_processes: Vec<ProcessIdentity>,
    pub session_count: usize,
    pub poll_count: u64,
}

impl Default for ResourceMonitorSnapshot {
    fn default() -> Self {
        Self {
            total: ResourceSnapshot {
                availability: ResourceAvailability::Unavailable,
                totals: None,
                sample_age_milliseconds: 0,
            },
            app: None,
            sessions: None,
            session_roots: Vec::new(),
            session_processes: Vec::new(),
            session_count: 0,
            poll_count: 0,
        }
    }
}

#[derive(Clone)]
pub struct ResourcePollRequest {
    generation: u64,
    app_identity: ProcessIdentity,
    session_identities: Vec<ProcessIdentity>,
    session_count: usize,
}

pub(crate) struct ResourceSampleSet {
    total: Vec<ProcessResourceSample>,
    app: Vec<ProcessResourceSample>,
    sessions: Vec<ProcessResourceSample>,
    session_count: usize,
}

struct ResourceBaseline {
    at: Instant,
    total: Vec<ProcessResourceSample>,
    app: Vec<ProcessResourceSample>,
    sessions: Vec<ProcessResourceSample>,
}

pub struct ResourceMonitor {
    enabled: bool,
    generation: u64,
    app_identity: Option<ProcessIdentity>,
    baseline: Option<ResourceBaseline>,
    last_success: Option<Instant>,
    snapshot: ResourceMonitorSnapshot,
}

impl ResourceMonitor {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            generation: u64::from(enabled),
            app_identity: platform::current_process_identity().ok(),
            baseline: None,
            last_success: None,
            snapshot: ResourceMonitorSnapshot::default(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn snapshot(&self) -> &ResourceMonitorSnapshot {
        &self.snapshot
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        self.generation = self.generation.wrapping_add(1);
        self.baseline = None;
        self.last_success = None;
        let poll_count = self.snapshot.poll_count;
        self.snapshot = ResourceMonitorSnapshot {
            poll_count,
            ..ResourceMonitorSnapshot::default()
        };
    }

    pub fn request(
        &mut self,
        session_identities: Vec<ProcessIdentity>,
        session_count: usize,
    ) -> Option<ResourcePollRequest> {
        if self.enabled && self.app_identity.is_none() {
            self.app_identity = platform::current_process_identity().ok();
        }
        Some(ResourcePollRequest {
            generation: self.generation,
            app_identity: self.enabled.then_some(self.app_identity).flatten()?,
            session_identities,
            session_count,
        })
    }

    pub fn apply(
        &mut self,
        request: &ResourcePollRequest,
        sample: Result<ResourceSampleSet, String>,
        now: Instant,
    ) {
        if !self.enabled || request.generation != self.generation {
            return;
        }
        self.snapshot.poll_count = self.snapshot.poll_count.saturating_add(1);
        match sample {
            Ok(sample) => self.apply_success(request, sample, now),
            Err(_) => self.apply_failure(now),
        }
    }

    fn apply_success(
        &mut self,
        request: &ResourcePollRequest,
        sample: ResourceSampleSet,
        now: Instant,
    ) {
        let elapsed = self
            .baseline
            .as_ref()
            .map(|baseline| duration_nanoseconds(now.saturating_duration_since(baseline.at)))
            .unwrap_or(0);
        let empty = Vec::new();
        let previous_total = self
            .baseline
            .as_ref()
            .map(|baseline| &baseline.total)
            .unwrap_or(&empty);
        let previous_app = self
            .baseline
            .as_ref()
            .map(|baseline| &baseline.app)
            .unwrap_or(&empty);
        let previous_sessions = self
            .baseline
            .as_ref()
            .map(|baseline| &baseline.sessions)
            .unwrap_or(&empty);
        let total = aggregate_resources(previous_total, &sample.total, elapsed);
        let app = aggregate_resources(previous_app, &sample.app, elapsed);
        let sessions = aggregate_resources(previous_sessions, &sample.sessions, elapsed);
        self.snapshot.total = ResourceSnapshot {
            availability: ResourceAvailability::Live,
            totals: Some(total),
            sample_age_milliseconds: 0,
        };
        self.snapshot.app = Some(app);
        self.snapshot.sessions = Some(sessions);
        self.snapshot.session_roots = request.session_identities.clone();
        self.snapshot.session_processes = sample
            .sessions
            .iter()
            .map(|sample| sample.identity)
            .collect();
        self.snapshot.session_count = sample.session_count;
        self.last_success = Some(now);
        self.baseline = Some(ResourceBaseline {
            at: now,
            total: sample.total,
            app: sample.app,
            sessions: sample.sessions,
        });
    }

    fn apply_failure(&mut self, now: Instant) {
        let Some(last_success) = self.last_success else {
            self.snapshot.total.availability = ResourceAvailability::Unavailable;
            self.snapshot.total.totals = None;
            self.snapshot.app = None;
            self.snapshot.sessions = None;
            self.snapshot.session_roots.clear();
            self.snapshot.session_processes.clear();
            self.snapshot.session_count = 0;
            self.snapshot.total.sample_age_milliseconds = 0;
            return;
        };
        self.snapshot.total.availability = ResourceAvailability::Stale;
        self.snapshot.total.sample_age_milliseconds =
            u64::try_from(now.saturating_duration_since(last_success).as_millis())
                .unwrap_or(u64::MAX);
    }

    #[cfg(test)]
    fn with_app_identity(enabled: bool, app_identity: ProcessIdentity) -> Self {
        let mut monitor = Self::new(enabled);
        monitor.app_identity = Some(app_identity);
        monitor
    }
}

pub fn collect(request: &ResourcePollRequest) -> Result<ResourceSampleSet, String> {
    collect_from_records(
        request,
        &platform::process_records().map_err(|error| error.to_string())?,
    )
}

fn collect_from_records(
    request: &ResourcePollRequest,
    records: &[ProcessResourceRecord],
) -> Result<ResourceSampleSet, String> {
    let app_roots = [request.app_identity];
    let mut total_roots = Vec::with_capacity(request.session_identities.len() + 1);
    total_roots.push(request.app_identity);
    total_roots.extend(request.session_identities.iter().copied());
    let total = process_tree_resources(records, &total_roots)
        .ok_or_else(|| "resource root identity changed during sampling".to_owned())?;
    let app = process_tree_resources(records, &app_roots)
        .ok_or_else(|| "app resource root identity changed during sampling".to_owned())?;
    let sessions = process_tree_resources(records, &request.session_identities)
        .ok_or_else(|| "session resource root identity changed during sampling".to_owned())?;
    Ok(ResourceSampleSet {
        total,
        app,
        sessions,
        session_count: request.session_count,
    })
}

pub fn compact_label(snapshot: &ResourceMonitorSnapshot) -> String {
    let Some(totals) = snapshot.total.totals else {
        return "CPU -- · RAM --".to_owned();
    };
    let cpu = totals
        .cpu_percent
        .map(|percent| format!("{percent:.0}%"))
        .unwrap_or_else(|| "--".to_owned());
    let values = format!("CPU {cpu} · RAM {}", format_bytes(totals.resident_bytes));
    match snapshot.total.availability {
        ResourceAvailability::Live => values,
        ResourceAvailability::Stale => format!("Stale · {values}"),
        ResourceAvailability::Unavailable => "CPU -- · RAM --".to_owned(),
    }
}

pub fn tooltip_text(snapshot: &ResourceMonitorSnapshot) -> String {
    let state = match snapshot.total.availability {
        ResourceAvailability::Live => "Live".to_owned(),
        ResourceAvailability::Stale => {
            format!("Stale, {} ms old", snapshot.total.sample_age_milliseconds)
        }
        ResourceAvailability::Unavailable => "Unavailable".to_owned(),
    };
    let app = snapshot
        .app
        .map(format_totals)
        .unwrap_or_else(|| "Unavailable".to_owned());
    let sessions = snapshot
        .sessions
        .map(format_totals)
        .unwrap_or_else(|| "Unavailable".to_owned());
    format!(
        "{state}\nApp tree: {app}\nDaemon/session tree: {sessions}\nSessions: {}",
        snapshot.session_count
    )
}

fn format_totals(totals: ResourceTotals) -> String {
    let cpu = totals
        .cpu_percent
        .map(|percent| format!("{percent:.0}%"))
        .unwrap_or_else(|| "--".to_owned());
    format!(
        "CPU {cpu}, RAM {}, {} processes",
        format_bytes(totals.resident_bytes),
        totals.process_count
    )
}

fn format_bytes(bytes: u64) -> String {
    const MEBIBYTE: u64 = 1024 * 1024;
    const GIBIBYTE: u64 = 1024 * MEBIBYTE;
    if bytes >= GIBIBYTE {
        return format!("{:.1} GB", bytes as f64 / GIBIBYTE as f64);
    }
    format!("{} MB", bytes.saturating_add(MEBIBYTE / 2) / MEBIBYTE)
}

fn duration_nanoseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(process_id: u32, start_identity: u64) -> ProcessIdentity {
        ProcessIdentity {
            process_id,
            start_identity,
        }
    }

    fn record(
        process_id: u32,
        start_identity: u64,
        parent_process_id: u32,
        cpu_time_nanoseconds: u64,
        resident_bytes: u64,
    ) -> ProcessResourceRecord {
        ProcessResourceRecord {
            sample: ProcessResourceSample {
                identity: identity(process_id, start_identity),
                cpu_time_nanoseconds,
                resident_bytes,
            },
            parent_identity: (parent_process_id != 0)
                .then(|| identity(parent_process_id, u64::from(parent_process_id) * 10)),
        }
    }

    #[test]
    fn resource_monitor_aggregates_overlapping_app_daemon_shell_and_grandchild_once() {
        let app = identity(1, 10);
        let daemon = identity(2, 20);
        let shell = identity(3, 30);
        let start = Instant::now();
        let mut monitor = ResourceMonitor::with_app_identity(true, app);
        let request = monitor.request(vec![daemon, shell], 1).unwrap();
        let first = [
            record(1, 10, 0, 0, 10),
            record(2, 20, 1, 0, 20),
            record(3, 30, 2, 0, 30),
            record(4, 40, 3, 0, 40),
        ];
        monitor.apply(&request, collect_from_records(&request, &first), start);
        let second = [
            record(1, 10, 0, 500_000_000, 10),
            record(2, 20, 1, 500_000_000, 20),
            record(3, 30, 2, 500_000_000, 30),
            record(4, 40, 3, 500_000_000, 40),
        ];
        monitor.apply(
            &request,
            collect_from_records(&request, &second),
            start + Duration::from_secs(1),
        );
        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.total.availability, ResourceAvailability::Live);
        assert_eq!(snapshot.total.totals.unwrap().cpu_percent, Some(200.0));
        assert_eq!(snapshot.total.totals.unwrap().resident_bytes, 100);
        assert_eq!(snapshot.total.totals.unwrap().process_count, 4);
        assert_eq!(snapshot.sessions.unwrap().process_count, 3);
        assert_eq!(snapshot.session_count, 1);
    }

    #[test]
    fn resource_monitor_rejects_pid_reuse_and_reports_stale_without_false_zero() {
        let app = identity(1, 10);
        let start = Instant::now();
        let mut monitor = ResourceMonitor::with_app_identity(true, app);
        let request = monitor.request(Vec::new(), 0).unwrap();
        let records = [record(1, 10, 0, 100, 10)];
        monitor.apply(&request, collect_from_records(&request, &records), start);
        let reused = [record(1, 11, 0, 200, 20)];
        monitor.apply(
            &request,
            collect_from_records(&request, &reused),
            start + Duration::from_secs(2),
        );
        assert_eq!(
            monitor.snapshot().total.availability,
            ResourceAvailability::Stale
        );
        assert_eq!(monitor.snapshot().total.totals.unwrap().resident_bytes, 10);
        assert_eq!(monitor.snapshot().total.sample_age_milliseconds, 2000);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resource_monitor_retries_transient_app_identity_failure() {
        let mut monitor = ResourceMonitor::new(true);
        monitor.app_identity = None;
        assert!(monitor.request(Vec::new(), 0).is_some());
    }

    #[test]
    fn resource_monitor_disable_stops_requests_and_reenable_uses_fresh_baseline() {
        let app = identity(1, 10);
        let start = Instant::now();
        let mut monitor = ResourceMonitor::with_app_identity(true, app);
        let request = monitor.request(Vec::new(), 0).unwrap();
        let first = [record(1, 10, 0, 100, 10)];
        monitor.apply(&request, collect_from_records(&request, &first), start);
        monitor.set_enabled(false);
        assert!(monitor.request(Vec::new(), 0).is_none());
        assert!(monitor.snapshot().total.totals.is_none());
        monitor.set_enabled(true);
        let fresh = monitor.request(Vec::new(), 0).unwrap();
        let second = [record(1, 10, 0, 1_000_000_000, 20)];
        monitor.apply(
            &fresh,
            collect_from_records(&fresh, &second),
            start + Duration::from_secs(1),
        );
        assert_eq!(monitor.snapshot().total.totals.unwrap().cpu_percent, None);
        assert_eq!(monitor.snapshot().total.totals.unwrap().resident_bytes, 20);
    }

    #[test]
    fn resource_monitor_labels_expose_live_stale_and_unavailable_without_color() {
        let unavailable = ResourceMonitorSnapshot::default();
        assert_eq!(compact_label(&unavailable), "CPU -- · RAM --");
        let stale = ResourceMonitorSnapshot {
            total: ResourceSnapshot {
                availability: ResourceAvailability::Stale,
                totals: Some(ResourceTotals {
                    cpu_percent: Some(18.2),
                    resident_bytes: 742 * 1024 * 1024,
                    process_count: 4,
                }),
                sample_age_milliseconds: 1200,
            },
            app: None,
            sessions: None,
            session_roots: Vec::new(),
            session_processes: Vec::new(),
            session_count: 2,
            poll_count: 3,
        };
        assert_eq!(compact_label(&stale), "Stale · CPU 18% · RAM 742 MB");
        assert!(tooltip_text(&stale).contains("Stale, 1200 ms old"));
        assert!(tooltip_text(&stale).contains("Sessions: 2"));
    }
}
