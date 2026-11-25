use super::{Result, error};
#[cfg(unix)]
use rustix::process::{Pid, Signal};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[cfg(unix)]
pub(super) fn stop(directory: &Path, device: u64, inode: u64) -> Result<()> {
    let expected = |path: &Path| {
        muxy_core::files::FileId::of_link(path)
            .is_ok_and(|id| id.device() == device && id.index() == inode)
    };
    let verified = || -> Result<PathBuf> {
        let canonical = directory.canonicalize().map_err(error)?;
        if expected(directory) && expected(&canonical) {
            Ok(canonical)
        } else {
            Err(error(
                "Worktree directory identity changed; files were preserved",
            ))
        }
    };
    let directory = verified()?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if verified()? != directory {
            return Err(error(
                "Worktree directory identity changed; files were preserved",
            ));
        }
        let processes: Vec<_> = directories()?
            .into_iter()
            .filter(|cwd| cwd.path.starts_with(&directory) && cwd.inside(device, inode))
            .map(|cwd| (cwd.pid, cwd.path))
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
            for signal in [Signal::TERM, Signal::KILL] {
                let still_here = directories()?.iter().any(|cwd| {
                    cwd.pid == pid && cwd.path.starts_with(&directory) && cwd.inside(device, inode)
                });
                if !still_here || identity(pid).as_ref() != Some(&before) {
                    break;
                }
                if let Err(cause) = signal_process(pid, signal)
                    && identity(pid).as_ref() == Some(&before)
                {
                    return Err(error(cause));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

#[cfg(unix)]
fn signal_process(pid: u32, signal: Signal) -> std::io::Result<()> {
    let pid = i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or_else(|| std::io::Error::other("invalid process ID"))?;
    Ok(rustix::process::kill_process(pid, signal)?)
}

#[cfg(unix)]
struct Cwd {
    pid: u32,
    path: PathBuf,
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    id: Option<(u64, u64)>,
}

#[cfg(unix)]
impl Cwd {
    fn inside(&self, device: u64, inode: u64) -> bool {
        use muxy_core::files::FileId;
        use rustix::fs::{Mode, OFlags};
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
        #[cfg(target_os = "linux")]
        let start = rustix::fs::open(format!("/proc/{}/cwd", self.pid), flags, Mode::empty());
        #[cfg(not(target_os = "linux"))]
        let start = rustix::fs::open(&self.path, flags | OFlags::NOFOLLOW, Mode::empty());
        let Ok(start) = start else {
            return false;
        };
        let mut current = std::fs::File::from(start);
        let id = |file: &std::fs::File| FileId::of(file).ok().map(|id| (id.device(), id.index()));
        #[cfg(not(target_os = "linux"))]
        if self.id.is_none() || id(&current) != self.id {
            return false;
        }
        for _ in 0..4096 {
            let Some(here) = id(&current) else {
                return false;
            };
            if here == (device, inode) {
                return true;
            }
            let Ok(parent) = rustix::fs::openat(&current, "..", flags, Mode::empty()) else {
                return false;
            };
            let parent = std::fs::File::from(parent);
            if id(&parent) == Some(here) {
                return false;
            }
            current = parent;
        }
        false
    }
}

#[cfg(target_os = "macos")]
fn directories() -> Result<Vec<Cwd>> {
    use std::os::unix::ffi::OsStrExt;
    let mut command = std::process::Command::new("/usr/sbin/lsof");
    command.args(["-n", "-P", "-a", "-d", "cwd", "-FpDin0"]);
    let bytes = super::command::capture(command)?;
    let (mut pid, mut device, mut inode) = (None, None, None);
    let mut result = Vec::new();
    for record in bytes.split(|b| *b == 0) {
        let record = record.strip_prefix(b"\n").unwrap_or(record);
        let text = || std::str::from_utf8(&record[1..]).ok();
        match record.first() {
            Some(b'p') => {
                pid = text().and_then(|s| s.parse().ok());
                (device, inode) = (None, None);
            }
            Some(b'D') => {
                device = text()
                    .and_then(|s| s.strip_prefix("0x"))
                    .and_then(|s| u64::from_str_radix(s, 16).ok());
            }
            Some(b'i') => inode = text().and_then(|s| s.parse().ok()),
            Some(b'n') => {
                if let Some(pid) = pid {
                    result.push(Cwd {
                        pid,
                        path: PathBuf::from(std::ffi::OsStr::from_bytes(&record[1..])),
                        id: device.zip(inode),
                    });
                }
            }
            _ => {}
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
fn directories() -> Result<Vec<Cwd>> {
    Ok(std::fs::read_dir("/proc")
        .map_err(error)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let pid = entry.file_name().to_str()?.parse().ok()?;
            Some(Cwd {
                pid,
                path: std::fs::read_link(entry.path().join("cwd")).ok()?,
                id: None,
            })
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

#[cfg(windows)]
pub(super) fn aside_path(directory: &Path, id: impl std::fmt::Display) -> Result<PathBuf> {
    let mut name = directory
        .file_name()
        .ok_or_else(|| error("Worktree folder has no name"))?
        .to_owned();
    name.push(format!(".muxy-removing-{id}"));
    Ok(directory.with_file_name(name))
}

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

#[cfg(windows)]
pub(super) fn restore(aside: &Path, directory: &Path) -> Result<()> {
    rename_retrying(aside, directory).map_err(error)
}

#[cfg(windows)]
pub(super) fn check_unused(directory: &Path, aside: &Path) -> Result<()> {
    set_aside(directory, aside)?;
    restore(aside, directory)
}

#[cfg(windows)]
pub(super) fn delete_set_aside(aside: &Path, device: u64, index: u64) -> std::io::Result<()> {
    let pinned = pin(aside, device, index)?;
    let mut git = None;
    for entry in std::fs::read_dir(aside)? {
        let entry = entry?;
        if muxy_core::paths::path_eq(Path::new(&entry.file_name()), Path::new(".git")) {
            git = Some(entry);
            continue;
        }
        remove_retrying(&entry.path(), Entry::of(entry.file_type()?))?;
    }
    if let Some(git) = git {
        remove_retrying(&git.path(), Entry::of(git.file_type()?))?;
    }
    drop(pinned);
    remove_retrying(aside, Entry::FolderLink)
}

#[cfg(windows)]
fn pin(path: &Path, device: u64, index: u64) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
        FILE_SHARE_WRITE, SYNCHRONIZE,
    };
    let folder = std::fs::OpenOptions::new()
        .access_mode(FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let attributes = folder.metadata()?.file_attributes();
    let identity = muxy_core::files::FileId::of(&folder)?;
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || attributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || identity.device() != device
        || identity.index() != index
    {
        return Err(std::io::Error::other(
            "the folder set aside for removal was replaced; it was kept",
        ));
    }
    Ok(folder)
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum Entry {
    Folder,
    FolderLink,
    File,
}

#[cfg(windows)]
impl Entry {
    fn of(file_type: std::fs::FileType) -> Self {
        use std::os::windows::fs::FileTypeExt;
        if file_type.is_symlink_dir() {
            Self::FolderLink
        } else if file_type.is_dir() {
            Self::Folder
        } else {
            Self::File
        }
    }
}

#[cfg(windows)]
fn remove_retrying(path: &Path, entry: Entry) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let removed = match entry {
            Entry::Folder => std::fs::remove_dir_all(path),
            Entry::FolderLink => std::fs::remove_dir(path),
            Entry::File => std::fs::remove_file(path),
        };
        match removed {
            Err(cause) if in_use(&cause) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            result => return result,
        }
    }
}

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
        let identity = muxy_core::files::FileId::of_link(&folder).unwrap();
        set_aside(&folder, &aside).unwrap();
        let deleted = delete_set_aside(&aside, identity.device(), identity.index());
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

    #[test]
    fn a_junction_swapped_in_for_the_set_aside_folder_is_not_followed() {
        let folder = folder();
        let elsewhere = folder.parent().unwrap().join("elsewhere");
        std::fs::create_dir(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("victim.txt"), "kept").unwrap();
        let identity = muxy_core::files::FileId::of_link(&folder).unwrap();
        let aside = aside_path(&folder, 5).unwrap();
        set_aside(&folder, &aside).unwrap();
        std::fs::rename(&aside, folder.parent().unwrap().join("moved")).unwrap();
        let status = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&aside)
            .arg(&elsewhere)
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        assert!(delete_set_aside(&aside, identity.device(), identity.index()).is_err());
        assert!(elsewhere.join("victim.txt").exists());
        std::fs::remove_dir(&aside).unwrap();
        std::fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_working_directory_keeps_the_folder_until_its_program_exits() {
        let folder = folder();
        let mut working = Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .current_dir(folder.join("nested"))
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
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

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod unix_tests {
    use super::*;

    #[test]
    fn processes_are_matched_to_a_folder_by_its_identity() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("muxy-cwd-{}", std::process::id()));
        let inner = root.join("worktree").join("deep");
        let other = root.join("other");
        std::fs::create_dir_all(&inner)?;
        std::fs::create_dir_all(&other)?;
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .current_dir(&inner)
            .spawn()?;
        let id = |path: &Path| {
            muxy_core::files::FileId::of_path(path).map(|id| (id.device(), id.index()))
        };
        let result = (|| -> std::io::Result<()> {
            let (device, inode) = id(&root.join("worktree"))?;
            let (other_device, other_inode) = id(&other)?;
            let pid = child.id();
            let deadline = Instant::now() + Duration::from_secs(10);
            let cwd = loop {
                let found = directories()
                    .map_err(std::io::Error::other)?
                    .into_iter()
                    .find(|cwd| cwd.pid == pid);
                if let Some(cwd) = found {
                    break cwd;
                }
                assert!(Instant::now() < deadline, "the child never showed up");
                std::thread::sleep(Duration::from_millis(50));
            };
            assert!(cwd.inside(device, inode));
            assert!(!cwd.inside(other_device, other_inode));
            Ok(())
        })();
        let _ = child.kill();
        let _ = child.wait();
        std::fs::remove_dir_all(&root)?;
        result
    }
}
