use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::path::PathBuf;
use std::ptr;

use windows_sys::Wdk::System::Threading::{
    NtQueryInformationProcess, ProcessBasicInformation, ProcessCommandLineInformation,
};
use windows_sys::Win32::Foundation::{
    FILETIME, HANDLE, INVALID_HANDLE_VALUE, RtlNtStatusToDosError, UNICODE_STRING,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_PROCESS_ID_LIST, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_BASIC_INFORMATION, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};

#[derive(Debug)]
pub(crate) struct Job(OwnedHandle);

#[derive(Debug)]
pub(crate) struct JobProcess {
    pub(crate) pid: u32,
    pub(crate) parent: u32,
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
    pub(crate) fn new() -> io::Result<Self> {
        owned(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) }).map(Self)
    }

    pub(crate) fn killed_on_drop() -> io::Result<Self> {
        let job = Self::new()?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
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
        check(unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), process) })
    }

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
        let listed = unsafe {
            QueryInformationJobObject(
                self.0.as_raw_handle(),
                JobObjectBasicProcessIdList,
                ptr::from_mut(&mut list).cast(),
                size_of_u32::<List>(),
                ptr::null_mut(),
            )
        };
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
        let process =
            owned(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) }).ok()?;
        let mut member = 0;
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

fn parent_id(process: &OwnedHandle) -> Option<u32> {
    let mut information = PROCESS_BASIC_INFORMATION::default();
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
    let read = unsafe { GetProcessTimes(process.as_raw_handle(), created, exited, kernel, user) };
    (read != 0)
        .then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

fn image_path(process: &OwnedHandle) -> Option<PathBuf> {
    for capacity in [260, 32 * 1024] {
        let mut buffer = vec![0_u16; capacity];
        let mut length = u32::try_from(capacity).ok()?;
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

fn command_line(process: &OwnedHandle) -> Option<String> {
    let mut needed = 0_u32;
    unsafe {
        NtQueryInformationProcess(
            process.as_raw_handle(),
            ProcessCommandLineInformation,
            ptr::null_mut(),
            0,
            &raw mut needed,
        )
    };
    let needed = match usize::try_from(needed).ok()? {
        0 => 8 * 1024,
        needed => needed.clamp(size_of::<UNICODE_STRING>(), 64 * 1024),
    };
    let mut buffer = vec![0_u64; needed.div_ceil(8)];
    let size = u32::try_from(buffer.len() * 8).ok()?;
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
    let text = unsafe { ptr::read(buffer.as_ptr().cast::<UNICODE_STRING>()) };
    if text.Buffer.is_null() {
        return None;
    }
    let length = usize::from(text.Length) / 2;
    let start = text.Buffer.cast_const();
    let end = buffer.as_ptr_range().end.cast::<u16>();
    if start < buffer.as_ptr().cast::<u16>() || start.wrapping_add(length) > end {
        return None;
    }
    let wide = unsafe { std::slice::from_raw_parts(start, length) };
    Some(String::from_utf16_lossy(wide))
}

fn owned(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
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

pub(crate) fn status_error(status: i32) -> io::Error {
    let code = unsafe { RtlNtStatusToDosError(status) };
    io::Error::from_raw_os_error(code.cast_signed())
}

fn size_of_u32<T>() -> u32 {
    u32::try_from(size_of::<T>()).unwrap_or(u32::MAX)
}
