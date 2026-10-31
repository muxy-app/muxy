//! Starts the server through a host: a copy of the app that lives as long as
//! the server.
//!
//! macOS holds the process that starts another one responsible for its privacy
//! permissions, such as Local Network. Once that process exits, macOS stops
//! honoring them, and terminal commands get "No route to host". The app quits
//! while its server keeps running, so the app starts a host instead. The host
//! is responsible for itself and is the app's own executable, so the server
//! keeps the app's permissions for as long as it runs.
//!
//! The host still belongs to the app, so macOS ends it, the server, and every
//! terminal once the app quits, unless the user allows Muxy under Background
//! App Activity. The host notes that stop so the app can explain it.

use signal_hook::consts::SIGTERM;
use signal_hook::iterator::SignalsInfo;
use signal_hook::iterator::exfiltrator::WithOrigin;
use std::ffi::{CString, OsStr, OsString, c_char, c_int};
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::{fs, ptr, thread};

/// The first argument of an app started as a host. The server's arguments
/// follow.
pub const FLAG: &str = "--server-host";

/// The macOS process that ends a quit app's processes when the app may not
/// run in the background.
const LOGIN_WINDOW: &str =
    "/System/Library/CoreServices/loginwindow.app/Contents/MacOS/loginwindow";

/// The file in the profile that records a stop by [`LOGIN_WINDOW`].
const BACKGROUND_STOP: &str = "stopped-in-background";

