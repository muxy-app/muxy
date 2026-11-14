//! The app is a GUI program, so Windows gives it no console. Output it is
//! asked for from a terminal goes to that terminal's console.

use windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
};

/// Writes standard output to the console of the program that started this
/// one, unless it already goes somewhere, such as a pipe.
pub(crate) fn attach() {
    // SAFETY: neither call takes pointers; a missing handle reads as null.
    unsafe {
        if GetStdHandle(STD_OUTPUT_HANDLE)
            .is_ok_and(|handle| handle.is_invalid() || handle.0.is_null())
        {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
