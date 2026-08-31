#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

use muxy_proto::session::ProcessIdentity;
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessRecord {
    pub identity: ProcessIdentity,
    pub parent_process_id: u32,
    pub process_group_id: u32,
    pub process_session_id: u32,
    pub tty_device: u64,
}

#[derive(Debug)]
pub struct ProcessTracker {
    root: ProcessIdentity,
    root_group: u32,
    root_session: u32,
    root_tty: u64,
    tracked: HashSet<ProcessIdentity>,
}

impl ProcessTracker {
    pub fn new(
        root: ProcessIdentity,
        root_group: u32,
        root_session: u32,
        root_tty: u64,
    ) -> io::Result<Self> {
        if root.process_id != root_group || root.process_id != root_session || root_tty == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "PTY root identity is not isolated",
            ));
        }
        Ok(Self {
            root,
            root_group,
            root_session,
            root_tty,
            tracked: HashSet::from([root]),
        })
    }

    pub fn observe(&mut self) -> io::Result<()> {
        let records = snapshot()?;
        self.observe_records(&records);
        Ok(())
    }

    pub fn terminate_all(&mut self, grace: Duration) -> io::Result<()> {
        self.observe()?;
        self.signal_all(libc::SIGTERM)?;
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            self.reap_root();
            self.observe()?;
            if self.all_gone()? {
                return Ok(());
            }
            self.signal_all(libc::SIGTERM)?;
            std::thread::sleep(Duration::from_millis(25));
        }
        self.signal_all(libc::SIGKILL)?;
        let kill_deadline = Instant::now() + grace;
        while Instant::now() < kill_deadline {
            self.reap_root();
            self.observe()?;
            if self.all_gone()? {
                return Ok(());
            }
            self.signal_all(libc::SIGKILL)?;
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "session process tree did not terminate",
        ))
    }

    fn observe_records(&mut self, records: &[ProcessRecord]) {
        let daemon_pid = std::process::id();
        let mut known_pids: HashSet<u32> = self
            .tracked
            .iter()
            .filter(|identity| records.iter().any(|record| record.identity == **identity))
            .map(|identity| identity.process_id)
            .collect();
        let mut changed = true;
        while changed {
            changed = false;
            for record in records {
                let reused = self.tracked.iter().any(|identity| {
                    identity.process_id == record.identity.process_id
                        && *identity != record.identity
                });
                if record.identity.process_id == daemon_pid || reused {
                    continue;
                }
                let related = record.identity == self.root
                    || record.process_session_id == self.root_session
                    || record.process_group_id == self.root_group
                    || (self.root_tty != 0 && record.tty_device == self.root_tty)
                    || known_pids.contains(&record.parent_process_id);
                if related && self.tracked.insert(record.identity) {
                    known_pids.insert(record.identity.process_id);
                    changed = true;
                }
            }
        }
    }

    fn signal_all(&self, signal: libc::c_int) -> io::Result<()> {
        let records = snapshot()?;
        for identity in &self.tracked {
            if identity.process_id == std::process::id()
                || !records.iter().any(|record| record.identity == *identity)
            {
                continue;
            }
            if let Ok(pid) = libc::pid_t::try_from(identity.process_id) {
                let result = unsafe { libc::kill(pid, signal) };
                if result != 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::ESRCH) {
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    fn all_gone(&self) -> io::Result<bool> {
        let records = snapshot()?;
        Ok(self
            .tracked
            .iter()
            .all(|identity| !records.iter().any(|record| record.identity == *identity)))
    }

    fn reap_root(&self) {
        if let Ok(pid) = libc::pid_t::try_from(self.root.process_id) {
            let mut status = 0;
            unsafe {
                libc::waitpid(pid, &raw mut status, libc::WNOHANG);
            }
        }
    }
}

pub fn current_process_identity() -> io::Result<ProcessIdentity> {
    process_identity(std::process::id())
}

pub fn process_identity(process_id: u32) -> io::Result<ProcessIdentity> {
    snapshot()?
        .into_iter()
        .find(|record| record.identity.process_id == process_id)
        .map(|record| record.identity)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "process identity not found"))
}

