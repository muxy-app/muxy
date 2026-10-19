//! The Win32 calls behind job objects and process listings on Windows.

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::ptr;

use windows_sys::Wdk::System::Threading::{
    NtQueryInformationProcess, ProcessBasicInformation, ProcessCommandLineInformation,
};
use windows_sys::Win32::Foundation::{
    FILETIME, HANDLE, INVALID_HANDLE_VALUE, RtlNtStatusToDosError, UNICODE_STRING,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_PROCESS_ID_LIST, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, GetProcessTimes, OpenProcess, OpenThread,
    PROCESS_BASIC_INFORMATION, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, ResumeThread, THREAD_SUSPEND_RESUME,
};

/// A job object. The processes its members start join it too.
#[derive(Debug)]
pub(crate) struct Job(OwnedHandle);

/// A process in a [`Job`]. Its open handle keeps its ID from being reused,
/// so the details read later belong to the same process.
#[derive(Debug)]
pub(crate) struct JobProcess {
    pub(crate) pid: u32,
    pub(crate) parent: u32,
    /// When it started, in 100 ns units since 1601.
    pub(crate) created: u64,
    handle: OwnedHandle,
}

impl JobProcess {
    pub(crate) fn image(&self) -> Option<PathBuf> {
        image_path(&self.handle)
    }

    pub(crate) fn command_line(&self) -> Option<String> {
        command_line(&self.handle)
    }
}

