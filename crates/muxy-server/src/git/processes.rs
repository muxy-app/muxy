//! Making sure nothing uses a worktree before it is deleted.

use super::{Result, error};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Stops every process whose working directory is inside `directory`.
#[cfg(unix)]
pub(super) fn stop(directory: &Path) -> Result<()> {
    use super::command::capture;
    use std::process::Command;
    let directory = directory.canonicalize().map_err(error)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let processes: Vec<_> = directories()?
            .into_iter()
            .filter(|(_, cwd)| cwd.starts_with(&directory))
            .collect();
        if processes.is_empty() {
            return Ok(());
        }
        if processes
            .iter()
            .any(|(pid, _)| *pid <= 1 || *pid == std::process::id())
        {
            return Err(error(
                "The server is running from this worktree; files were preserved",
            ));
        }
        if Instant::now() >= deadline {
            return Err(error(
                "Processes are still using this worktree; files were preserved",
            ));
        }
        for (pid, _) in processes {
            let Some(before) = identity(pid) else {
                continue;
            };
            for signal in ["-TERM", "-KILL"] {
                let still_here = directories()?
                    .iter()
                    .any(|(current, cwd)| *current == pid && cwd.starts_with(&directory));
                if !still_here || identity(pid).as_ref() != Some(&before) {
                    break;
                }
                let mut command = Command::new("/bin/kill");
                command.args([signal, "--", &pid.to_string()]);
                if let Err(e) = capture(command)
                    && identity(pid).as_ref() == Some(&before)
                {
                    return Err(e);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn directories() -> Result<Vec<(u32, PathBuf)>> {
    use super::command::capture;
    use std::os::unix::ffi::OsStrExt;
    use std::process::Command;
    let mut command = Command::new("/usr/sbin/lsof");
    command.args(["-n", "-P", "-a", "-d", "cwd", "-Fpn0"]);
    let bytes = capture(command)?;
    let mut pid = None;
    let mut result = Vec::new();
    for record in bytes.split(|b| *b == 0) {
        let record = record.strip_prefix(b"\n").unwrap_or(record);
        if let Some(value) = record.strip_prefix(b"p") {
            pid = std::str::from_utf8(value).ok().and_then(|s| s.parse().ok());
        }
        if let Some(value) = record.strip_prefix(b"n")
            && let Some(pid) = pid
        {
            result.push((pid, PathBuf::from(std::ffi::OsStr::from_bytes(value))));
        }
    }
    Ok(result)
}
#[cfg(target_os = "macos")]
fn identity(pid: u32) -> Option<String> {
    use libproc::libproc::{bsd_info::BSDInfo, proc_pid::pidinfo};
    let info = pidinfo::<BSDInfo>(i32::try_from(pid).ok()?, 0).ok()?;
    Some(format!(
        "{}:{}",
        info.pbi_start_tvsec, info.pbi_start_tvusec
    ))
}
#[cfg(target_os = "linux")]
fn directories() -> Result<Vec<(u32, PathBuf)>> {
    Ok(std::fs::read_dir("/proc")
        .map_err(error)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let pid = entry.file_name().to_str()?.parse().ok()?;
            Some((pid, std::fs::read_link(entry.path().join("cwd")).ok()?))
        })
        .collect())
}
#[cfg(target_os = "linux")]
fn identity(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(19)
        .map(str::to_owned)
}

/// Where a removal sets `directory` aside: `<name>.muxy-removing-<id>`
/// beside it.
#[cfg(windows)]
pub(super) fn aside_path(directory: &Path, id: impl std::fmt::Display) -> Result<PathBuf> {
    let mut name = directory
        .file_name()
        .ok_or_else(|| error("Worktree folder has no name"))?
        .to_owned();
    name.push(format!(".muxy-removing-{id}"));
    Ok(directory.with_file_name(name))
}

/// Renames `directory` to `aside`. Windows can't read another process's
/// working directory, but it refuses this rename while a file in the folder
/// is open or a program works inside it.
#[cfg(windows)]
pub(super) fn set_aside(directory: &Path, aside: &Path) -> Result<()> {
    rename_retrying(directory, aside).map_err(|cause| {
        if in_use(&cause) {
            error("Processes are still using this worktree; files were preserved")
        } else {
            error(cause)
        }
    })
}

/// Puts a folder [`set_aside`] back.
#[cfg(windows)]
pub(super) fn restore(aside: &Path, directory: &Path) -> Result<()> {
    rename_retrying(aside, directory).map_err(error)
}

/// Fails, before anything changes, while a file in `directory` is open.
#[cfg(windows)]
pub(super) fn check_unused(directory: &Path, aside: &Path) -> Result<()> {
    set_aside(directory, aside)?;
    restore(aside, directory)
}

/// Deletes a folder [`set_aside`], leaving its `.git` file for last, so a
/// folder that must go back is still a worktree Git and Muxy can open.
#[cfg(windows)]
pub(super) fn delete_set_aside(aside: &Path) -> std::io::Result<()> {
    let mut git = None;
    for entry in std::fs::read_dir(aside)? {
        let entry = entry?;
        if muxy_core::paths::path_eq(Path::new(&entry.file_name()), Path::new(".git")) {
            git = Some(entry);
            continue;
        }
        remove_retrying(&entry.path(), entry.file_type()?.is_dir())?;
    }
    if let Some(git) = git {
        remove_retrying(&git.path(), git.file_type()?.is_dir())?;
    }
    remove_retrying(aside, true)
}

/// Retries for a moment while ended processes finish exiting.
#[cfg(windows)]
fn remove_retrying(path: &Path, directory: bool) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let removed = if directory {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        match removed {
            Err(cause) if in_use(&cause) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            result => return result,
        }
    }
}

/// Retries for a moment, since antivirus, indexing and sync tools open new
/// files briefly.
#[cfg(windows)]
fn rename_retrying(from: &Path, to: &Path) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match std::fs::rename(from, to) {
            Err(cause) if in_use(&cause) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            result => return result,
        }
    }
}

