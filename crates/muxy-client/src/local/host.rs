//! Starts the server through a host: a copy of the app that lives as long as
//! the server.
//!
//! macOS holds the process that starts another one responsible for its privacy
//! permissions, such as Local Network. Once that process exits, macOS stops
//! honoring them, and terminal commands get "No route to host". The app quits
//! while its server keeps running, so the app starts a host instead. The host
//! is responsible for itself and is the app's own executable, so the server
//! keeps the app's permissions for as long as it runs.

use std::ffi::{CString, OsStr, OsString, c_char, c_int};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::{ptr, thread};

/// The first argument of an app started as a host. The server's arguments
/// follow.
pub const FLAG: &str = "--server-host";

/// Runs the server beside this executable with `arguments` and returns once it
/// exits. Anyone can start the app with [`FLAG`], so the host never runs a
/// program named by its caller: that program would get the app's permissions.
pub fn run(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let Ok(executable) = muxy_core::executable::current_path() else {
        return ExitCode::FAILURE;
    };
    let status = Command::new(server_beside(&executable))
        .args(arguments)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if status.is_ok_and(|status| status.success()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
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
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    #[test]
    fn host_gets_the_server_command_in_its_own_process_group() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let app = directory.path().join("app");
        let report = directory.path().join("report");
        std::fs::write(
            &app,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" \"$HOME\" \"$$\" \"$(ps -o pgid= -p $$ | tr -d ' ')\" > {}.tmp\nmv {0}.tmp {0}\n",
                report.display()
            ),
        )?;
        std::fs::set_permissions(&app, std::fs::Permissions::from_mode(0o755))?;
        spawn(&app, &["--socket".into(), "/tmp/server.sock".into()])?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while !report.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        let report = std::fs::read_to_string(report)?;
        let lines: Vec<&str> = report.lines().collect();
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(lines[..4], [FLAG, "--socket", "/tmp/server.sock", &home]);
        assert_eq!(lines[4], lines[5], "the host leads its own process group");
        Ok(())
    }

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
}
