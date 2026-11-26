#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Identity {
    pub(crate) user: u32,
    pub(crate) terminal: u64,
    pub(crate) started: u64,
}

#[cfg(target_os = "macos")]
pub(crate) fn identity(pid: u32) -> Option<Identity> {
    use libproc::bsd_info::BSDInfo;
    const ZOMBIE: u32 = 5;
    let info = libproc::proc_pid::pidinfo::<BSDInfo>(i32::try_from(pid).ok()?, 0).ok()?;
    (info.pbi_pid == pid && info.pbi_status != ZOMBIE).then(|| Identity {
        user: info.pbi_uid,
        terminal: u64::from(info.e_tdev),
        started: info
            .pbi_start_tvsec
            .wrapping_mul(1_000_000)
            .wrapping_add(info.pbi_start_tvusec),
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn identity(pid: u32) -> Option<Identity> {
    use std::os::unix::fs::MetadataExt;
    let stat = std::fs::read(format!("/proc/{pid}/stat")).ok()?;
    let (terminal, started) = parse_stat(&stat)?;
    let user = std::fs::metadata(format!("/proc/{pid}")).ok()?.uid();
    Some(Identity {
        user,
        terminal,
        started,
    })
}

#[cfg(any(target_os = "linux", test))]
fn parse_stat(stat: &[u8]) -> Option<(u64, u64)> {
    let end = stat.iter().rposition(|byte| *byte == b')')?;
    let fields: Vec<&[u8]> = stat
        .get(end + 1..)?
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty())
        .collect();
    if matches!(*fields.first()?, b"Z" | b"X" | b"x") {
        return None;
    }
    let number = |index: usize| -> Option<u64> {
        std::str::from_utf8(fields.get(index)?).ok()?.parse().ok()
    };
    Some((number(4)?, number(19)?))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) fn identity(_pid: u32) -> Option<Identity> {
    None
}

#[cfg(unix)]
pub(crate) fn current_user() -> u32 {
    rustix::process::geteuid().as_raw()
}

#[cfg(not(unix))]
pub(crate) fn current_user() -> u32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_stat_fields_are_read_after_the_last_parenthesis() {
        let stat = b"42 (a ) b) S 1 42 42 34817 42 0 0 0 0 0 0 0 0 0 20 0 1 0 987654 0 0";
        assert_eq!(parse_stat(stat), Some((34817, 987_654)));
        assert_eq!(
            parse_stat(b"42 (sleep) Z 1 42 42 34817 42 0 0 0 0 0 0 0 0 0 20 0 1 0 9"),
            None
        );
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn a_running_process_is_owned_by_this_user_and_a_finished_one_is_gone() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();
        let running = identity(pid);
        let _ = child.kill();
        let _ = child.wait();
        let running = running.expect("identity of a running process");
        assert_eq!(running.user, current_user());
        assert_eq!(identity(pid), None);
    }
}