#[cfg(windows)]
fn in_use(error: &std::io::Error) -> bool {
    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION};
    error.raw_os_error().is_some_and(|code| {
        [ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION].contains(&code.cast_unsigned())
    })
}

#[cfg(all(test, windows))]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    fn folder() -> PathBuf {
        let folder = std::env::temp_dir()
            .join(format!("muxy-aside-{}", muxy_protocol::OperationId::new()))
            .join("worktree");
        std::fs::create_dir_all(folder.join("nested")).unwrap();
        folder
    }

    #[test]
    fn a_folder_is_set_aside_and_restored() {
        let folder = folder();
        std::fs::write(folder.join("nested/file.txt"), "kept").unwrap();
        let aside = aside_path(&folder, 1).unwrap();
        assert!(aside.ends_with("worktree.muxy-removing-1"));
        set_aside(&folder, &aside).unwrap();
        assert!(!folder.exists());
        restore(&aside, &folder).unwrap();
        assert_eq!(
            std::fs::read_to_string(folder.join("nested/file.txt")).unwrap(),
            "kept"
        );
        std::fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }

    #[test]
    fn an_open_file_even_one_that_allows_deletion_keeps_the_folder() {
        let folder = folder();
        // The standard library shares read, write and delete access.
        let open = std::fs::File::create(folder.join("nested/open.txt")).unwrap();
        let aside = aside_path(&folder, 2).unwrap();
        assert_eq!(
            check_unused(&folder, &aside).unwrap_err().message(),
            "Processes are still using this worktree; files were preserved"
        );
        assert!(folder.join("nested/open.txt").exists());
        drop(open);
        check_unused(&folder, &aside).unwrap();
        assert!(folder.join("nested/open.txt").exists());
        std::fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }

    /// A running program doesn't hold its folder either, but its file can't
    /// be deleted, so removal puts the folder back.
    #[test]
    fn a_running_programs_file_survives_deleting_its_folder() {
        let folder = folder();
        let system = std::env::var_os("SystemRoot").unwrap();
        let program = folder.join("copied-ping.exe");
        std::fs::copy(Path::new(&system).join(r"System32\PING.EXE"), &program).unwrap();
        let mut running = Command::new(&program)
            .args(["-n", "30", "127.0.0.1"])
            .current_dir(std::env::temp_dir())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        std::fs::write(folder.join(".git"), "gitdir: elsewhere").unwrap();
        let aside = aside_path(&folder, 4).unwrap();
        set_aside(&folder, &aside).unwrap();
        let deleted = delete_set_aside(&aside);
        restore(&aside, &folder).unwrap();
        running.kill().unwrap();
        running.wait().unwrap();
        assert!(deleted.is_err());
        assert!(folder.join("copied-ping.exe").exists());
        assert!(
            folder.join(".git").exists(),
            "the worktree's .git goes last"
        );
        std::fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }

    /// A running program holds its working directory, so removal ends the
    /// sessions that report one inside the folder first.
    #[test]
    fn a_working_directory_keeps_the_folder_until_its_program_exits() {
        let folder = folder();
        let mut working = Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .current_dir(folder.join("nested"))
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        // Its first words mean it has started, working directory and all.
        let mut output = BufReader::new(working.stdout.take().unwrap());
        let mut line = String::new();
        while line.trim().is_empty() {
            line.clear();
            assert_ne!(output.read_line(&mut line).unwrap(), 0);
        }
        let aside = aside_path(&folder, 6).unwrap();
        let busy = check_unused(&folder, &aside);
        working.kill().unwrap();
        working.wait().unwrap();
        assert_eq!(
            busy.unwrap_err().message(),
            "Processes are still using this worktree; files were preserved"
        );
        check_unused(&folder, &aside).unwrap();
        std::fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }
}
