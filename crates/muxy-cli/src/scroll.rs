//! Scrollback for one pane: a frozen copy of its screen plus the history the
//! server has paged in for it. The live grid keeps updating underneath.

use std::sync::atomic::{AtomicU64, Ordering};

use muxy_client::{ClientError, RunGrid};
use muxy_protocol::{ErrorCode, HistoryCursor, HistoryPage};

const RECENT_ROWS: u16 = 200;
const OLDER_ROWS: u16 = 500;
/// Older history is requested once the view comes this close to its top.
const PREFETCH_ROWS: usize = 100;

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryRequest {
    pub token: u64,
    pub before: HistoryCursor,
    pub max_rows: u16,
}

impl HistoryRequest {
    fn new(before: HistoryCursor) -> Self {
        Self {
            token: NEXT_REQUEST.fetch_add(1, Ordering::Relaxed),
            before,
            max_rows: if before.0 == 0 {
                RECENT_ROWS
            } else {
                OLDER_ROWS
            },
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Scroll {
    view: Option<RunGrid>,
    /// Rows above the bottom that were asked for.
    wanted: usize,
    /// Rows above the bottom that are shown; less than `wanted` while
    /// history is still loading.
    offset: usize,
    pending: Option<HistoryRequest>,
    restarted: bool,
}

impl Scroll {
    pub(crate) fn is_active(&self) -> bool {
        self.view.is_some()
    }

    pub(crate) fn offset(&self) -> usize {
        self.offset
    }

    /// Rows that can be scrolled through, once the server has said.
    pub(crate) fn total(&self, height: usize) -> Option<usize> {
        let view = self.view.as_ref().filter(|view| view.history_fresh)?;
        Some(
            usize::try_from(view.history_total)
                .unwrap_or(usize::MAX)
                .saturating_add(view.rows.len())
                .saturating_sub(height),
        )
    }

    /// The grid to draw: the frozen view while scrolled, otherwise `live`.
    pub(crate) fn grid<'a>(&'a self, live: &'a RunGrid) -> &'a RunGrid {
        self.view.as_ref().unwrap_or(live)
    }

    /// The first content row shown in a pane `height` rows tall. The live
    /// screen is drawn from its top; the frozen view from its bottom.
    pub(crate) fn start(&self, live: &RunGrid, height: usize) -> usize {
        let Some(view) = &self.view else {
            return live.history.len();
        };
        view.history
            .len()
            .saturating_add(view.rows.len())
            .saturating_sub(height)
            .saturating_sub(self.offset)
    }

    pub(crate) fn by(
        &mut self,
        delta: isize,
        live: &RunGrid,
        height: usize,
    ) -> Option<HistoryRequest> {
        self.to(self.wanted.saturating_add_signed(delta), live, height)
    }

    pub(crate) fn to(
        &mut self,
        wanted: usize,
        live: &RunGrid,
        height: usize,
    ) -> Option<HistoryRequest> {
        if wanted == 0 {
            self.bottom();
            return None;
        }
        if self.pending.is_none() {
            self.restarted = false;
        }
        if self.view.is_none() {
            let mut view = live.clone();
            view.clear_history();
            self.view = Some(view);
        }
        self.wanted = wanted;
        self.settle(height);
        self.request(height)
    }

    pub(crate) fn bottom(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn receive(
        &mut self,
        request: HistoryRequest,
        result: Result<HistoryPage, ClientError>,
        height: usize,
    ) -> Option<HistoryRequest> {
        if self.pending != Some(request) {
            return None;
        }
        self.pending = None;
        let view = self.view.as_mut()?;
        match result {
            Ok(page) => {
                if request.before.0 == 0 {
                    view.replace_history(page);
                } else {
                    view.fetch_older(page);
                }
                self.settle(height);
                if self.wanted == 0 {
                    self.bottom();
                    return None;
                }
                self.request(height)
            }
            Err(ClientError::Server(error))
                if error.code == ErrorCode::StaleHistoryCursor
                    && request.before.0 != 0
                    && !self.restarted =>
            {
                self.restarted = true;
                view.clear_history();
                self.settle(height);
                self.request(height)
            }
            Err(_) => {
                if request.before.0 == 0 {
                    self.bottom();
                }
                None
            }
        }
    }

    fn settle(&mut self, height: usize) {
        let Some(view) = &self.view else {
            return;
        };
        if let Some(total) = self.total(height) {
            self.wanted = self.wanted.min(total);
        }
        let loaded = view
            .history
            .len()
            .saturating_add(view.rows.len())
            .saturating_sub(height);
        self.offset = self.wanted.min(loaded);
    }

    fn request(&mut self, height: usize) -> Option<HistoryRequest> {
        if self.pending.is_some() {
            return None;
        }
        let view = self.view.as_ref()?;
        let before = if view.history_fresh {
            let loaded = view
                .history
                .len()
                .saturating_add(view.rows.len())
                .saturating_sub(height);
            if self.offset.saturating_add(PREFETCH_ROWS) < loaded && self.wanted <= self.offset {
                return None;
            }
            view.history_cursor?
        } else {
            HistoryCursor(0)
        };
        let request = HistoryRequest::new(before);
        self.pending = Some(request);
        Some(request)
    }
}

#[cfg(test)]
mod tests {
    use muxy_protocol::{Cursor, CursorShape, ErrorReply, Row, Run, SavedScreen, Size, Style};

    use super::*;

    fn row(text: &str) -> Row {
        Row {
            index: 0,
            runs: vec![Run {
                text: text.into(),
                width: u16::try_from(text.len()).unwrap_or(u16::MAX),
                style: Style::default(),
            }],
        }
    }

    fn screen(prefix: &str) -> SavedScreen {
        SavedScreen {
            graphics: muxy_protocol::Graphics::default(),
            size: Size { cols: 10, rows: 3 },
            rows: (0..3)
                .map(|index| Row {
                    index,
                    ..row(&format!("{prefix}{index}"))
                })
                .collect(),
            cursor: Cursor {
                shape: CursorShape::default(),
                row: 0,
                col: 0,
                visible: true,
            },
            reason: None,
        }
    }

    fn page(rows: usize, next: Option<u64>, total: u64, with_screen: bool) -> HistoryPage {
        HistoryPage {
            prompts: Vec::new(),
            rows: (0..rows).map(|index| row(&format!("h{index}"))).collect(),
            next: next.map(HistoryCursor),
            total_rows: total,
            screen: with_screen.then(|| screen("frozen")),
        }
    }

    #[test]
    fn scrolling_up_freezes_the_screen_and_pages_history_in_from_the_newest() {
        let live = RunGrid::from_saved(screen("live"));
        let mut scroll = Scroll::default();
        let recent = scroll.by(5, &live, 3).ok_or("recent request");
        assert_eq!(recent.map(|request| request.before), Ok(HistoryCursor(0)));
        assert!(scroll.is_active());
        assert_eq!(scroll.offset(), 0);
        assert!(scroll.by(5, &live, 3).is_none(), "one request at a time");

        let older = recent
            .ok()
            .and_then(|recent| scroll.receive(recent, Ok(page(200, Some(800), 1000, true)), 3));
        assert_eq!(scroll.offset(), 10);
        assert_eq!(scroll.grid(&live).row_text(0), "frozen0");
        assert_eq!(scroll.start(&live, 3), 190);
        assert!(older.is_none(), "far from the top of what is loaded");

        let older = scroll.to(150, &live, 3).ok_or("older request");
        assert_eq!(
            older.map(|request| (request.before, request.max_rows)),
            Ok((HistoryCursor(800), OLDER_ROWS))
        );
        assert_eq!(
            older.ok().and_then(|older| {
                scroll.receive(older, Ok(page(500, Some(300), 1000, false)), 3)
            }),
            None
        );
        assert_eq!(scroll.offset(), 150);
        assert_eq!(scroll.start(&live, 3), 550);
        assert_eq!(scroll.total(3), Some(1000));
    }

    #[test]
    fn scrolling_stops_at_the_oldest_row_and_returns_live_at_the_bottom() {
        let live = RunGrid::from_saved(screen("live"));
        let mut scroll = Scroll::default();
        let recent = scroll.by(50, &live, 3);
        assert!(
            recent
                .and_then(|recent| scroll.receive(recent, Ok(page(20, None, 20, true)), 3))
                .is_none()
        );
        assert_eq!(scroll.offset(), 20);
        assert!(scroll.by(10, &live, 3).is_none());
        assert_eq!(scroll.offset(), 20);
        assert!(scroll.by(-25, &live, 3).is_none());
        assert!(!scroll.is_active());
        assert_eq!(scroll.grid(&live).row_text(0), "live0");
    }

    #[test]
    fn stale_cursors_restart_once_and_failed_first_pages_return_live() {
        let live = RunGrid::from_saved(screen("live"));
        let mut scroll = Scroll::default();
        let recent = scroll.by(1, &live, 3);
        let older = recent
            .and_then(|recent| scroll.receive(recent, Ok(page(300, Some(700), 1000, true)), 3));
        assert!(older.is_none());
        let older = scroll.to(250, &live, 3);
        assert_eq!(
            older.map(|request| request.before),
            Some(HistoryCursor(700))
        );
        let stale = || {
            Err(ClientError::Server(ErrorReply {
                code: ErrorCode::StaleHistoryCursor,
                message: String::new(),
            }))
        };
        let restart = older.and_then(|older| scroll.receive(older, stale(), 3));
        assert_eq!(
            restart.map(|request| request.before),
            Some(HistoryCursor(0))
        );
        assert!(scroll.is_active());
        let failed = restart.and_then(|restart| scroll.receive(restart, stale(), 3));
        assert!(failed.is_none());
        assert!(!scroll.is_active());
    }
}
