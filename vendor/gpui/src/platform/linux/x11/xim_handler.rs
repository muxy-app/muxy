use std::default::Default;

use x11rb::protocol::{Event, xproto};
use xim::{AHashMap, AttributeName, Client, ClientError, ClientHandler, InputStyle};

pub enum XimCallbackEvent {
    XimXEvent(x11rb::protocol::Event),
    /// The range is the composing caret in UTF-16 code units of the preedit string.
    XimPreeditEvent(xproto::Window, String, Option<std::ops::Range<usize>>),
    XimCommitEvent(xproto::Window, String),
}

pub struct XimHandler {
    pub im_id: u16,
    pub ic_id: u16,
    pub connected: bool,
    pub window: xproto::Window,
    pub last_callback_event: Option<XimCallbackEvent>,
}

impl XimHandler {
    pub fn new() -> Self {
        Self {
            im_id: Default::default(),
            ic_id: Default::default(),
            connected: false,
            window: Default::default(),
            last_callback_event: None,
        }
    }
}

impl<C: Client<XEvent = xproto::KeyPressEvent>> ClientHandler<C> for XimHandler {
    fn handle_connect(&mut self, client: &mut C) -> Result<(), ClientError> {
        client.open("C")
    }

    fn handle_open(&mut self, client: &mut C, input_method_id: u16) -> Result<(), ClientError> {
        self.im_id = input_method_id;

        client.get_im_values(input_method_id, &[AttributeName::QueryInputStyle])
    }

    fn handle_get_im_values(
        &mut self,
        client: &mut C,
        input_method_id: u16,
        _attributes: AHashMap<AttributeName, Vec<u8>>,
    ) -> Result<(), ClientError> {
        let ic_attributes = client
            .build_ic_attributes()
            .push(AttributeName::InputStyle, InputStyle::PREEDIT_CALLBACKS)
            .push(AttributeName::ClientWindow, self.window)
            .push(AttributeName::FocusWindow, self.window)
            .build();
        client.create_ic(input_method_id, ic_attributes)
    }

    fn handle_create_ic(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        self.connected = true;
        self.ic_id = input_context_id;
        Ok(())
    }

    fn handle_commit(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        _input_context_id: u16,
        text: &str,
    ) -> Result<(), ClientError> {
        self.last_callback_event = Some(XimCallbackEvent::XimCommitEvent(
            self.window,
            String::from(text),
        ));
        Ok(())
    }

    fn handle_forward_event(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        _input_context_id: u16,
        _flag: xim::ForwardEventFlag,
        xev: C::XEvent,
    ) -> Result<(), ClientError> {
        match xev.response_type {
            x11rb::protocol::xproto::KEY_PRESS_EVENT => {
                self.last_callback_event = Some(XimCallbackEvent::XimXEvent(Event::KeyPress(xev)));
            }
            x11rb::protocol::xproto::KEY_RELEASE_EVENT => {
                self.last_callback_event =
                    Some(XimCallbackEvent::XimXEvent(Event::KeyRelease(xev)));
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_close(&mut self, client: &mut C, _input_method_id: u16) -> Result<(), ClientError> {
        client.disconnect()
    }

    fn handle_preedit_draw(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        _input_context_id: u16,
        caret: i32,
        _chg_first: i32,
        _chg_len: i32,
        _status: xim::PreeditDrawStatus,
        preedit_string: &str,
        _feedbacks: Vec<xim::Feedback>,
    ) -> Result<(), ClientError> {
        // XIMReverse: 1, XIMPrimary: 8, XIMTertiary: 32: selected text
        // XIMUnderline: 2, XIMSecondary: 16: underlined text
        // XIMHighlight: 4: normal text
        // XIMVisibleToForward: 64, XIMVisibleToBackward: 128, XIMVisibleCenter: 256: text align position
        // XIMPrimary, XIMHighlight, XIMSecondary, XIMTertiary are not specified,
        // but interchangeable as above
        // Currently there's no way to support these.
        self.last_callback_event = Some(XimCallbackEvent::XimPreeditEvent(
            self.window,
            String::from(preedit_string),
            xim_caret_utf16(preedit_string, caret),
        ));
        Ok(())
    }
}

/// XIM reports the caret as a character index into the preedit string.
/// The input handler wants that caret in UTF-16 code units of the same string.
fn xim_caret_utf16(text: &str, caret: i32) -> Option<std::ops::Range<usize>> {
    if caret < 0 {
        return None;
    }
    let caret = caret as usize;
    let mut utf16 = 0;
    for (index, ch) in text.chars().enumerate() {
        if index == caret {
            return Some(utf16..utf16);
        }
        utf16 += ch.len_utf16();
    }
    (text.chars().count() == caret).then_some(utf16..utf16)
}

#[cfg(test)]
mod tests {
    use super::xim_caret_utf16;

    #[test]
    fn caret_after_a_wide_character_is_one_utf16_unit() {
        assert_eq!(xim_caret_utf16("你", 1), Some(1..1));
        assert_eq!(xim_caret_utf16("你", 0), Some(0..0));
        assert_eq!(xim_caret_utf16("ab你", 3), Some(3..3));
    }
}
