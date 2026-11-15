use std::error::Error;
use std::io::{self, Read, Write};
use std::os::windows::io::{AsRawHandle, BorrowedHandle, OwnedHandle};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use portable_pty::{Child, MasterPty, PtySize};
use windows_sys::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{INFINITE, WaitForSingleObject};

type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
pub(super) struct Console(Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>);

impl Console {
    pub(super) fn new(master: Box<dyn MasterPty + Send>) -> Self {
        Self(Arc::new(Mutex::new(Some(master))))
    }

    pub(super) fn resize(&self, size: PtySize) -> Result<(), BoxError> {
        match self.lock().as_ref() {
            Some(master) => master.resize(size).map_err(Into::into),
            None => Ok(()),
        }
    }

    pub(super) fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, BoxError> {
        self.lock()
            .as_ref()
            .ok_or("pseudoconsole is closed")?
            .try_clone_reader()
            .map_err(Into::into)
    }

    pub(super) fn take_writer(&self) -> Result<Box<dyn Write + Send>, BoxError> {
        self.lock()
            .as_ref()
            .ok_or("pseudoconsole is closed")?
            .take_writer()
            .map_err(Into::into)
    }

    pub(super) fn close_on_exit(&self, child: &dyn Child) -> io::Result<()> {
        let handle = child
            .as_raw_handle()
            .ok_or_else(|| io::Error::other("child has no process handle"))?;
        let process = unsafe { BorrowedHandle::borrow_raw(handle) }.try_clone_to_owned()?;
        let console = self.clone();
        thread::Builder::new()
            .name("pty-exit".to_owned())
            .spawn(move || {
                if wait(&process).is_ok() {
                    let master = console.lock().take();
                    drop(master);
                }
            })?;
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, Option<Box<dyn MasterPty + Send>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub(super) fn has_exited(child: &dyn Child) -> bool {
    child.as_raw_handle().is_some_and(|handle| {
        let result = unsafe { WaitForSingleObject(handle, 0) };
        result == WAIT_OBJECT_0
    })
}

fn wait(process: &OwnedHandle) -> io::Result<()> {
    if unsafe { WaitForSingleObject(process.as_raw_handle(), INFINITE) } == WAIT_FAILED {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
