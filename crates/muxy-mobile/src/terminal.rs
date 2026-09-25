use std::sync::Arc;

use muxy_client::Client;
use muxy_protocol::Size;

use crate::MobileError;
use crate::keys::{self, Key, Modifiers};
use crate::records::Screen;
use crate::scrollback::Scrollback;
use crate::terminals::{Terminals, View};

/// One attached terminal session.
#[derive(uniffi::Object)]
pub struct Terminal {
    pub(crate) client: Client,
    pub(crate) view: Arc<View>,
    pub(crate) terminals: Arc<Terminals>,
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

    /// The visible screen; call again after a `ScreenChanged` event.
    pub fn screen(&self) -> Screen {
        let content = self.view.lock();
        Screen::new(&content.grid, &content.title, &content.directory.0)
    }

    /// Copies the newest history rows and the screen for scrolling back; new
    /// output keeps updating `screen` but not the copy.
    pub fn scrollback(&self, max_rows: u16) -> Result<Arc<Scrollback>, MobileError> {
        Scrollback::take(&self.client, self.view.channel, max_rows).map(Arc::new)
    }

    pub fn send_input(&self, bytes: Vec<u8>) -> Result<(), MobileError> {
        Ok(self.client.send_input(self.view.channel, &bytes)?)
    }

    pub fn send_key(&self, key: Key, modifiers: Modifiers) -> Result<(), MobileError> {
        let modes = self.view.lock().grid.modes;
        match keys::encode(&key, modifiers, modes) {
            Some(bytes) => self.send_input(bytes),
            None => Ok(()),
        }
    }

    /// Sends text as one paste, marked as such when the program asks for it.
    pub fn paste(&self, text: String) -> Result<(), MobileError> {
        let bracketed = self.view.lock().grid.modes.bracketed_paste;
        self.send_input(keys::paste(&text, bracketed))
    }

    /// Resizes the session for every client viewing it, not just this device.
    pub fn resize(&self, columns: u16, rows: u16) -> Result<(), MobileError> {
        let size = Size {
            cols: columns,
            rows,
        };
        // Resize first so the server's full redraw lands on a grid of the new size.
        self.view.lock().grid.resize(size);
        Ok(self.client.resize(self.view.channel, size)?)
    }

    /// Stops receiving this terminal; the session keeps running.
    pub fn detach(&self) -> Result<(), MobileError> {
        self.terminals.remove(self.view.channel);
        Ok(self.client.detach(self.view.channel)?)
    }
}
