//! Starting the local server away from this console, and checking where a
//! running one is.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::ExitStatusExt;
use std::path::Path;
use std::process::ExitStatus;
use std::ptr;

use muxy_protocol::transport::{self, LocalStream};
use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CREATE_NO_WINDOW, CreateProcessW, GetExitCodeProcess, INFINITE,
    OpenProcess, OpenProcessToken, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
    STARTUPINFOW, WaitForSingleObject,
};

use crate::{ClientError, ServerElsewhere};

/// A program started by [`spawn_detached`](super::spawn_detached).
#[derive(Debug)]
pub struct Detached(OwnedHandle);

impl Detached {
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.status(0)
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.status(INFINITE)?
            .ok_or_else(|| io::Error::other("the program is still running"))
    }

    fn status(&self, timeout: u32) -> io::Result<Option<ExitStatus>> {
        // SAFETY: the process handle is open.
        match unsafe { WaitForSingleObject(self.0.as_raw_handle(), timeout) } {
            WAIT_OBJECT_0 => {}
            WAIT_TIMEOUT => return Ok(None),
            _ => return Err(io::Error::last_os_error()),
        }
        let mut code = 0;
        // SAFETY: the process handle is open and `code` is a live u32.
        if unsafe { GetExitCodeProcess(self.0.as_raw_handle(), &raw mut code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Some(ExitStatus::from_raw(code)))
    }
}

/// Starts `program` with a hidden console, which the programs it starts
/// inherit, and outside the job of the terminal that runs this program, so
/// it outlives both. Its own console keeps that terminal's Ctrl+C from
/// reaching it. It inherits no handles, so it can't hold open the output
/// that someone reads from this program, and it runs in its own folder, so
/// it doesn't keep the caller's folder from being deleted. A job that
/// forbids leaving it still gets the program, which may then end with that
/// job.
pub(super) fn spawn_detached(program: &Path, arguments: &[OsString]) -> io::Result<Detached> {
    match create_process(
        program,
        arguments,
        CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB,
    ) {
        Err(error) if access_denied(&error) => create_process(program, arguments, CREATE_NO_WINDOW),
        result => result,
    }
}

fn create_process(program: &Path, arguments: &[OsString], flags: u32) -> io::Result<Detached> {
    let application = wide(program.as_os_str())?;
    let mut line = wide(&command_line(program.as_os_str(), arguments))?;
    let folder = program
        .parent()
        .map(|folder| wide(folder.as_os_str()))
        .transpose()?;
    let startup = STARTUPINFOW {
        cb: u32::try_from(size_of::<STARTUPINFOW>()).map_err(io::Error::other)?,
        ..STARTUPINFOW::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: the strings are live and NUL-terminated, the command line is
    // writable as CreateProcessW requires, and both structures are live.
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            line.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            0,
            flags,
            ptr::null(),
            folder.as_ref().map_or(ptr::null(), Vec::as_ptr),
            &raw const startup,
            &raw mut process,
        )
    };
    if created == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both handles are new and owned here.
    let (handle, _thread) = unsafe {
        (
            OwnedHandle::from_raw_handle(process.hProcess),
            OwnedHandle::from_raw_handle(process.hThread),
        )
    };
    Ok(Detached(handle))
}

/// The command line that `CommandLineToArgvW` and the C runtime split back
/// into `program` and `arguments`.
fn command_line(program: &OsStr, arguments: &[OsString]) -> OsString {
    let mut line = OsString::from("\"");
    line.push(program);
    line.push("\"");
    for argument in arguments {
        line.push(" ");
        line.push(quote(argument));
    }
    line
}

fn quote(argument: &OsStr) -> OsString {
    let text: Vec<u16> = argument.encode_wide().collect();
    let [space, tab, quote, backslash] = [b' ', b'\t', b'"', b'\\'].map(u16::from);
    if !text.is_empty() && !text.iter().any(|unit| [space, tab, quote].contains(unit)) {
        return argument.to_owned();
    }
    let mut quoted = vec![quote];
    let mut backslashes = 0;
    for unit in text {
        if unit == backslash {
            backslashes += 1;
        } else {
            if unit == quote {
                quoted.extend(std::iter::repeat_n(backslash, backslashes + 1));
            }
            backslashes = 0;
        }
        quoted.push(unit);
    }
    quoted.extend(std::iter::repeat_n(backslash, backslashes));
    quoted.push(quote);
    std::os::windows::ffi::OsStringExt::from_wide(&quoted)
}

/// `text` as a NUL-terminated wide string, refusing one with a NUL inside.
fn wide(text: &OsStr) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = text.encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a program, folder or argument contains NUL",
        ));
    }
    wide.push(0);
    Ok(wide)
}

