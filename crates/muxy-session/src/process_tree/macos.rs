use super::ProcessRecord;
use muxy_proto::session::ProcessIdentity;
use std::io;
use std::mem::{size_of, zeroed};

const PROC_ALL_PIDS: u32 = 1;

pub fn snapshot() -> io::Result<Vec<ProcessRecord>> {
    let bytes = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
    if bytes < 0 {
        return Err(io::Error::last_os_error());
    }
    let capacity = bytes as usize / size_of::<libc::pid_t>() + 128;
    let mut process_ids = vec![0 as libc::pid_t; capacity];
    let buffer_bytes = process_ids.len() * size_of::<libc::pid_t>();
    let written = unsafe {
        libc::proc_listpids(
            PROC_ALL_PIDS,
            0,
            process_ids.as_mut_ptr().cast(),
            i32::try_from(buffer_bytes).unwrap_or(i32::MAX),
        )
    };
    if written < 0 {
        return Err(io::Error::last_os_error());
    }
    process_ids.truncate(written as usize / size_of::<libc::pid_t>());
    Ok(process_ids
        .into_iter()
        .filter(|process_id| *process_id > 0)
        .filter_map(read_record)
        .collect())
}

fn read_record(process_id: libc::pid_t) -> Option<ProcessRecord> {
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
        return None;
    }
    let process_session_id = unsafe { libc::getsid(process_id) };
    if process_session_id < 0 {
        return None;
    }
    Some(ProcessRecord {
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
        tty_device: tty_device(info.e_tdev),
    })
}

fn tty_device(device: u32) -> u64 {
    if device == u32::MAX {
        0
    } else {
        u64::from(device)
    }
}

#[cfg(test)]
mod tests {
    use super::tty_device;

    #[test]
    fn no_device_sentinel_is_not_a_shared_tty_identity() {
        assert_eq!(tty_device(u32::MAX), 0);
        assert_eq!(tty_device(42), 42);
    }
}
