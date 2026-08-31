use super::policy::{ForegroundState, ProcessSafety, TerminalSafetyFacts};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProcessIdentity {
    pub process_id: u32,
    pub start_identity: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessRecord {
    pub identity: ProcessIdentity,
    pub parent_process_id: u32,
    pub process_group_id: u32,
    pub process_session_id: u32,
    pub user_id: u32,
    pub tty_device: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalProcessRoot {
    pub identity: ProcessIdentity,
    pub process_group_id: u32,
    pub process_session_id: u32,
    pub user_id: u32,
    pub tty_device: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalProcessSample {
    pub facts: TerminalSafetyFacts,
    pub root: Option<TerminalProcessRoot>,
}

pub trait ProcessSnapshot {
    fn records(&self) -> Option<Vec<ProcessRecord>>;
}

pub fn terminal_root_from_snapshot(
    terminal_owner_process_id: u32,
    foreground_process_id: u32,
    records: &[ProcessRecord],
) -> Option<TerminalProcessRoot> {
    if terminal_owner_process_id == 0 || foreground_process_id == 0 || has_duplicate_pids(records) {
        return None;
    }
    let terminal_user_id =
        record_by_pid(records, terminal_owner_process_id).map(|record| record.user_id);
    let mut current = foreground_process_id;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        let Some(record) = record_by_pid(records, current) else {
            break;
        };
        if record.parent_process_id == terminal_owner_process_id {
            return Some(root_from_record(record));
        }
        let parent = record_by_pid(records, record.parent_process_id);
        if terminal_user_id == Some(record.user_id)
            && (parent.is_some_and(|parent| parent.user_id != record.user_id)
                || (parent.is_none()
                    && record.parent_process_id == record.process_session_id
                    && record.tty_device != 0))
        {
            return Some(root_from_record(record));
        }
        if record.parent_process_id == 0 || record.parent_process_id == current {
            break;
        }
        current = record.parent_process_id;
    }
    let foreground = record_by_pid(records, foreground_process_id)?;
    let session_root = record_by_pid(records, foreground.process_session_id)?;
    (foreground.tty_device != 0 && session_root.tty_device == foreground.tty_device)
        .then(|| root_from_record(session_root))
}

pub fn safety_from_snapshot(
    root: TerminalProcessRoot,
    foreground_process_id: u32,
    records: Option<&[ProcessRecord]>,
    alternate_screen: bool,
) -> TerminalSafetyFacts {
    let Some(records) = records else {
        return TerminalSafetyFacts::unknown_with_alternate_screen(alternate_screen);
    };
    if has_duplicate_pids(records) {
        return TerminalSafetyFacts::unknown_with_alternate_screen(alternate_screen);
    }
    let Some(root_record) = record_by_pid(records, root.identity.process_id) else {
        return if foreground_process_id == root.identity.process_id {
            ended_process_facts(alternate_screen)
        } else {
            TerminalSafetyFacts::unknown_with_alternate_screen(alternate_screen)
        };
    };
    if root_record.identity != root.identity
        || root_record.process_group_id != root.process_group_id
        || root_record.process_session_id != root.process_session_id
        || root_record.user_id != root.user_id
        || root_record.tty_device != root.tty_device
    {
        return TerminalSafetyFacts::unknown_with_alternate_screen(alternate_screen);
    }
    let Some(foreground_record) = record_by_pid(records, foreground_process_id) else {
        return TerminalSafetyFacts::unknown_with_alternate_screen(alternate_screen);
    };
    let related = related_processes(root, records);
    if !related.contains(&foreground_record.identity) {
        return TerminalSafetyFacts::unknown_with_alternate_screen(alternate_screen);
    }
    TerminalSafetyFacts {
        foreground: if foreground_record.identity == root.identity {
            ForegroundState::Idle
        } else {
            ForegroundState::Busy
        },
        process_safety: if related.len() == 1 {
            ProcessSafety::SafeToLoseOrdinaryShell
        } else {
            ProcessSafety::Unsafe
        },
        alternate_screen,
    }
}

#[cfg(target_os = "macos")]
pub fn sample_terminal_safety(
    terminal_owner_process_id: u32,
    foreground_process_id: u32,
    expected_root: Option<TerminalProcessRoot>,
    alternate_screen: bool,
) -> TerminalProcessSample {
    let snapshot = MacProcessSnapshot {
        terminal_owner_process_id,
        foreground_process_id,
    };
    let Some(records) = snapshot.records() else {
        return unknown_sample(expected_root, alternate_screen);
    };
    let root = match expected_root {
        Some(root) => root,
        None => {
            let Some(root) = terminal_root_from_snapshot(
                terminal_owner_process_id,
                foreground_process_id,
                &records,
            ) else {
                if record_by_pid(&records, foreground_process_id).is_none() {
                    return TerminalProcessSample {
                        facts: ended_process_facts(alternate_screen),
                        root: None,
                    };
                }
                return unknown_sample(None, alternate_screen);
            };
            root
        }
    };
    let root = if record_by_pid(&records, root.identity.process_id).is_none()
        && foreground_process_id != root.identity.process_id
    {
        replacement_root(root, &records).unwrap_or(root)
    } else {
        root
    };
    let facts = safety_from_snapshot(
        root,
        foreground_process_id,
        Some(&records),
        alternate_screen,
    );
    TerminalProcessSample {
        facts,
        root: Some(root),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn sample_terminal_safety(
    _terminal_owner_process_id: u32,
    _foreground_process_id: u32,
    expected_root: Option<TerminalProcessRoot>,
    alternate_screen: bool,
) -> TerminalProcessSample {
    unknown_sample(expected_root, alternate_screen)
}

fn unknown_sample(
    root: Option<TerminalProcessRoot>,
    alternate_screen: bool,
) -> TerminalProcessSample {
    TerminalProcessSample {
        facts: TerminalSafetyFacts::unknown_with_alternate_screen(alternate_screen),
        root,
    }
}

fn ended_process_facts(alternate_screen: bool) -> TerminalSafetyFacts {
    TerminalSafetyFacts {
        foreground: ForegroundState::Idle,
        process_safety: ProcessSafety::SafeToLoseOrdinaryShell,
        alternate_screen,
    }
}

fn root_from_record(record: &ProcessRecord) -> TerminalProcessRoot {
    TerminalProcessRoot {
        identity: record.identity,
        process_group_id: record.process_group_id,
        process_session_id: record.process_session_id,
        user_id: record.user_id,
        tty_device: record.tty_device,
    }
}

fn record_by_pid(records: &[ProcessRecord], process_id: u32) -> Option<&ProcessRecord> {
    records
        .iter()
        .find(|record| record.identity.process_id == process_id)
}

fn has_duplicate_pids(records: &[ProcessRecord]) -> bool {
    let mut process_ids = HashSet::new();
    records
        .iter()
        .any(|record| !process_ids.insert(record.identity.process_id))
}

#[cfg(any(target_os = "macos", test))]
fn replacement_root(
    root: TerminalProcessRoot,
    records: &[ProcessRecord],
) -> Option<TerminalProcessRoot> {
    let related = related_processes(root, records);
    let related_process_ids = related
        .iter()
        .map(|identity| identity.process_id)
        .collect::<HashSet<_>>();
    let mut candidates = records.iter().filter(|record| {
        related.contains(&record.identity)
            && !related_process_ids.contains(&record.parent_process_id)
    });
    let replacement = candidates.next()?;
    candidates
        .next()
        .is_none()
        .then(|| root_from_record(replacement))
}

fn related_processes(
    root: TerminalProcessRoot,
    records: &[ProcessRecord],
) -> HashSet<ProcessIdentity> {
    let mut related = records
        .iter()
        .filter(|record| {
            record.user_id == root.user_id
                && (record.identity == root.identity
                    || (root.process_group_id != 0
                        && record.process_group_id == root.process_group_id)
                    || (root.process_session_id != 0
                        && record.process_session_id == root.process_session_id)
                    || (root.tty_device != 0 && record.tty_device == root.tty_device))
        })
        .map(|record| record.identity)
        .collect::<HashSet<_>>();
    let mut changed = true;
    while changed {
        changed = false;
        let related_pids = related
            .iter()
            .map(|identity| identity.process_id)
            .collect::<HashSet<_>>();
        for record in records {
            if related_pids.contains(&record.parent_process_id) && related.insert(record.identity) {
                changed = true;
            }
        }
    }
    related
}

#[cfg(target_os = "macos")]
struct MacProcessSnapshot {
    terminal_owner_process_id: u32,
    foreground_process_id: u32,
}

#[cfg(target_os = "macos")]
impl ProcessSnapshot for MacProcessSnapshot {
    fn records(&self) -> Option<Vec<ProcessRecord>> {
        use std::mem::size_of;

        const PROC_UID_ONLY: u32 = 4;
        let user_id = unsafe { libc::getuid() };
        let bytes = unsafe { libc::proc_listpids(PROC_UID_ONLY, user_id, std::ptr::null_mut(), 0) };
        if bytes < 0 {
            return None;
        }
        let capacity = bytes as usize / size_of::<libc::pid_t>() + 128;
        let mut process_ids = vec![0 as libc::pid_t; capacity];
        let buffer_bytes = process_ids.len().checked_mul(size_of::<libc::pid_t>())?;
        let written = unsafe {
            libc::proc_listpids(
                PROC_UID_ONLY,
                user_id,
                process_ids.as_mut_ptr().cast(),
                i32::try_from(buffer_bytes).ok()?,
            )
        };
        if written < 0 || written as usize >= buffer_bytes {
            return None;
        }
        process_ids.truncate(written as usize / size_of::<libc::pid_t>());
        let mut records = Vec::with_capacity(process_ids.len());
        for process_id in process_ids.into_iter().filter(|process_id| *process_id > 0) {
            if let Some(record) = mac_process_record(process_id).ok()? {
                records.push(record);
            }
        }
        let mut pending = vec![self.terminal_owner_process_id, self.foreground_process_id];
        let mut visited = HashSet::new();
        while let Some(process_id) = pending.pop() {
            if process_id == 0 || !visited.insert(process_id) {
                continue;
            }
            let record = match record_by_pid(&records, process_id).copied() {
                Some(record) => record,
                None => {
                    let Ok(Some(record)) = mac_process_record(process_id as libc::pid_t) else {
                        continue;
                    };
                    records.push(record);
                    record
                }
            };
            pending.push(record.parent_process_id);
            pending.push(record.process_session_id);
        }
        Some(records)
    }
}

#[cfg(target_os = "macos")]
fn mac_process_record(process_id: libc::pid_t) -> Result<Option<ProcessRecord>, ()> {
    use std::mem::{size_of, zeroed};

    let mut info: libc::proc_bsdinfo = unsafe { zeroed() };
    let size = unsafe {
        libc::proc_pidinfo(
            process_id,
            libc::PROC_PIDTBSDINFO,
            0,
            (&raw mut info).cast(),
            size_of::<libc::proc_bsdinfo>() as i32,
        )
    };
    if size as usize != size_of::<libc::proc_bsdinfo>() {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ESRCH) || process_disappeared(process_id) {
            Ok(None)
        } else {
            Err(())
        };
    }
    let process_session_id = unsafe { libc::getsid(process_id) };
    if process_session_id < 0 {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ESRCH) || process_disappeared(process_id) {
            Ok(None)
        } else {
            Err(())
        };
    }
    Ok(Some(ProcessRecord {
        identity: ProcessIdentity {
            process_id: info.pbi_pid,
            start_identity: info
                .pbi_start_tvsec
                .saturating_mul(1_000_000)
                .saturating_add(info.pbi_start_tvusec),
        },
        parent_process_id: info.pbi_ppid,
        process_group_id: info.pbi_pgid,
        process_session_id: process_session_id as u32,
        user_id: info.pbi_uid,
        tty_device: tty_device(info.e_tdev),
    }))
}

#[cfg(target_os = "macos")]
fn process_disappeared(process_id: libc::pid_t) -> bool {
    if unsafe { libc::kill(process_id, 0) } == 0 {
        return false;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

#[cfg(target_os = "macos")]
fn tty_device(device: u32) -> u64 {
    if device == u32::MAX {
        0
    } else {
        u64::from(device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        process_id: u32,
        start_identity: u64,
        parent_process_id: u32,
        process_group_id: u32,
        process_session_id: u32,
        tty_device: u64,
    ) -> ProcessRecord {
        ProcessRecord {
            identity: ProcessIdentity {
                process_id,
                start_identity,
            },
            parent_process_id,
            process_group_id,
            process_session_id,
            user_id: 501,
            tty_device,
        }
    }

    fn root() -> TerminalProcessRoot {
        TerminalProcessRoot {
            identity: ProcessIdentity {
                process_id: 10,
                start_identity: 1,
            },
            process_group_id: 10,
            process_session_id: 10,
            user_id: 501,
            tty_device: 4,
        }
    }

    #[test]
    fn offline_process_root_is_resolved_from_foreground_ancestry() {
        let records = [
            record(10, 1, 1, 10, 10, 4),
            record(11, 2, 10, 11, 10, 4),
            record(12, 3, 11, 12, 10, 4),
        ];
        assert_eq!(terminal_root_from_snapshot(1, 10, &records), Some(root()));
        assert_eq!(terminal_root_from_snapshot(1, 12, &records), Some(root()));
        assert_eq!(terminal_root_from_snapshot(2, 12, &records), Some(root()));
        assert_eq!(
            terminal_root_from_snapshot(
                1,
                12,
                &[record(12, 3, 11, 12, 10, 4), record(11, 2, 12, 11, 10, 4),],
            ),
            None
        );
    }

    #[test]
    fn offline_process_root_uses_shell_below_an_unavailable_session_launcher() {
        let records = [
            record(1, 1, 0, 1, 1, 0),
            record(11, 2, 10, 11, 10, 4),
            record(12, 3, 11, 12, 10, 4),
        ];
        assert_eq!(
            terminal_root_from_snapshot(1, 12, &records),
            Some(root_from_record(&records[1]))
        );
    }

    #[test]
    fn offline_process_root_skips_a_different_user_session_launcher() {
        let mut launcher = record(10, 1, 1, 10, 10, 4);
        launcher.user_id = 0;
        let records = [
            record(1, 1, 0, 1, 1, 0),
            launcher,
            record(11, 2, 10, 11, 10, 4),
            record(12, 3, 11, 12, 10, 4),
        ];
        assert_eq!(
            terminal_root_from_snapshot(1, 12, &records),
            Some(TerminalProcessRoot {
                identity: ProcessIdentity {
                    process_id: 11,
                    start_identity: 2,
                },
                process_group_id: 11,
                process_session_id: 10,
                user_id: 501,
                tty_device: 4,
            })
        );
    }

    #[test]
    fn offline_process_safety_distinguishes_idle_foreground_and_related_processes() {
        let idle = safety_from_snapshot(root(), 10, Some(&[record(10, 1, 1, 10, 10, 4)]), false);
        assert_eq!(idle.foreground, ForegroundState::Idle);
        assert_eq!(idle.process_safety, ProcessSafety::SafeToLoseOrdinaryShell);

        let foreground_child = safety_from_snapshot(
            root(),
            11,
            Some(&[record(10, 1, 1, 10, 10, 4), record(11, 2, 10, 11, 10, 4)]),
            false,
        );
        assert_eq!(foreground_child.foreground, ForegroundState::Busy);
        assert_eq!(foreground_child.process_safety, ProcessSafety::Unsafe);

        let reparented_background = safety_from_snapshot(
            root(),
            10,
            Some(&[record(10, 1, 1, 10, 10, 4), record(12, 3, 1, 12, 10, 4)]),
            false,
        );
        assert_eq!(reparented_background.foreground, ForegroundState::Idle);
        assert_eq!(reparented_background.process_safety, ProcessSafety::Unsafe);
    }

    #[test]
    fn offline_process_replaces_an_ended_launcher_with_the_unique_surviving_shell() {
        let shell = record(11, 2, 10, 11, 10, 4);
        let child = record(12, 3, 11, 12, 10, 4);
        assert_eq!(
            replacement_root(root(), &[shell]),
            Some(root_from_record(&shell))
        );
        assert_eq!(
            replacement_root(root(), &[shell, child]),
            Some(root_from_record(&shell))
        );
        let reparented = record(13, 4, 1, 13, 10, 4);
        assert_eq!(replacement_root(root(), &[shell, reparented]), None);
    }

    #[test]
    fn offline_process_ended_root_is_safe_but_reused_identity_is_unknown() {
        assert_eq!(
            safety_from_snapshot(root(), 10, Some(&[]), false),
            TerminalSafetyFacts {
                foreground: ForegroundState::Idle,
                process_safety: ProcessSafety::SafeToLoseOrdinaryShell,
                alternate_screen: false,
            }
        );
        assert_eq!(
            safety_from_snapshot(root(), 10, Some(&[record(10, 2, 1, 10, 10, 4)]), false,),
            TerminalSafetyFacts::unknown_with_alternate_screen(false)
        );
        assert_eq!(
            safety_from_snapshot(root(), 11, Some(&[]), false),
            TerminalSafetyFacts::unknown_with_alternate_screen(false)
        );
    }

    #[test]
    fn offline_process_safety_fails_awake_for_missing_incomplete_or_reused_samples() {
        assert_eq!(
            safety_from_snapshot(root(), 10, None, true),
            TerminalSafetyFacts::unknown_with_alternate_screen(true)
        );
        assert_eq!(
            safety_from_snapshot(root(), 10, Some(&[record(10, 2, 1, 10, 10, 4)]), false,),
            TerminalSafetyFacts::unknown_with_alternate_screen(false)
        );
        assert_eq!(
            safety_from_snapshot(
                root(),
                10,
                Some(&[record(10, 1, 1, 10, 10, 4), record(10, 2, 1, 10, 10, 4),]),
                false,
            ),
            TerminalSafetyFacts::unknown_with_alternate_screen(false)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_process_snapshot_contains_the_current_immutable_identity() {
        let process_id = std::process::id();
        let records = MacProcessSnapshot {
            terminal_owner_process_id: process_id,
            foreground_process_id: process_id,
        }
        .records()
        .unwrap();
        let current = record_by_pid(&records, std::process::id()).unwrap();
        assert_ne!(current.identity.start_identity, 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn no_device_sentinel_is_not_a_shared_tty_identity() {
        assert_eq!(tty_device(u32::MAX), 0);
        assert_eq!(tty_device(42), 42);
    }
}