/// Refuses the server at the other end of `stream` if it runs in another
/// Windows session or at another elevation than this program.
pub(super) fn check_server(stream: &LocalStream) -> Result<(), ClientError> {
    let ours = placement(std::process::id())?;
    let server = match placement(transport::peer_pid(stream)?) {
        Ok(server) => server,
        // A normal program may not look into an elevated one.
        Err(error) if access_denied(&error) && !ours.elevated => Placement {
            elevated: true,
            ..ours
        },
        Err(error) => return Err(error.into()),
    };
    compare(ours, server).map_err(ClientError::ServerElsewhere)
}

/// Where a process runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Placement {
    session: u32,
    elevated: bool,
}

fn compare(ours: Placement, server: Placement) -> Result<(), ServerElsewhere> {
    if server.session != ours.session {
        return Err(ServerElsewhere::Session);
    }
    match (ours.elevated, server.elevated) {
        (false, true) => Err(ServerElsewhere::Elevated),
        (true, false) => Err(ServerElsewhere::NotElevated),
        _ => Ok(()),
    }
}

fn placement(pid: u32) -> io::Result<Placement> {
    let mut session = 0;
    // SAFETY: `session` is a live u32.
    if unsafe { ProcessIdToSessionId(pid, &raw mut session) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Placement {
        session,
        elevated: elevated(pid)?,
    })
}

/// Whether process `pid` runs elevated. A normal program may not read an
/// elevated one's token, so a refusal counts as elevated.
fn elevated(pid: u32) -> io::Result<bool> {
    // SAFETY: OpenProcess has no memory arguments.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the handle is new and owned here.
    let process = unsafe { OwnedHandle::from_raw_handle(process) };
    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: the process handle is open and `token` is a live HANDLE.
    if unsafe { OpenProcessToken(process.as_raw_handle(), TOKEN_QUERY, &raw mut token) } == 0 {
        let error = io::Error::last_os_error();
        return if access_denied(&error) {
            Ok(true)
        } else {
            Err(error)
        };
    }
    // SAFETY: the handle is new and owned here.
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut elevation = TOKEN_ELEVATION::default();
    let size = u32::try_from(size_of::<TOKEN_ELEVATION>()).map_err(io::Error::other)?;
    let mut returned = 0;
    // SAFETY: the token is open, and the buffer is a live TOKEN_ELEVATION of
    // the size passed.
    let read = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenElevation,
            ptr::from_mut(&mut elevation).cast(),
            size,
            &raw mut returned,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(elevation.TokenIsElevated != 0)
}

fn access_denied(error: &io::Error) -> bool {
    error.raw_os_error().map(i32::cast_unsigned) == Some(ERROR_ACCESS_DENIED)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORMAL: Placement = Placement {
        session: 1,
        elevated: false,
    };
    const ELEVATED: Placement = Placement {
        session: 1,
        elevated: true,
    };

    #[test]
    fn only_a_server_in_the_same_session_at_the_same_elevation_is_used() {
        assert_eq!(compare(NORMAL, NORMAL), Ok(()));
        assert_eq!(compare(ELEVATED, ELEVATED), Ok(()));
        assert_eq!(
            compare(NORMAL, ELEVATED),
            Err(ServerElsewhere::Elevated),
            "a normal program must not drive administrator shells"
        );
        assert_eq!(compare(ELEVATED, NORMAL), Err(ServerElsewhere::NotElevated));
        let ssh = Placement {
            session: 0,
            ..ELEVATED
        };
        assert_eq!(
            compare(NORMAL, ssh),
            Err(ServerElsewhere::Session),
            "the session is reported before the elevation"
        );
        assert_eq!(compare(ssh, NORMAL), Err(ServerElsewhere::Session));
    }

    #[test]
    fn arguments_survive_the_command_line() {
        let line = |arguments: &[&str]| {
            command_line(
                OsStr::new(r"C:\Program Files\muxy.exe"),
                &arguments.iter().map(OsString::from).collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            line(&["--socket", r"C:\My Files\server.sock", ""]),
            r#""C:\Program Files\muxy.exe" --socket "C:\My Files\server.sock" """#
        );
        assert_eq!(
            line(&[r#"say "hi""#, r"C:\trailing\", r"C:\trailing space\"]),
            r#""C:\Program Files\muxy.exe" "say \"hi\"" C:\trailing\ "C:\trailing space\\""#
        );
        assert_eq!(
            line(&[r#"a\"b"#]),
            r#""C:\Program Files\muxy.exe" "a\\\"b""#
        );
    }

    #[test]
    fn a_detached_program_reports_how_it_exited() -> io::Result<()> {
        let system = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        let shell = Path::new(&system).join(r"System32\cmd.exe");
        let mut program = spawn_detached(&shell, &["/d".into(), "/c".into(), "exit 7".into()])?;
        assert_eq!(program.wait()?.code(), Some(7));
        assert_eq!(
            program.try_wait()?.and_then(|status| status.code()),
            Some(7)
        );
        assert!(spawn_detached(&shell.with_file_name("missing.exe"), &[]).is_err());
        Ok(())
    }

    #[test]
    fn a_server_in_this_process_passes_the_check() -> Result<(), ClientError> {
        let (ours, _theirs) = transport::socket_pair()?;
        check_server(&ours)
    }
}
