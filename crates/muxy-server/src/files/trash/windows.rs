//! The Windows Recycle Bin, through the shell's `IFileOperation`. A file the
//! shell would delete permanently, as on a drive without a Recycle Bin, is
//! kept instead.
#![allow(
    clippy::inline_always,
    clippy::ref_as_ptr,
    reason = "The COM implement macro generates such code"
)]

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::E_ABORT;
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::UI::Shell::{
    FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE,
    FileOperation, IFileOperation, IFileOperationProgressSink, IFileOperationProgressSink_Impl,
    IShellItem, SHCreateItemFromParsingName, TSF_DELETE_RECYCLE_IF_POSSIBLE,
};
use windows::core::{HRESULT, PCWSTR, Ref, implement};

use super::{Result, Root, Target, error};

const NO_RECYCLE_BIN: &str = "This drive has no Recycle Bin, so the file was kept";

pub(in crate::files) fn move_to_trash(root: &Root, source: &Target) -> Result<()> {
    let expected = source.stat()?;
    let absolute = root.path.join(&source.relative);
    // The shell works by path, which must still name the file just checked.
    let current = muxy_core::files::FileId::of_link(&absolute).map_err(error)?;
    if current.device() != expected.device || current.index() != expected.index {
        return Err(error(
            "Path changed before moving to the Recycle Bin; retry",
        ));
    }
    // The shell's file operations need a single-threaded apartment.
    std::thread::scope(|scope| {
        scope
            .spawn(|| recycle(&absolute))
            .join()
            .unwrap_or_else(|_| Err(error("The Recycle Bin operation failed")))
    })
}

fn recycle(path: &Path) -> Result<()> {
    // SAFETY: this thread was just started and ends after the operation, so
    // its apartment is initialized once and released below.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
        .ok()
        .map_err(error)?;
    let result = perform(path);
    // SAFETY: balances the successful CoInitializeEx above.
    unsafe { CoUninitialize() };
    result
}

fn perform(path: &Path) -> Result<()> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let kept = std::sync::Arc::new(AtomicBool::new(false));
    // SAFETY: COM is initialized on this thread, and every pointer passed
    // stays alive for the calls that use it.
    unsafe {
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(error)?;
        operation
            .SetOperationFlags(
                FOFX_RECYCLEONDELETE
                    | FOF_NOCONFIRMATION
                    | FOF_NOERRORUI
                    | FOF_SILENT
                    | FOFX_EARLYFAILURE,
            )
            .map_err(error)?;
        let item: IShellItem =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).map_err(error)?;
        let sink: IFileOperationProgressSink = RecycleOnly {
            kept: std::sync::Arc::clone(&kept),
        }
        .into();
        operation.DeleteItem(&item, &sink).map_err(error)?;
        let performed = operation.PerformOperations();
        if kept.load(Ordering::Acquire) {
            return Err(error(NO_RECYCLE_BIN));
        }
        performed.map_err(error)?;
        if operation
            .GetAnyOperationsAborted()
            .map_err(error)?
            .as_bool()
        {
            return Err(error("Moving to the Recycle Bin was cancelled"));
        }
    }
    Ok(())
}

/// Cancels any delete that wouldn't go to the Recycle Bin.
#[implement(IFileOperationProgressSink)]
struct RecycleOnly {
    kept: std::sync::Arc<AtomicBool>,
}

impl IFileOperationProgressSink_Impl for RecycleOnly_Impl {
    fn PreDeleteItem(&self, flags: u32, _item: Ref<IShellItem>) -> windows::core::Result<()> {
        if flags & TSF_DELETE_RECYCLE_IF_POSSIBLE.0.cast_unsigned() == 0 {
            self.kept.store(true, Ordering::Release);
            return Err(E_ABORT.into());
        }
        Ok(())
    }

    fn StartOperations(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn FinishOperations(&self, _result: HRESULT) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreRenameItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostRenameItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostMoveItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostCopyItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostDeleteItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreNewItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostNewItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: &PCWSTR,
        _: u32,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _total: u32, _done: u32) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
}
