use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProcessIdentity {
    pub process_id: u32,
    pub start_identity: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessResourceSample {
    pub identity: ProcessIdentity,
    pub cpu_time_nanoseconds: u64,
    pub resident_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceTotals {
    pub cpu_percent: Option<f64>,
    pub resident_bytes: u64,
    pub process_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceAvailability {
    Live,
    Stale,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceSnapshot {
    pub availability: ResourceAvailability,
    pub totals: Option<ResourceTotals>,
    pub sample_age_milliseconds: u64,
}

pub fn aggregate_resources(
    previous: &[ProcessResourceSample],
    current: &[ProcessResourceSample],
    elapsed_nanoseconds: u64,
) -> ResourceTotals {
    let previous = deduplicate(previous);
    let current = deduplicate(current);
    let resident_bytes = current.values().map(|sample| sample.resident_bytes).sum();
    let mut cpu_delta = 0u64;
    let mut matched = false;
    let mut invalid_delta = false;
    for sample in current.values() {
        let Some(held) = previous.get(&sample.identity.process_id) else {
            continue;
        };
        if held.identity.start_identity != sample.identity.start_identity {
            continue;
        }
        if sample.cpu_time_nanoseconds < held.cpu_time_nanoseconds {
            invalid_delta = true;
            continue;
        }
        matched = true;
        cpu_delta =
            cpu_delta.saturating_add(sample.cpu_time_nanoseconds - held.cpu_time_nanoseconds);
    }
    let cpu_percent = (matched && !invalid_delta && elapsed_nanoseconds > 0)
        .then(|| cpu_delta as f64 / elapsed_nanoseconds as f64 * 100.0);
    ResourceTotals {
        cpu_percent,
        resident_bytes,
        process_count: current.len(),
    }
}

fn deduplicate(samples: &[ProcessResourceSample]) -> HashMap<u32, ProcessResourceSample> {
    let mut result: HashMap<u32, ProcessResourceSample> = HashMap::new();
    let mut conflicts = HashSet::new();
    for sample in samples {
        if conflicts.contains(&sample.identity.process_id) {
            continue;
        }
        match result.get(&sample.identity.process_id) {
            Some(held) if held.identity.start_identity != sample.identity.start_identity => {
                result.remove(&sample.identity.process_id);
                conflicts.insert(sample.identity.process_id);
            }
            Some(_) => {}
            None => {
                result.insert(sample.identity.process_id, *sample);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pid: u32, start: u64, cpu: u64, resident: u64) -> ProcessResourceSample {
        ProcessResourceSample {
            identity: ProcessIdentity {
                process_id: pid,
                start_identity: start,
            },
            cpu_time_nanoseconds: cpu,
            resident_bytes: resident,
        }
    }

    #[test]
    fn aggregation_deduplicates_overlapping_roots_and_sums_memory_once() {
        let previous = [sample(1, 10, 100, 20), sample(2, 20, 200, 30)];
        let current = [
            sample(1, 10, 200, 40),
            sample(1, 10, 200, 40),
            sample(2, 20, 400, 60),
        ];
        let totals = aggregate_resources(&previous, &current, 100);
        assert_eq!(totals.process_count, 2);
        assert_eq!(totals.resident_bytes, 100);
        assert_eq!(totals.cpu_percent, Some(300.0));
    }

    #[test]
    fn pid_start_mismatch_cannot_contribute_reused_process_cpu() {
        let totals = aggregate_resources(&[sample(7, 1, 900, 20)], &[sample(7, 2, 1000, 30)], 100);
        assert_eq!(totals.cpu_percent, None);
        assert_eq!(totals.resident_bytes, 30);
    }

    #[test]
    fn regressed_cpu_counter_reports_unavailable_not_zero() {
        let previous = [sample(1, 10, 200, 20), sample(2, 20, 100, 30)];
        let current = [sample(1, 10, 100, 20), sample(2, 20, 200, 30)];
        assert_eq!(
            aggregate_resources(&previous, &current, 100).cpu_percent,
            None
        );
    }

    #[test]
    fn no_baseline_or_elapsed_time_reports_cpu_unavailable_not_zero() {
        let current = [sample(1, 1, 10, 20)];
        assert_eq!(aggregate_resources(&[], &current, 100).cpu_percent, None);
        assert_eq!(aggregate_resources(&current, &current, 0).cpu_percent, None);
    }
}
