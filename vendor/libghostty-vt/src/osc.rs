use std::{ffi::CStr, marker::PhantomData, mem::MaybeUninit};

use crate::{
    alloc::{Allocator, Object},
    error::{Result, from_result},
    ffi,
};

#[derive(Debug)]
pub struct Parser<'alloc>(Object<'alloc, ffi::OscParserImpl>);

impl<'alloc> Parser<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::OscParser = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_osc_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    pub fn reset(&mut self) {
        unsafe { ffi::ghostty_osc_reset(self.0.as_raw()) }
    }

    pub fn set_unknown_max_bytes(&mut self, max_bytes: usize) -> Result<&mut Self> {
        let result = unsafe {
            ffi::ghostty_osc_set(
                self.0.as_raw(),
                ffi::OscOption::UNKNOWN_MAX_BYTES,
                std::ptr::from_ref(&max_bytes).cast(),
            )
        };
        from_result(result)?;
        Ok(self)
    }

    pub fn next_byte(&mut self, byte: u8) {
        unsafe { ffi::ghostty_osc_next(self.0.as_raw(), byte) }
    }

    pub fn end<'p>(&'p mut self, terminator: u8) -> Command<'p, 'alloc> {
        Command {
            inner: unsafe { ffi::ghostty_osc_end(self.0.as_raw(), terminator) },
            _parser: PhantomData,
        }
    }
}

impl Drop for Parser<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_osc_free(self.0.as_raw()) }
    }
}

#[derive(Debug)]
pub struct Command<'p, 'alloc> {
    inner: ffi::OscCommand,
    _parser: PhantomData<&'p Parser<'alloc>>,
}

impl<'p> Command<'p, '_> {
    #[must_use]
    pub fn command_type(self) -> CommandType<'p> {
        self.command_type_inner().unwrap_or(CommandType::Invalid)
    }

    fn command_type_inner(&self) -> Option<CommandType<'p>> {
        use ffi::OscCommandData as Data;
        use ffi::OscCommandType as Type;

        let raw_type = unsafe { ffi::ghostty_osc_command_type(self.inner) };
        Some(match raw_type {
            Type::CHANGE_WINDOW_TITLE => {
                let title = self.get::<*const std::ffi::c_char>(Data::CHANGE_WINDOW_TITLE_STR)?;
                let title = unsafe { CStr::from_ptr(title) };
                CommandType::ChangeWindowTitle { title }
            }
            Type::CHANGE_WINDOW_ICON => CommandType::ChangeWindowIcon,
            Type::SEMANTIC_PROMPT => CommandType::SemanticPrompt,
            Type::CLIPBOARD_CONTENTS => CommandType::ClipboardContents,
            Type::REPORT_PWD => CommandType::ReportPwd,
            Type::MOUSE_SHAPE => CommandType::MouseShape,
            Type::COLOR_OPERATION => CommandType::ColorOperation,
            Type::KITTY_COLOR_PROTOCOL => CommandType::KittyColorProtocol,
            Type::SHOW_DESKTOP_NOTIFICATION => CommandType::ShowDesktopNotification,
            Type::HYPERLINK_START => CommandType::HyperlinkStart,
            Type::HYPERLINK_END => CommandType::HyperlinkEnd,
            Type::CONEMU_SLEEP => CommandType::ConemuSleep,
            Type::CONEMU_SHOW_MESSAGE_BOX => CommandType::ConemuShowMessageBox,
            Type::CONEMU_CHANGE_TAB_TITLE => CommandType::ConemuChangeTabTitle,
            Type::CONEMU_PROGRESS_REPORT => CommandType::ConemuProgressReport,
            Type::CONEMU_WAIT_INPUT => CommandType::ConemuWaitInput,
            Type::CONEMU_GUIMACRO => CommandType::ConemuGuiMacro,
            Type::CONEMU_RUN_PROCESS => CommandType::ConemuRunProcess,
            Type::CONEMU_OUTPUT_ENVIRONMENT_VARIABLE => {
                CommandType::ConemuOutputEnvironmentVariable
            }
            Type::CONEMU_XTERM_EMULATION => CommandType::ConemuXtermEmulation,
            Type::CONEMU_COMMENT => CommandType::ConemuComment,
            Type::KITTY_TEXT_SIZING => CommandType::KittyTextSizing,
            Type::KITTY_CLIPBOARD_PROTOCOL => CommandType::KittyClipboardProtocol,
            Type::KITTY_DND_PROTOCOL => CommandType::KittyDndProtocol,
            Type::HIERARCHICAL_CONTEXT => CommandType::HierarchicalContext,
            Type::KITTY_DESKTOP_NOTIFICATION => CommandType::KittyDesktopNotification,
            Type::UNKNOWN => {
                let truncated = self.get(Data::UNKNOWN_TRUNCATED)?;
                let terminator = self
                    .get::<ffi::OscTerminator::Type>(Data::UNKNOWN_TERMINATOR)?
                    .try_into()
                    .ok()?;
                let content = unsafe { self.get::<ffi::String>(Data::UNKNOWN_CONTENT)?.to_bytes() };
                CommandType::Unknown {
                    content,
                    truncated,
                    terminator,
                }
            }
            Type::PROGRAM_STATUS => CommandType::ProgramStatus,

            _ => return None,
        })
    }

    fn get<T>(&self, tag: ffi::OscCommandData::Type) -> Option<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result =
            unsafe { ffi::ghostty_osc_command_data(self.inner, tag, value.as_mut_ptr().cast()) };

        if result {
            Some(unsafe { value.assume_init() })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandType<'p> {
    #[default]
    Invalid,
    ChangeWindowTitle {
        title: &'p CStr,
    },
    ChangeWindowIcon,
    SemanticPrompt,
    ClipboardContents,
    ReportPwd,
    MouseShape,
    ColorOperation,
    KittyColorProtocol,
    ShowDesktopNotification,
    HyperlinkStart,
    HyperlinkEnd,
    ConemuSleep,
    ConemuShowMessageBox,
    ConemuChangeTabTitle,
    ConemuProgressReport,
    ConemuWaitInput,
    ConemuGuiMacro,
    ConemuRunProcess,
    ConemuOutputEnvironmentVariable,
    ConemuXtermEmulation,
    ConemuComment,
    KittyTextSizing,
    KittyClipboardProtocol,
    KittyDndProtocol,
    HierarchicalContext,
    KittyDesktopNotification,
    Unknown {
        content: &'p [u8],
        truncated: bool,
        terminator: Terminator,
    },
    ProgramStatus,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum Terminator {
    St = ffi::OscTerminator::ST,
    Bel = ffi::OscTerminator::BEL,
}