pub fn identity_is_alive(identity: ProcessIdentity) -> bool {
    process_identity(identity.process_id)
        .is_ok_and(|current| current.start_identity == identity.start_identity)
}

pub fn record(process_id: u32) -> io::Result<ProcessRecord> {
    snapshot()?
        .into_iter()
        .find(|record| record.identity.process_id == process_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "process record not found"))
}

#[cfg(test)]
fn descendants_by_parent(records: &[ProcessRecord], root: u32) -> HashSet<u32> {
    let by_parent = records
        .iter()
        .fold(HashMap::<u32, Vec<u32>>::new(), |mut map, item| {
            map.entry(item.parent_process_id)
                .or_default()
                .push(item.identity.process_id);
            map
        });
    let mut descendants = HashSet::new();
    let mut pending = vec![root];
    while let Some(parent) = pending.pop() {
        if let Some(children) = by_parent.get(&parent) {
            for child in children {
                if descendants.insert(*child) {
                    pending.push(*child);
                }
            }
        }
    }
    descendants
}

#[cfg(target_os = "macos")]
pub fn snapshot() -> io::Result<Vec<ProcessRecord>> {
    macos::snapshot()
}

#[cfg(target_os = "linux")]
pub fn snapshot() -> io::Result<Vec<ProcessRecord>> {
    linux::snapshot()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn snapshot() -> io::Result<Vec<ProcessRecord>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "process inspection is unsupported",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_expansion_reaches_changed_process_groups() {
        let records = [
            ProcessRecord {
                identity: ProcessIdentity {
                    process_id: 10,
                    start_identity: 1,
                },
                parent_process_id: 1,
                process_group_id: 10,
                process_session_id: 10,
                tty_device: 4,
            },
            ProcessRecord {
                identity: ProcessIdentity {
                    process_id: 11,
                    start_identity: 2,
                },
                parent_process_id: 10,
                process_group_id: 11,
                process_session_id: 10,
                tty_device: 4,
            },
            ProcessRecord {
                identity: ProcessIdentity {
                    process_id: 12,
                    start_identity: 3,
                },
                parent_process_id: 11,
                process_group_id: 12,
                process_session_id: 12,
                tty_device: 0,
            },
        ];
        assert_eq!(descendants_by_parent(&records, 10), HashSet::from([11, 12]));
        let mut tracker = ProcessTracker::new(records[0].identity, 10, 10, 4).unwrap();
        tracker.observe_records(&records);
        assert!(tracker.tracked.contains(&records[2].identity));
    }

    #[test]
    fn reused_process_identifiers_are_never_adopted() {
        let root = ProcessRecord {
            identity: ProcessIdentity {
                process_id: 10,
                start_identity: 1,
            },
            parent_process_id: 1,
            process_group_id: 10,
            process_session_id: 10,
            tty_device: 4,
        };
        let child = ProcessRecord {
            identity: ProcessIdentity {
                process_id: 11,
                start_identity: 2,
            },
            parent_process_id: 10,
            process_group_id: 10,
            process_session_id: 10,
            tty_device: 4,
        };
        let mut tracker = ProcessTracker::new(root.identity, 10, 10, 4).unwrap();
        tracker.observe_records(&[root, child]);
        let reused = ProcessRecord {
            identity: ProcessIdentity {
                process_id: 11,
                start_identity: 3,
            },
            parent_process_id: 10,
            process_group_id: 10,
            process_session_id: 10,
            tty_device: 4,
        };
        tracker.observe_records(&[root, reused]);
        assert!(tracker.tracked.contains(&child.identity));
        assert!(!tracker.tracked.contains(&reused.identity));
    }

    #[test]
    fn current_process_identity_is_stable() {
        let first = current_process_identity().unwrap();
        let second = current_process_identity().unwrap();
        assert_eq!(first, second);
        assert!(identity_is_alive(first));
    }
}
