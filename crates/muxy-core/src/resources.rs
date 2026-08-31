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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessResourceRecord {
    pub sample: ProcessResourceSample,
    pub parent_identity: Option<ProcessIdentity>,
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

pub fn process_tree_resources(
    records: &[ProcessResourceRecord],
    roots: &[ProcessIdentity],
) -> Option<Vec<ProcessResourceSample>> {
    let records = deduplicate_records(records)?;
    if roots.iter().any(|root| {
        records
            .get(&root.process_id)
            .is_none_or(|record| record.sample.identity != *root)
    }) {
        return None;
    }
    let mut selected = roots.iter().copied().collect::<HashSet<_>>();
    let mut changed = true;
    while changed {
        changed = false;
        for record in records.values() {
            if record
                .parent_identity
                .is_some_and(|parent| selected.contains(&parent))
                && selected.insert(record.sample.identity)
            {
                changed = true;
            }
        }
    }
    let mut samples = selected
        .into_iter()
        .filter_map(|identity| {
            records
                .get(&identity.process_id)
                .map(|record| record.sample)
        })
        .collect::<Vec<_>>();
    samples.sort_by_key(|sample| sample.identity.process_id);
    Some(samples)
}

pub fn aggregate_resources(
    previous: &[ProcessResourceSample],
    current: &[ProcessResourceSample],
    elapsed_nanoseconds: u64,
) -> ResourceTotals {
    let previous = deduplicate(previous);
    let current = deduplicate(current);
    let resident_bytes = current.values().fold(0u64, |total, sample| {
        total.saturating_add(sample.resident_bytes)
    });
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

fn deduplicate_records(
    records: &[ProcessResourceRecord],
) -> Option<HashMap<u32, ProcessResourceRecord>> {
    let mut result: HashMap<u32, ProcessResourceRecord> = HashMap::new();
    for record in records {
        match result.get(&record.sample.identity.process_id) {
            Some(held) if held != record => return None,
            Some(_) => {}
            None => {
                result.insert(record.sample.identity.process_id, *record);
            }
        }
    }
    Some(result)
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

    fn record(pid: u32, start: u64, parent: u32, cpu: u64, resident: u64) -> ProcessResourceRecord {
        ProcessResourceRecord {
            sample: sample(pid, start, cpu, resident),
            parent_identity: (parent != 0).then(|| ProcessIdentity {
                process_id: parent,
                start_identity: u64::from(parent) * 10,
            }),
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
    fn process_tree_expands_authenticated_overlapping_roots_once() {
        let records = [
            record(1, 10, 0, 100, 10),
            record(2, 20, 1, 200, 20),
            record(3, 30, 2, 300, 30),
            record(4, 40, 0, 400, 40),
        ];
        let samples = process_tree_resources(
            &records,
            &[records[0].sample.identity, records[1].sample.identity],
        )
        .unwrap();
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.identity.process_id)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            aggregate_resources(&samples, &samples, 1).resident_bytes,
            60
        );
    }

    #[test]
    fn process_tree_rejects_root_reuse_and_ambiguous_process_records() {
        let records = [record(7, 2, 0, 100, 10), record(8, 3, 7, 100, 10)];
        assert!(
            process_tree_resources(
                &records,
                &[ProcessIdentity {
                    process_id: 7,
                    start_identity: 1,
                }],
            )
            .is_none()
        );
        let ambiguous = [record(7, 1, 0, 100, 10), record(7, 2, 0, 100, 10)];
        assert!(process_tree_resources(&ambiguous, &[]).is_none());
        let conflicting_parent = [record(7, 1, 0, 100, 10), record(7, 1, 6, 100, 10)];
        assert!(process_tree_resources(&conflicting_parent, &[]).is_none());
        let conflicting_sample = [record(7, 1, 0, 100, 10), record(7, 1, 0, 101, 10)];
        assert!(process_tree_resources(&conflicting_sample, &[]).is_none());
        let reused_parent = [
            record(7, 1, 0, 100, 10),
            ProcessResourceRecord {
                sample: sample(8, 3, 100, 10),
                parent_identity: Some(ProcessIdentity {
                    process_id: 7,
                    start_identity: 2,
                }),
            },
        ];
        assert_eq!(
            process_tree_resources(&reused_parent, &[reused_parent[0].sample.identity]).unwrap(),
            [reused_parent[0].sample]
        );
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
