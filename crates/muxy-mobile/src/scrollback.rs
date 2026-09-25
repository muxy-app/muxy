use std::sync::{Mutex, PoisonError};

use muxy_client::{Client, ClientError};
use muxy_protocol::{ChannelId, ErrorCode, HistoryCursor, HistoryPage};

use crate::MobileError;
use crate::records::Line;

/// A terminal's history and screen as they were at one moment, so reading
/// back is not disturbed by new output.
#[derive(uniffi::Object)]
pub struct Scrollback {
    client: Client,
    channel: ChannelId,
    lines: Vec<Line>,
    history_rows: u64,
    /// Where the next older page ends; `None` once nothing older remains.
    older: Mutex<Option<HistoryCursor>>,
}

impl std::fmt::Debug for Scrollback {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Scrollback")
            .field("channel", &self.channel)
            .field("lines", &self.lines.len())
            .finish_non_exhaustive()
    }
}

impl Scrollback {
    pub(crate) fn take(
        client: &Client,
        channel: ChannelId,
        max_rows: u16,
    ) -> Result<Self, MobileError> {
        let newest = HistoryCursor(0);
        let page = client.history_page(channel, newest, page_size(max_rows))?;
        let lines = page
            .rows
            .iter()
            .chain(page.screen.iter().flat_map(|screen| &screen.rows))
            .map(Line::from_row)
            .collect();
        Ok(Self {
            client: client.clone(),
            channel,
            lines,
            history_rows: page.total_rows,
            older: Mutex::new(page.next),
        })
    }
}

#[uniffi::export]
impl Scrollback {
    /// The newest history rows followed by the screen, oldest first.
    pub fn lines(&self) -> Vec<Line> {
        self.lines.clone()
    }

    /// Rows the server kept above the screen when this copy was taken.
    pub fn history_rows(&self) -> u64 {
        self.history_rows
    }

    /// The rows just above those already loaded, oldest first. Empty once
    /// nothing older remains, or once the history was cleared or rewrapped;
    /// take a new scrollback to see it again.
    pub fn load_older(&self, max_rows: u16) -> Result<Vec<Line>, MobileError> {
        let mut older = self.older.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(before) = *older else {
            return Ok(Vec::new());
        };
        match self
            .client
            .history_page(self.channel, before, page_size(max_rows))
        {
            Ok(HistoryPage { rows, next, .. }) => {
                *older = next;
                Ok(rows.iter().map(Line::from_row).collect())
            }
            Err(ClientError::Server(error)) if error.code == ErrorCode::StaleHistoryCursor => {
                *older = None;
                Ok(Vec::new())
            }
            Err(error) => Err(error.into()),
        }
    }
}

/// The server serves 1 to 500 rows per page.
fn page_size(max_rows: u16) -> u16 {
    max_rows.clamp(1, 500)
}
