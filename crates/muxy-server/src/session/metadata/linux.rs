use std::fs;
use std::os::unix::ffi::OsStrExt;

use muxy_protocol::ServerPath;

pub(super) fn foreground_member(group: u32) -> Option<(i32, String)> {
    let member = |pid| {
        let stat = fs::read(format!("/proc/{pid}/stat")).ok()?;
        let (process_group, name) = parse_stat(&stat)?;
        (process_group == group).then_some((i32::try_from(pid).ok()?, name))
    };
    if let Some(leader) = member(group) {
        return Some(leader);
    }
    // Pipelines can outlive their group leader. Exit/permission races are normal.
    let mut pids: Vec<u32> = fs::read_dir("/proc")
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .collect();
    pids.sort_unstable();
    pids.into_iter().find_map(member)
}

pub(super) fn process_directory(pid: i32) -> Option<ServerPath> {
    let path = fs::read_link(format!("/proc/{pid}/cwd")).ok()?;
    path.is_absolute()
        .then(|| ServerPath(path.as_os_str().as_bytes().to_vec()))
}

fn parse_stat(stat: &[u8]) -> Option<(u32, String)> {
    // comm may contain spaces, parentheses, newlines and non-UTF-8 bytes.
    let opening = stat.iter().position(|byte| *byte == b'(')? + 1;
    let end = stat.iter().rposition(|byte| *byte == b')')?;
    let name = String::from_utf8_lossy(stat.get(opening..end)?).into_owned();
    let mut fields = stat
        .get(end + 1..)?
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty());
    if matches!(fields.next()?, b"Z" | b"X" | b"x") {
        return None;
    }
    fields.next()?; // parent PID
    let group = std::str::from_utf8(fields.next()?).ok()?.parse().ok()?;
    Some((group, name))
}

pub(super) fn agent_provider(group: u32) -> Option<muxy_protocol::AgentProvider> {
    use std::io::Read;
    let mut pids: Vec<u32> = fs::read_dir("/proc")
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.parse().ok())
        .collect();
    pids.sort_unstable();
    pids.into_iter()
        .filter_map(|pid| {
            let stat = fs::read(format!("/proc/{pid}/stat")).ok()?;
            let (pgrp, name) = parse_stat(&stat)?;
            (pgrp == group).then_some((pid, name))
        })
        .take(128)
        .find_map(|(pid, name)| {
            let mut bytes = Vec::new();
            let _ = fs::File::open(format!("/proc/{pid}/cmdline"))
                .ok()?
                .take(64 * 1024)
                .read_to_end(&mut bytes);
            let args = bytes
                .split(|b| *b == 0)
                .take(64)
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .collect::<Vec<_>>();
            crate::detection::identify(&name, &args)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_names_do_not_shift_the_group_field_and_zombies_are_not_members() {
        assert_eq!(
            parse_stat(b"12 (a ) b\n\xff) S 1 42 42 0"),
            Some((42, "a ) b\n\u{fffd}".into()))
        );
        for stat in [
            b"12 (sleep) Z 1 42".as_slice(),
            b"12 (sleep) X 1 42",
            b"12 ()",
            b"bad",
        ] {
            assert_eq!(parse_stat(stat), None);
        }
    }

    #[test]
    fn exited_process_metadata_is_unavailable() {
        assert_eq!(process_directory(-1), None);
        assert_eq!(foreground_member(u32::MAX), None);
    }
}
