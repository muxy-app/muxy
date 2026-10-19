//! The Win32 calls behind stop requests: a console control handler, and a
//! hidden top-level window that hears the session end. Message-only windows
//! don't receive `WM_QUERYENDSESSION`.

use std::io;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Console::{
    CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT, SetConsoleCtrlHandler,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, MSG,
    PostMessageW, PostQuitMessage, RegisterClassW, WM_CLOSE, WM_DESTROY, WM_ENDSESSION,
    WM_QUERYENDSESSION, WNDCLASSW, WS_OVERLAPPED,
};

/// The system ends the process soon after a console close or session end is
/// handled; the server gets this long to stop cleanly first.
const GRACE: Duration = Duration::from_secs(4);

type Stop = Arc<dyn Fn() + Send + Sync>;

/// The handlers are process-wide callbacks without a context argument.
static STOP: Mutex<Option<Stop>> = Mutex::new(None);
static PENDING: AtomicBool = AtomicBool::new(false);
static FINISHED: Mutex<bool> = Mutex::new(false);
static DONE: Condvar = Condvar::new();

pub(super) struct Console;

impl Console {
    pub(super) fn install() -> io::Result<Self> {
        // SAFETY: `control` is a valid handler for the life of the process.
        if unsafe { SetConsoleCtrlHandler(Some(control), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self)
    }

    /// Calls `stop` for every request from now on, and now for one that
    /// arrived before.
    pub(super) fn listen(stop: Stop) {
        let current = {
            let mut slot = STOP.lock().unwrap_or_else(PoisonError::into_inner);
            *slot = Some(stop);
            slot.clone()
        };
        if PENDING.swap(false, Ordering::AcqRel)
            && let Some(stop) = current
        {
            stop();
        }
    }
}

/// A thread running the hidden window's message loop.
pub(super) struct Session {
    window: isize,
    thread: JoinHandle<()>,
}

impl Session {
    pub(super) fn start() -> io::Result<Self> {
        let (created, window) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("server-session-end".into())
            .spawn(move || match create_window() {
                Ok(window) => {
                    let _ = created.send(Ok(window as isize));
                    run_messages();
                }
                Err(error) => {
                    let _ = created.send(Err(error));
                }
            })?;
        let window = window
            .recv()
            .map_err(|_| io::Error::other("session window thread stopped"))??;
        Ok(Self { window, thread })
    }

    pub(super) fn close(self) -> io::Result<()> {
        *FINISHED.lock().unwrap_or_else(PoisonError::into_inner) = true;
        DONE.notify_all();
        // SAFETY: the window belongs to the thread joined below.
        unsafe { PostMessageW(self.window as HWND, WM_CLOSE, 0, 0) };
        self.thread
            .join()
            .map_err(|_| io::Error::other("session window thread panicked"))
    }
}

fn request() {
    let stop = STOP.lock().unwrap_or_else(PoisonError::into_inner).clone();
    match stop {
        Some(stop) => stop(),
        None => PENDING.store(true, Ordering::Release),
    }
}

fn wait_until_finished() {
    let finished = FINISHED.lock().unwrap_or_else(PoisonError::into_inner);
    let _ = DONE
        .wait_timeout_while(finished, GRACE, |finished| !*finished)
        .unwrap_or_else(PoisonError::into_inner);
}

unsafe extern "system" fn control(kind: u32) -> i32 {
    request();
    if matches!(
        kind,
        CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT
    ) {
        wait_until_finished();
    }
    1
}

unsafe extern "system" fn window_procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_QUERYENDSESSION => 1,
        WM_ENDSESSION => {
            if wparam != 0 {
                request();
                wait_until_finished();
            }
            0
        }
        WM_CLOSE => {
            // SAFETY: this thread owns the window.
            unsafe { DestroyWindow(window) };
            0
        }
        WM_DESTROY => {
            // SAFETY: PostQuitMessage has no memory arguments.
            unsafe { PostQuitMessage(0) };
            0
        }
        // SAFETY: the arguments come from the system unchanged.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn create_window() -> io::Result<HWND> {
    let class: Vec<u16> = "MuxyServerSessionEnd\0".encode_utf16().collect();
    // SAFETY: a null name returns this executable's module handle.
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let description = WNDCLASSW {
        lpfnWndProc: Some(window_procedure),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..WNDCLASSW::default()
    };
    // SAFETY: `description` and the class name it points to are live.
    if unsafe { RegisterClassW(&raw const description) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the class was registered above; the window is never shown.
    let window = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    };
    if window.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(window)
}

fn run_messages() {
    let mut message = MSG::default();
    // SAFETY: `message` is a live MSG; zero and negative results end the loop.
    while unsafe { GetMessageW(&raw mut message, ptr::null_mut(), 0, 0) } > 0 {
        // SAFETY: the message was just filled in by GetMessageW.
        unsafe { DispatchMessageW(&raw const message) };
    }
}