/// Runs the server beside this executable with `arguments` and returns once it
/// exits. Anyone can start the app with [`FLAG`], so the host never runs a
/// program named by its caller: that program would get the app's permissions.
pub fn run(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let Ok(executable) = muxy_core::executable::current_path() else {
        return ExitCode::FAILURE;
    };
    let Ok(mut server) = Command::new(server_beside(&executable))
        .args(arguments)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return ExitCode::FAILURE;
    };
    let _ = watch_background_stop();
    if server.wait().is_ok_and(|status| status.success()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Whether macOS stopped the server after the app quit, because the app may
/// not run in the background. Forgets that stop, so it is reported once.
///
/// loginwindow also ends the server when the user logs out or restarts,
/// and is replaced then, so only a stop by the one still running counts.
pub fn take_background_stop(profile: &Path) -> bool {
    let path = profile.join(BACKGROUND_STOP);
    let Ok(sender) = fs::read_to_string(&path) else {
        return false;
    };
    let _ = fs::remove_file(&path);
    sender
        .split_once(' ')
        .and_then(|(process, _)| process.parse().ok())
        .and_then(identity)
        .is_some_and(|running| running == sender)
}

/// Notes a SIGTERM from [`LOGIN_WINDOW`] in the profile, then ends the host as
/// that signal would have.
fn watch_background_stop() -> io::Result<()> {
    let mut signals = SignalsInfo::<WithOrigin>::new([SIGTERM])?;
    thread::Builder::new()
        .name("muxy-server-host-signals".into())
        .spawn(move || {
            if let Some(origin) = signals.forever().next() {
                if let Some(process) = origin.process
                    && sent_by_login_window(process.pid)
                    && let Ok(profile) = muxy_core::dirs::muxy_dir()
                {
                    let _ = note_background_stop(&profile, process.pid);
                }
                let _ = signal_hook::low_level::emulate_default_handler(SIGTERM);
            }
        })?;
    Ok(())
}

fn note_background_stop(profile: &Path, sender: libc::pid_t) -> io::Result<()> {
    let sender = identity(sender).ok_or_else(|| io::Error::other("the sender has exited"))?;
    fs::write(profile.join(BACKGROUND_STOP), sender)
}

/// A process's ID and start time, which no later process shares.
fn identity(process: libc::pid_t) -> Option<String> {
    // SAFETY: `proc_bsdinfo` is plain data, valid when zeroed.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = c_int::try_from(size_of_val(&info)).ok()?;
    // SAFETY: `info` is writable for `size` bytes.
    let read = unsafe {
        libc::proc_pidinfo(
            process,
            libc::PROC_PIDTBSDINFO,
            0,
            (&raw mut info).cast(),
            size,
        )
    };
    (read == size).then(|| {
        format!(
            "{process} {}.{:06}",
            info.pbi_start_tvsec, info.pbi_start_tvusec
        )
    })
}

fn sent_by_login_window(process: libc::pid_t) -> bool {
    executable_of(process).is_some_and(|path| path == Path::new(LOGIN_WINDOW))
}

fn executable_of(process: libc::pid_t) -> Option<PathBuf> {
    let mut path = vec![0_u8; usize::try_from(libc::PROC_PIDPATHINFO_MAXSIZE).ok()?];
    let capacity = u32::try_from(path.len()).ok()?;
    // SAFETY: `path` is writable for `capacity` bytes.
    let length = unsafe { libc::proc_pidpath(process, path.as_mut_ptr().cast(), capacity) };
    path.truncate(usize::try_from(length).ok().filter(|length| *length > 0)?);
    Some(PathBuf::from(OsString::from_vec(path)))
}

/// The app that can host `server`: this executable, when it runs from an app
/// bundle and `server` is the one beside it. Without a bundle there is no app
/// identity to keep.
pub(super) fn app_for(server: &Path) -> io::Result<Option<PathBuf>> {
    let executable = muxy_core::executable::current_path()?;
    Ok((muxy_core::bundle::containing(&executable).is_some()
        && server == server_beside(&executable))
    .then_some(executable))
}

fn server_beside(executable: &Path) -> PathBuf {
    executable.with_file_name("muxy-server")
}

/// Starts `app` as the host of its server, in its own process group and with
/// output discarded, like a server started directly.
pub(super) fn spawn(app: &Path, arguments: &[OsString]) -> io::Result<()> {
    let argv = c_strings(
        [app.as_os_str(), OsStr::new(FLAG)]
            .into_iter()
            .chain(arguments.iter().map(OsString::as_os_str)),
    )?;
    let environment = c_strings(std::env::vars_os().map(|(mut pair, value)| {
        pair.push("=");
        pair.push(value);
        pair
    }))?;
    let process = spawn_disclaimed(&argv, &environment)?;
    thread::Builder::new()
        .name("muxy-server-host-wait".into())
        .spawn(move || wait(process))?;
    Ok(())
}

fn spawn_disclaimed(argv: &[CString], environment: &[CString]) -> io::Result<libc::pid_t> {
    let mut attributes = Attributes::new()?;
    let mut actions = FileActions::new()?;
    let flags =
        libc::c_short::try_from(libc::POSIX_SPAWN_SETPGROUP | libc::POSIX_SPAWN_CLOEXEC_DEFAULT)
            .map_err(io::Error::other)?;
    // SAFETY: `attributes` and `actions` are initialized and outlive the calls.
    unsafe {
        check(libc::posix_spawnattr_setflags(&raw mut attributes.0, flags))?;
        check(libc::posix_spawnattr_setpgroup(&raw mut attributes.0, 0))?;
    }
    disclaim(&mut attributes)?;
    for (descriptor, access) in [
        (libc::STDIN_FILENO, libc::O_RDONLY),
        (libc::STDOUT_FILENO, libc::O_WRONLY),
        (libc::STDERR_FILENO, libc::O_WRONLY),
    ] {
        // SAFETY: `actions` is initialized and the path is a valid C string.
        check(unsafe {
            libc::posix_spawn_file_actions_addopen(
                &raw mut actions.0,
                descriptor,
                c"/dev/null".as_ptr(),
                access,
                0,
            )
        })?;
    }
    let argv_pointers = pointers(argv);
    let environment_pointers = pointers(environment);
    let mut process = 0;
    // SAFETY: both pointer arrays are null-terminated and point into strings
    // that outlive the call.
    check(unsafe {
        libc::posix_spawn(
            &raw mut process,
            argv_pointers[0],
            &raw const actions.0,
            &raw const attributes.0,
            argv_pointers.as_ptr(),
            environment_pointers.as_ptr(),
        )
    })?;
    Ok(process)
}

/// Makes the host responsible for itself. The call is private to macOS, as
/// Chromium and LLDB also use it, so it is looked up at run time. Without it
/// the host still starts the server, which then keeps this app process
/// responsible, as before.
fn disclaim(attributes: &mut Attributes) -> io::Result<()> {
    type Disclaim = unsafe extern "C" fn(*mut libc::posix_spawnattr_t, c_int) -> c_int;
    // SAFETY: the name is a valid C string.
    let symbol = unsafe {
        libc::dlsym(
            libc::RTLD_DEFAULT,
            c"responsibility_spawnattrs_setdisclaim".as_ptr(),
        )
    };
    if symbol.is_null() {
        return Ok(());
    }
    // SAFETY: the symbol has this signature on every macOS that exports it.
    let disclaim = unsafe { std::mem::transmute::<*mut libc::c_void, Disclaim>(symbol) };
    // SAFETY: `attributes` is initialized.
    check(unsafe { disclaim(&raw mut attributes.0, 1) })
}

fn wait(process: libc::pid_t) {
    let mut status = 0;
    // SAFETY: `process` is this process's child and `status` is writable.
    while unsafe { libc::waitpid(process, &raw mut status, 0) } == -1
        && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted
    {}
}

fn c_strings<T: AsRef<OsStr>>(values: impl IntoIterator<Item = T>) -> io::Result<Vec<CString>> {
    values
        .into_iter()
        .map(|value| Ok(CString::new(value.as_ref().as_bytes())?))
        .collect()
}

fn pointers(values: &[CString]) -> Vec<*mut c_char> {
    values
        .iter()
        .map(|value| value.as_ptr().cast_mut())
        .chain([ptr::null_mut()])
        .collect()
}

fn check(code: c_int) -> io::Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code))
    }
}

