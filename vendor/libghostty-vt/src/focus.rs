use crate::{
    error::{Result, from_result_with_len},
    ffi,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Gained,
    Lost,
}

impl Event {
    pub fn encode(self, buf: &mut [u8]) -> Result<usize> {
        let mut written: usize = 0;
        let result = unsafe {
            ffi::ghostty_focus_encode(
                self.into(),
                buf.as_mut_ptr().cast(),
                buf.len(),
                &raw mut written,
            )
        };
        from_result_with_len(result, written)
    }
}

impl From<Event> for ffi::FocusEvent::Type {
    fn from(value: Event) -> Self {
        match value {
            Event::Gained => ffi::FocusEvent::GAINED,
            Event::Lost => ffi::FocusEvent::LOST,
        }
    }
}
