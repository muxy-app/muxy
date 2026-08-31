use muxy_core::resources::{ProcessIdentity, ProcessResourceRecord, ProcessResourceSample};
use std::collections::HashSet;
use std::io;
use std::mem::{MaybeUninit, size_of, zeroed};

const PROC_UID_ONLY: u32 = 4;

pub fn current_process_identity() -> io::Result<ProcessIdentity> {
    basic_record(std::process::id() as libc::pid_t)?
        .map(|record| record.identity)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "current process identity not found",
            )
        })
}

pub fn process_records() -> io::Result<Vec<ProcessResourceRecord>> {
    let user_id = unsafe { libc::getuid() };
    let bytes = unsafe { libc::proc_listpids(PROC_UID_ONLY, user_id, std::ptr::null_mut(), 0) };
    if bytes < 0 {
        return Err(io::Error::last_os_error());
    }
    let capacity = bytes as usize / size_of::<libc::pid_t>() + 128;
    let mut process_ids = vec![0 as libc::pid_t; capacity];
    let buffer_bytes = process_ids
        .len()
        .checked_mul(size_of::<libc::pid_t>())
        .ok_or_else(|| io::Error::other("process list is too large"))?;
    let written = unsafe {
        libc::proc_listpids(
            PROC_UID_ONLY,
            user_id,
            process_ids.as_mut_ptr().cast(),
            i32::try_from(buffer_bytes)
                .map_err(|_| io::Error::other("process list is too large"))?,
        )
    };
    if written < 0 {
        return Err(io::Error::last_os_error());
    }
    if written as usize >= buffer_bytes {
        return Err(io::Error::other("process list changed during sampling"));
    }
    process_ids.truncate(written as usize / size_of::<libc::pid_t>());
    let mut seen = HashSet::new();
    let mut records = Vec::with_capacity(process_ids.len());
    for process_id in process_ids
        .into_iter()
        .filter(|process_id| *process_id > 0 && seen.insert(*process_id))
    {
        if let Some(record) = resource_record(process_id)? {
            records.push(record);
        }
    }
    Ok(records)
}

#[derive(Clone, Copy)]
struct BasicRecord {
    identity: ProcessIdentity,
    parent_process_id: u32,
}

fn resource_record(process_id: libc::pid_t) -> io::Result<Option<ProcessResourceRecord>> {
    let Some(before) = basic_record(process_id)? else {
        return Ok(None);
    };
    let parent_identity = if before.parent_process_id == 0 {
        None
    } else {
        basic_record(before.parent_process_id as libc::pid_t)
            .ok()
            .flatten()
            .map(|record| record.identity)
    };
    let mut usage = MaybeUninit::<libc::rusage_info_v2>::uninit();
    let result = unsafe {
        libc::proc_pid_rusage(process_id, libc::RUSAGE_INFO_V2, usage.as_mut_ptr().cast())
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ESRCH) || process_disappeared(process_id) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    let usage = unsafe { usage.assume_init() };
    let Some(after) = basic_record(process_id)? else {
        return Ok(None);
    };
    if before.identity != after.identity || before.parent_process_id != after.parent_process_id {
        return Ok(None);
    }
    if let Some(parent_identity) = parent_identity {
        let current_parent = basic_record(before.parent_process_id as libc::pid_t)
            .ok()
            .flatten()
            .map(|record| record.identity);
        if current_parent != Some(parent_identity) {
            return Ok(None);
        }
    }
    Ok(Some(ProcessResourceRecord {
        sample: ProcessResourceSample {
            identity: before.identity,
            cpu_time_nanoseconds: usage.ri_user_time.saturating_add(usage.ri_system_time),
            resident_bytes: usage.ri_resident_size,
        },
        parent_identity,
    }))
}

fn basic_record(process_id: libc::pid_t) -> io::Result<Option<BasicRecord>> {
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
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ESRCH) || process_disappeared(process_id) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    Ok(Some(BasicRecord {
        identity: ProcessIdentity {
            process_id: info.pbi_pid,
            start_identity: info
                .pbi_start_tvsec
                .saturating_mul(1_000_000)
                .saturating_add(info.pbi_start_tvusec),
        },
        parent_process_id: info.pbi_ppid,
    }))
}

fn process_disappeared(process_id: libc::pid_t) -> bool {
    if unsafe { libc::kill(process_id, 0) } == 0 {
        return false;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::resources::process_tree_resources;
    use std::process::{Child, Command};

    struct OwnedChild(Child);

    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn resource_monitor_macos_sample_contains_current_identity_and_resources() {
        let identity = current_process_identity().unwrap();
        let records = process_records().unwrap();
        let current = records
            .iter()
            .find(|record| record.sample.identity == identity)
            .unwrap();
        assert_ne!(identity.start_identity, 0);
        assert_ne!(current.sample.resident_bytes, 0);
    }

    #[test]
    fn resource_monitor_macos_process_tree_contains_owned_child_once() {
        let root = current_process_identity().unwrap();
        let child = OwnedChild(Command::new("/bin/sleep").arg("30").spawn().unwrap());
        let child_id = child.0.id();
        let records = process_records().unwrap();
        let samples = process_tree_resources(&records, &[root]).unwrap();
        assert_eq!(
            samples
                .iter()
                .filter(|sample| sample.identity.process_id == child_id)
                .count(),
            1
        );
    }
}