struct Attributes(libc::posix_spawnattr_t);

impl Attributes {
    fn new() -> io::Result<Self> {
        let mut attributes = ptr::null_mut();
        // SAFETY: `attributes` is writable.
        check(unsafe { libc::posix_spawnattr_init(&raw mut attributes) })?;
        Ok(Self(attributes))
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: initialized in `new` and destroyed once.
        unsafe { libc::posix_spawnattr_destroy(&raw mut self.0) };
    }
}

struct FileActions(libc::posix_spawn_file_actions_t);

impl FileActions {
    fn new() -> io::Result<Self> {
        let mut actions = ptr::null_mut();
        // SAFETY: `actions` is writable.
        check(unsafe { libc::posix_spawn_file_actions_init(&raw mut actions) })?;
        Ok(Self(actions))
    }
}

impl Drop for FileActions {
    fn drop(&mut self) {
        // SAFETY: initialized in `new` and destroyed once.
        unsafe { libc::posix_spawn_file_actions_destroy(&raw mut self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_never_runs_a_program_its_caller_names() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let marker = directory.path().join("ran");
        let command = format!("touch {}", marker.display());
        assert_eq!(
            run(["/bin/sh".into(), "-c".into(), command.into()]),
            ExitCode::FAILURE,
            "this test binary has no server beside it"
        );
        assert!(!marker.exists());
        Ok(())
    }

    #[test]
    fn background_stop_is_reported_once() -> io::Result<()> {
        let profile = tempfile::tempdir()?;
        assert!(!take_background_stop(profile.path()));
        note_background_stop(profile.path(), own_pid()?)?;
        assert!(take_background_stop(profile.path()));
        assert!(!take_background_stop(profile.path()));
        Ok(())
    }

    #[test]
    fn a_stop_by_an_exited_sender_is_forgotten() -> io::Result<()> {
        let profile = tempfile::tempdir()?;
        let marker = profile.path().join(BACKGROUND_STOP);
        let current = identity(own_pid()?).ok_or_else(|| io::Error::other("no identity"))?;
        let (process, _) = current
            .split_once(' ')
            .ok_or_else(|| io::Error::other("no start time"))?;
        fs::write(&marker, format!("{process} 0.000000"))?;
        assert!(!take_background_stop(profile.path()));
        assert!(!marker.exists());
        Ok(())
    }

    fn own_pid() -> io::Result<libc::pid_t> {
        libc::pid_t::try_from(std::process::id()).map_err(io::Error::other)
    }

    #[test]
    fn only_login_window_stops_count_as_background_stops() -> io::Result<()> {
        let own = own_pid()?;
        assert_eq!(
            executable_of(own),
            Some(std::env::current_exe()?.canonicalize()?)
        );
        assert!(!sent_by_login_window(own));
        Ok(())
    }
}
