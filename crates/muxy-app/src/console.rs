use windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
};

pub(crate) fn attach() {
    unsafe {
        if GetStdHandle(STD_OUTPUT_HANDLE)
            .is_ok_and(|handle| handle.is_invalid() || handle.0.is_null())
        {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
