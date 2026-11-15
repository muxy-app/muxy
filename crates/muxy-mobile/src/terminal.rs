use std::sync::Arc;

use muxy_client::{Client, ClientError};
use muxy_protocol::{ErrorCode, Size};

use crate::MobileError;
use crate::keys::{self, Key, Modifiers};
use crate::mouse::{self, MouseButton, ScrollDirection};
use crate::records::{Screen, text};
use crate::scrollback::Scrollback;
use crate::terminals::{Terminals, View};

#[doc = " One attached terminal session."]
#[derive(uniffi::Object)]
pub struct Terminal {
    pub(crate) client: Client,
    pub(crate) view: Arc<View>,
    pub(crate) terminals: Arc<Terminals>,
    pub(crate) titles: Arc<crate::titles::Titles>,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Terminal")
            .field("session", &self.view.session)
            .finish_non_exhaustive()
    }
}

#[uniffi::export]
impl Terminal {
    pub fn session_id(&self) -> u64 {
        self.view.session.get()
    }

    #[doc = " The visible screen; call again after a `ScreenChanged` event."]
    pub fn screen(&self) -> Screen {
        let content = self.view.lock();
        Screen::new(
            &content.terminal.grid,
            &content.title,
            &content.directory.0,
            content.input,
        )
    }

    #[doc = " Copies the newest history rows and the screen for scrolling back; new"]
    #[doc = " output keeps updating `screen` but not the copy."]
    pub fn scrollback(&self, max_rows: u16) -> Result<Arc<Scrollback>, MobileError> {
        Scrollback::take(&self.client, self.view.channel, max_rows).map(Arc::new)
    }

    #[doc = " Takes back the size this phone gave the session first, if the"]
    #[doc = " desktop resized it since."]
    pub fn send_input(&self, bytes: Vec<u8>) -> Result<(), MobileError> {
        self.reclaim_size();
        Ok(self.client.send_input(self.view.channel, &bytes)?)
    }

    pub fn send_key(&self, key: Key, modifiers: Modifiers) -> Result<(), MobileError> {
        let modes = self.view.lock().terminal.grid.modes;
        match keys::encode(&key, modifiers, modes) {
            Some(bytes) => self.send_input(bytes),
            None => Ok(()),
        }
    }

    #[doc = " Sends text as one paste, marked as such when the program asks for it."]
    pub fn paste(&self, text: String) -> Result<(), MobileError> {
        let bracketed = self.view.lock().terminal.grid.modes.bracketed_paste;
        self.send_input(keys::paste(&text, bracketed))
    }

    #[doc = " Sends a file of up to 100 MiB, such as a pasted image, for this"]
    #[doc = " session to keep on the server's computer, and returns its absolute"]
    #[doc = " path there. Servers from before uploads fail with `Unsupported`."]
    pub fn upload(&self, name: String, bytes: Vec<u8>) -> Result<String, MobileError> {
        let path = self.client.upload(self.view.session, &name, &bytes)?;
        Ok(text(&path))
    }

    #[doc = " Pastes absolute paths, such as `upload`'s, quoted for the program in"]
    #[doc = " the terminal's foreground and separated by spaces, which AI tools read"]
    #[doc = " as attached files. Nothing is pasted where they can't be quoted safely."]
    pub fn paste_paths(&self, paths: Vec<String>) -> Result<(), MobileError> {
        let modes = self.view.lock().terminal.grid.modes;
        let process = self.titles.process(self.view.session);
        let quoting = muxy_client::input::PathQuoting::for_process(process.as_ref())
            .ok_or(ClientError::Invalid(ErrorCode::BadRequest))?;
        let bytes = keys::paths(&paths, quoting, modes)
            .ok_or(ClientError::Invalid(ErrorCode::BadRequest))?;
        self.send_input(bytes)
    }

    #[doc = " Clicks at a cell: a press, then a release. The program sees it only"]
    #[doc = " while `Screen::mouse_tracking` is on. A click that takes the phone's"]
    #[doc = " size back from the desktop isn't sent, since it was aimed at the"]
    #[doc = " desktop's layout."]
    pub fn click(
        &self,
        button: MouseButton,
        row: u16,
        column: u16,
        modifiers: Modifiers,
    ) -> Result<(), MobileError> {
        if self.reclaim_size() {
            return Ok(());
        }
        for event in mouse::click(button, row, column, modifiers) {
            self.client.send_mouse(self.view.channel, event)?;
        }
        Ok(())
    }

    #[doc = " Turns the mouse wheel one step at a cell. The program sees it only"]
    #[doc = " while `Screen::mouse_tracking` or `Screen::alternate_scroll` is on."]
    #[doc = " Like `click`, a step that takes the phone's size back isn't sent."]
    pub fn scroll(
        &self,
        direction: ScrollDirection,
        row: u16,
        column: u16,
    ) -> Result<(), MobileError> {
        if self.reclaim_size() {
            return Ok(());
        }
        let event = mouse::scroll(direction, row, column);
        Ok(self.client.send_mouse(self.view.channel, event)?)
    }

    #[doc = " Resizes the session for every client viewing it, not just this device."]
    #[doc = " Typing, tapping, or scrolling here takes this size back after the"]
    #[doc = " desktop resizes the session."]
    pub fn resize(&self, columns: u16, rows: u16) -> Result<(), MobileError> {
        let size = Size {
            cols: columns,
            rows,
        };
        self.view.lock().terminal.grid.resize(size);
        self.client.resize(self.view.channel, size)?;
        self.view.lock().claim(size);
        Ok(())
    }

    fn reclaim_size(&self) -> bool {
        let Some((size, seq)) = self.view.lock().reclaim() else {
            return false;
        };
        if self.client.resize(self.view.channel, size).is_err() {
            return false;
        }
        self.view.lock().reclaimed(size, seq);
        true
    }

    #[doc = " Stops receiving this terminal; the session keeps running."]
    pub fn detach(&self) -> Result<(), MobileError> {
        self.terminals.remove(self.view.channel);
        Ok(self.client.detach(self.view.channel)?)
    }
}