impl Job {
    /// A job whose processes keep running when it is dropped.
    pub(crate) fn new() -> io::Result<Self> {
        // SAFETY: null attributes and name create an unnamed, uninheritable job.
        owned(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) }).map(Self)
    }

    /// A job whose processes all end when it is dropped.
    pub(crate) fn killed_on_drop() -> io::Result<Self> {
        let job = Self::new()?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limits` is a live structure of the size passed.
        check(unsafe {
            SetInformationJobObject(
                job.0.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                ptr::from_ref(&limits).cast(),
                size_of_u32::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>(),
            )
        })?;
        Ok(job)
    }

    pub(crate) fn assign(&self, process: RawHandle) -> io::Result<()> {
        // SAFETY: both handles are open; the caller owns `process`.
        check(unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), process) })
    }

    /// Ends every process in the job now.
    pub(crate) fn terminate(&self) {
        // SAFETY: the job handle is open.
        unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) };
    }

    /// The processes in the job, oldest first. Processes that exit while
    /// they are read are left out.
    pub(crate) fn processes(&self) -> Vec<JobProcess> {
        let mut processes: Vec<_> = self
            .process_ids()
            .into_iter()
            .filter_map(|pid| self.process(pid))
            .collect();
        processes.sort_by_key(|process| process.created);
        processes
    }

    fn process_ids(&self) -> Vec<u32> {
        const CAPACITY: usize = 256;
        #[repr(C)]
        struct List {
            header: JOBOBJECT_BASIC_PROCESS_ID_LIST,
            more: [usize; CAPACITY - 1],
        }
        let mut list = List {
            header: JOBOBJECT_BASIC_PROCESS_ID_LIST::default(),
            more: [0; CAPACITY - 1],
        };
        // SAFETY: `list` is a live buffer of the size passed, laid out as the
        // header followed by room for `CAPACITY` IDs.
        let listed = unsafe {
            QueryInformationJobObject(
                self.0.as_raw_handle(),
                JobObjectBasicProcessIdList,
                ptr::from_mut(&mut list).cast(),
                size_of_u32::<List>(),
                ptr::null_mut(),
            )
        };
        // More processes than fit is reported as an error with a partial list.
        if listed == 0 && list.header.NumberOfProcessIdsInList == 0 {
            return Vec::new();
        }
        let count = (list.header.NumberOfProcessIdsInList as usize).min(CAPACITY);
        std::iter::once(list.header.ProcessIdList[0])
            .chain(list.more)
            .take(count)
            .filter_map(|pid| u32::try_from(pid).ok())
            .collect()
    }

    fn process(&self, pid: u32) -> Option<JobProcess> {
        // SAFETY: OpenProcess has no memory arguments.
        let process =
            owned(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) }).ok()?;
        let mut member = 0;
        // SAFETY: both handles are open and `member` is a live BOOL.
        let checked = unsafe {
            IsProcessInJob(
                process.as_raw_handle(),
                self.0.as_raw_handle(),
                &raw mut member,
            )
        };
        if checked == 0 || member == 0 {
            return None;
        }
        Some(JobProcess {
            pid,
            parent: parent_id(&process)?,
            created: creation_time(&process)?,
            handle: process,
        })
    }
}

/// Starts `command` suspended, puts it in `job`, then lets it run, so every
/// process it starts joins the job too. It never opens a console window.
pub(crate) fn spawn_in(job: &Job, command: &mut Command) -> io::Result<Child> {
    let mut child = command
        .creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW)
        .spawn()?;
    let started = job
        .assign(child.as_raw_handle())
        .and_then(|()| resume_threads(child.id()));
    if let Err(error) = started {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    Ok(child)
}

/// Resumes the threads of a process created suspended: just its first one.
fn resume_threads(pid: u32) -> io::Result<()> {
    // SAFETY: CreateToolhelp32Snapshot has no memory arguments.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the snapshot handle is new and owned here.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    let mut entry = THREADENTRY32 {
        dwSize: size_of_u32::<THREADENTRY32>(),
        ..THREADENTRY32::default()
    };
    let mut resumed = false;
    // SAFETY: `entry` is a live structure whose size field is set.
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: OpenThread has no memory arguments.
            let thread =
                owned(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) })?;
            // SAFETY: the thread handle is open with suspend and resume access.
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            resumed = true;
        }
        // SAFETY: as for Thread32First.
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    }
    if resumed {
        Ok(())
    } else {
        Err(io::Error::other("the new process has no thread to resume"))
    }
}

fn parent_id(process: &OwnedHandle) -> Option<u32> {
    let mut information = PROCESS_BASIC_INFORMATION::default();
    // SAFETY: `information` is a live structure of the size passed.
    let status = unsafe {
        NtQueryInformationProcess(
            process.as_raw_handle(),
            ProcessBasicInformation,
            ptr::from_mut(&mut information).cast(),
            size_of_u32::<PROCESS_BASIC_INFORMATION>(),
            ptr::null_mut(),
        )
    };
    (status >= 0)
        .then(|| u32::try_from(information.InheritedFromUniqueProcessId).ok())
        .flatten()
}

fn creation_time(process: &OwnedHandle) -> Option<u64> {
    let mut times = [FILETIME::default(); 4];
    let [created, exited, kernel, user] = &mut times;
    // SAFETY: each pointer is a live FILETIME.
    let read = unsafe { GetProcessTimes(process.as_raw_handle(), created, exited, kernel, user) };
    (read != 0)
        .then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

/// The program's path. Most fit in 260 units; long paths get one retry.
fn image_path(process: &OwnedHandle) -> Option<PathBuf> {
    for capacity in [260, 32 * 1024] {
        let mut buffer = vec![0_u16; capacity];
        let mut length = u32::try_from(capacity).ok()?;
        // SAFETY: `buffer` holds `length` UTF-16 units.
        let read = unsafe {
            QueryFullProcessImageNameW(
                process.as_raw_handle(),
                PROCESS_NAME_WIN32,
                buffer.as_mut_ptr(),
                &raw mut length,
            )
        };
        if read != 0 {
            buffer.truncate(length as usize);
            return Some(PathBuf::from(OsString::from_wide(&buffer)));
        }
    }
    None
}

/// The command line, read in a buffer of the size the system reports.
fn command_line(process: &OwnedHandle) -> Option<String> {
    let mut needed = 0_u32;
    // SAFETY: a zero-length query writes only the size it needs.
    unsafe {
        NtQueryInformationProcess(
            process.as_raw_handle(),
            ProcessCommandLineInformation,
            ptr::null_mut(),
            0,
            &raw mut needed,
        )
    };
    // A system that doesn't report the size gets a generous guess.
    let needed = match usize::try_from(needed).ok()? {
        0 => 8 * 1024,
        needed => needed.clamp(size_of::<UNICODE_STRING>(), 64 * 1024),
    };
    let mut buffer = vec![0_u64; needed.div_ceil(8)];
    let size = u32::try_from(buffer.len() * 8).ok()?;
    // SAFETY: `buffer` is a live, 8-byte aligned buffer of `size` bytes. On
    // success it starts with a UNICODE_STRING pointing into itself.
    let status = unsafe {
        NtQueryInformationProcess(
            process.as_raw_handle(),
            ProcessCommandLineInformation,
            buffer.as_mut_ptr().cast(),
            size,
            ptr::null_mut(),
        )
    };
    if status < 0 {
        return None;
    }
    // SAFETY: the call succeeded, so the buffer starts with a UNICODE_STRING.
    let text = unsafe { ptr::read(buffer.as_ptr().cast::<UNICODE_STRING>()) };
    if text.Buffer.is_null() {
        return None;
    }
    let length = usize::from(text.Length) / 2;
    let start = text.Buffer.cast_const();
    let end = buffer.as_ptr_range().end.cast::<u16>();
    // The text must lie inside the buffer the call filled.
    if start < buffer.as_ptr().cast::<u16>() || start.wrapping_add(length) > end {
        return None;
    }
    // SAFETY: the range was checked to lie inside `buffer`.
    let wide = unsafe { std::slice::from_raw_parts(start, length) };
    Some(String::from_utf16_lossy(wide))
}

fn owned(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: the handle was just returned to this caller, which owns it.
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
}

fn check(result: i32) -> io::Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// The Win32 error for an NT status.
pub(crate) fn status_error(status: i32) -> io::Error {
    // SAFETY: RtlNtStatusToDosError has no memory arguments.
    let code = unsafe { RtlNtStatusToDosError(status) };
    io::Error::from_raw_os_error(code.cast_signed())
}

fn size_of_u32<T>() -> u32 {
    u32::try_from(size_of::<T>()).unwrap_or(u32::MAX)
}
