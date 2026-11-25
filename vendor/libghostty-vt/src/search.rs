use std::marker::PhantomData;

use crate::{
    Terminal,
    alloc::{Allocator, Object},
    error::{Error, Result, from_optional_result, from_result},
    ffi,
    selection::Selection,
};

#[derive(Debug)]
pub struct Search<'alloc: 'term, 'term> {
    inner: Object<'alloc, ffi::SearchImpl>,
    terminal: ffi::Terminal,
    _terminal: PhantomData<&'term ()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[repr(i32)]
pub enum Status {
    Running = ffi::SearchStatus::RUNNING,
    FeedRequired = ffi::SearchStatus::FEED_REQUIRED,
    Complete = ffi::SearchStatus::COMPLETE,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[repr(i32)]
pub enum Scroll {
    #[default]
    IfNeeded = ffi::SearchScroll::IF_NEEDED,
    None = ffi::SearchScroll::NONE,
}

impl<'alloc: 'term, 'term> Search<'alloc, 'term> {
    pub fn new(terminal: &mut Terminal<'term, '_>) -> Result<Self> {
        unsafe { Self::new_inner(terminal, std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(
        alloc: &'alloc Allocator<'ctx>,
        terminal: &mut Terminal<'term, '_>,
    ) -> Result<Self> {
        unsafe { Self::new_inner(terminal, alloc.to_raw()) }
    }

    unsafe fn new_inner(
        terminal: &mut Terminal<'term, '_>,
        alloc: *const ffi::Allocator,
    ) -> Result<Self> {
        let mut raw = std::ptr::null_mut();
        from_result(unsafe {
            ffi::ghostty_search_new(alloc, &raw mut raw, terminal.inner.as_raw())
        })?;
        Ok(Self {
            inner: Object::new(raw)?,
            terminal: terminal.inner.as_raw(),
            _terminal: PhantomData,
        })
    }

    fn check_terminal(&self, terminal: &Terminal<'_, '_>) -> Result<()> {
        if terminal.inner.as_raw() == self.terminal {
            Ok(())
        } else {
            Err(Error::InvalidValue)
        }
    }

    pub fn set_needle(
        &mut self,
        terminal: &mut Terminal<'_, '_>,
        needle: &[u8],
    ) -> Result<&mut Self> {
        self.check_terminal(terminal)?;
        let raw = ffi::String {
            ptr: needle.as_ptr(),
            len: needle.len(),
        };
        from_result(unsafe {
            ffi::ghostty_search_set(
                self.inner.as_raw(),
                ffi::SearchOption::NEEDLE,
                (&raw const raw).cast(),
            )
        })?;
        Ok(self)
    }

    pub fn needle(&self) -> Result<Option<&[u8]>> {
        let mut raw = ffi::String::default();
        let result = unsafe {
            ffi::ghostty_search_get(
                self.inner.as_raw(),
                ffi::SearchData::NEEDLE,
                (&raw mut raw).cast(),
            )
        };
        Ok(from_optional_result(result, raw)?.map(|raw| unsafe { raw.to_bytes() }))
    }

    pub fn feed(&mut self, terminal: &mut Terminal<'_, '_>) -> Result<()> {
        self.check_terminal(terminal)?;
        from_result(unsafe { ffi::ghostty_search_feed(self.inner.as_raw()) })
    }

    pub fn tick(&mut self) -> Result<Status> {
        let mut status = ffi::SearchStatus::COMPLETE;
        from_result(unsafe { ffi::ghostty_search_tick(self.inner.as_raw(), &raw mut status) })?;
        status.try_into().map_err(|_| Error::InvalidValue)
    }

    pub fn run(&mut self, terminal: &mut Terminal<'_, '_>) -> Result<()> {
        self.check_terminal(terminal)?;
        from_result(unsafe { ffi::ghostty_search_run(self.inner.as_raw()) })
    }

    fn get<T: Default>(&self, key: ffi::SearchData::Type) -> Result<T> {
        let mut value = T::default();
        from_result(unsafe {
            ffi::ghostty_search_get(self.inner.as_raw(), key, (&raw mut value).cast())
        })?;
        Ok(value)
    }

    pub fn status(&self) -> Result<Status> {
        self.get::<ffi::SearchStatus::Type>(ffi::SearchData::STATUS)?
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }

    pub fn total_matches(&self) -> Result<usize> {
        self.get(ffi::SearchData::TOTAL_MATCHES)
    }

    pub fn selected_index(&self) -> Result<Option<usize>> {
        let mut index = 0usize;
        let result = unsafe {
            ffi::ghostty_search_get(
                self.inner.as_raw(),
                ffi::SearchData::SELECTED_INDEX,
                (&raw mut index).cast(),
            )
        };
        from_optional_result(result, index)
    }

    pub fn scroll(&self) -> Result<Scroll> {
        self.get::<ffi::SearchScroll::Type>(ffi::SearchData::SELECT_SCROLL)?
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }

    pub fn set_scroll(&mut self, scroll: Scroll) -> Result<&mut Self> {
        let raw: ffi::SearchScroll::Type = scroll.into();
        from_result(unsafe {
            ffi::ghostty_search_set(
                self.inner.as_raw(),
                ffi::SearchOption::SELECT_SCROLL,
                (&raw const raw).cast(),
            )
        })?;
        Ok(self)
    }

    fn select(
        &mut self,
        terminal: &mut Terminal<'_, '_>,
        key: ffi::SearchOption::Type,
    ) -> Result<bool> {
        self.check_terminal(terminal)?;
        let result = unsafe { ffi::ghostty_search_set(self.inner.as_raw(), key, std::ptr::null()) };
        Ok(from_optional_result(result, ())?.is_some())
    }

    pub fn select_next(&mut self, terminal: &mut Terminal<'_, '_>) -> Result<bool> {
        self.select(terminal, ffi::SearchOption::SELECT_NEXT)
    }

    pub fn select_prev(&mut self, terminal: &mut Terminal<'_, '_>) -> Result<bool> {
        self.select(terminal, ffi::SearchOption::SELECT_PREV)
    }

    pub fn snapshot<'s, 'ta: 'cb, 'cb>(
        &'s mut self,
        terminal: &'s mut Terminal<'ta, 'cb>,
    ) -> Result<Snapshot<'s, 'alloc, 'term, 'ta, 'cb>> {
        self.feed(terminal)?;
        Ok(Snapshot {
            search: self,
            terminal,
        })
    }
}

impl Drop for Search<'_, '_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_search_free(self.inner.as_raw()) };
    }
}

#[derive(Debug)]
pub struct Snapshot<'s, 'alloc: 'term, 'term, 'ta: 'cb, 'cb> {
    search: &'s Search<'alloc, 'term>,
    terminal: &'s Terminal<'ta, 'cb>,
}

impl<'ta: 'cb, 'cb> Snapshot<'_, '_, '_, 'ta, 'cb> {
    #[must_use]
    pub fn terminal(&self) -> &Terminal<'ta, 'cb> {
        self.terminal
    }

    pub fn selected_match(&self) -> Result<Option<Selection<'_>>> {
        let mut raw = ffi::sized!(ffi::Selection);
        let result = unsafe {
            ffi::ghostty_search_get(
                self.search.inner.as_raw(),
                ffi::SearchData::SELECTED_MATCH,
                (&raw mut raw).cast(),
            )
        };
        Ok(from_optional_result(result, raw)?.map(|raw| unsafe { Selection::from_raw(raw) }))
    }

    fn read_matches<'s>(
        &'s self,
        storage: &'s mut MatchBuffer,
        key: ffi::SearchData::Type,
    ) -> Result<Matches<'s>> {
        loop {
            let mut buffer = ffi::SelectionBuffer {
                ptr: if storage.inner.is_empty() {
                    std::ptr::null_mut()
                } else {
                    storage.inner.as_mut_ptr()
                },
                cap: storage.inner.len(),
                len: 0,
            };
            let result = unsafe {
                ffi::ghostty_search_get(self.search.inner.as_raw(), key, (&raw mut buffer).cast())
            };
            if result == ffi::Result::OUT_OF_SPACE {
                storage
                    .inner
                    .resize(buffer.len, ffi::sized!(ffi::Selection));
                continue;
            }
            from_result(result)?;
            let values = &storage.inner[..buffer.len];
            return Ok(Matches {
                inner: values.iter(),
            });
        }
    }

    pub fn matches<'s>(&'s self, storage: &'s mut MatchBuffer) -> Result<Matches<'s>> {
        self.read_matches(storage, ffi::SearchData::MATCHES)
    }

    pub fn viewport_matches<'s>(&'s self, storage: &'s mut MatchBuffer) -> Result<Matches<'s>> {
        self.read_matches(storage, ffi::SearchData::VIEWPORT_MATCHES)
    }
}

#[derive(Debug, Default)]
pub struct MatchBuffer {
    inner: Vec<ffi::Selection>,
}

impl MatchBuffer {
    #[must_use]
    pub const fn new() -> Self {
        Self { inner: Vec::new() }
    }
}

#[derive(Debug)]
pub struct Matches<'s> {
    inner: std::slice::Iter<'s, ffi::Selection>,
}

impl<'s> Iterator for Matches<'s> {
    type Item = Selection<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|raw| unsafe { Selection::from_raw(*raw) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for Matches<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner
            .next_back()
            .map(|raw| unsafe { Selection::from_raw(*raw) })
    }
}

impl ExactSizeIterator for Matches<'_> {}
impl std::iter::FusedIterator for Matches<'_> {}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;

    fn drive(search: &mut Search<'_, '_>, terminal: &mut Terminal<'_, '_>) {
        loop {
            search.feed(terminal).unwrap();
            let mut status = search.status().unwrap();
            while status == Status::Running {
                status = search.tick().unwrap();
            }
            if status == Status::Complete {
                break;
            }
            assert_eq!(status, Status::FeedRequired);
        }
    }

    #[test]
    fn feeding_and_ticking_finds_matches() {
        let mut terminal = Terminal::new(20, 4).unwrap();
        terminal.vt_write(b"Hello hello\r\nother");
        let mut search = Search::new(&mut terminal).unwrap();

        assert_eq!(search.status().unwrap(), Status::Complete);
        assert_eq!(search.needle().unwrap(), None);
        assert!(!search.select_next(&mut terminal).unwrap());

        search.set_needle(&mut terminal, b"HELLO").unwrap();
        assert_eq!(search.needle().unwrap(), Some(&b"HELLO"[..]));
        assert_eq!(search.status().unwrap(), Status::FeedRequired);
        assert_eq!(search.total_matches().unwrap(), 0);

        drive(&mut search, &mut terminal);
        assert_eq!(search.total_matches().unwrap(), 2);

        terminal.vt_write(b"\r\nhello");
        assert_eq!(search.total_matches().unwrap(), 2);
        drive(&mut search, &mut terminal);
        assert_eq!(search.total_matches().unwrap(), 3);

        search.set_needle(&mut terminal, b"").unwrap();
        assert_eq!(search.needle().unwrap(), None);
        assert_eq!(search.status().unwrap(), Status::Complete);
    }

    #[test]
    fn selection_moves_newest_to_oldest_and_wraps() {
        let mut terminal = Terminal::new(20, 4).unwrap();
        terminal.vt_write(b"hit\r\nhit\r\nhit");
        let mut search = Search::new(&mut terminal).unwrap();
        search.set_needle(&mut terminal, b"hit").unwrap();
        search.run(&mut terminal).unwrap();
        assert_eq!(search.selected_index().unwrap(), None);

        let mut indices = Vec::new();
        for _ in 0..4 {
            assert!(search.select_next(&mut terminal).unwrap());
            indices.push(search.selected_index().unwrap().unwrap());
        }
        assert_eq!(indices, [0, 1, 2, 0]);
        assert!(search.select_prev(&mut terminal).unwrap());
        assert_eq!(search.selected_index().unwrap(), Some(2));

        let mut storage = MatchBuffer::new();
        let snapshot = search.snapshot(&mut terminal).unwrap();
        let selected = snapshot.selected_match().unwrap().unwrap();
        let oldest = snapshot.matches(&mut storage).unwrap().next_back().unwrap();
        let row = |selection: &Selection<'_>| {
            snapshot
                .terminal()
                .point_from_grid_ref(&selection.start(), crate::terminal::PointSpace::Active)
                .unwrap()
                .unwrap()
        };
        assert_eq!(row(&selected), row(&oldest));
    }

    #[test]
    fn equal_needle_keeps_results() {
        let mut terminal = Terminal::new(20, 4).unwrap();
        terminal.vt_write(b"hit\r\nhit");
        let mut search = Search::new(&mut terminal).unwrap();
        search.set_needle(&mut terminal, b"hit").unwrap();
        search.run(&mut terminal).unwrap();
        assert!(search.select_next(&mut terminal).unwrap());
        assert!(search.select_next(&mut terminal).unwrap());
        assert_eq!(search.selected_index().unwrap(), Some(1));

        search.set_needle(&mut terminal, b"HIT").unwrap();
        assert_eq!(search.selected_index().unwrap(), Some(1));
        assert_eq!(search.total_matches().unwrap(), 2);

        search.set_needle(&mut terminal, b"hi").unwrap();
        assert_eq!(search.selected_index().unwrap(), None);
        assert_eq!(search.total_matches().unwrap(), 0);
    }

    #[test]
    fn alternate_screen_results_are_separate() {
        let mut terminal = Terminal::new(20, 4).unwrap();
        terminal.vt_write(b"hit");
        let mut search = Search::new(&mut terminal).unwrap();
        search.set_needle(&mut terminal, b"hit").unwrap();
        search.run(&mut terminal).unwrap();
        assert_eq!(search.total_matches().unwrap(), 1);

        terminal.vt_write(b"\x1b[?1049h\x1b[Hhit hit hit");
        search.run(&mut terminal).unwrap();
        assert_eq!(search.total_matches().unwrap(), 3);

        terminal.vt_write(b"\x1b[?1049l");
        search.run(&mut terminal).unwrap();
        assert_eq!(search.total_matches().unwrap(), 1);
    }

    #[test]
    fn selecting_scrolls_according_to_policy() {
        let mut terminal = Terminal::new(10, 3).unwrap();
        terminal.vt_write(b"hit");
        for _ in 0..10 {
            terminal.vt_write(b"\r\nfiller");
        }
        let bottom = terminal.scrollbar().unwrap().offset;
        let select = |scroll| {
            let mut terminal = Terminal::new(10, 3).unwrap();
            terminal.vt_write(b"hit");
            for _ in 0..10 {
                terminal.vt_write(b"\r\nfiller");
            }
            let mut search = Search::new(&mut terminal).unwrap();
            search.set_scroll(scroll).unwrap();
            assert_eq!(search.scroll().unwrap(), scroll);
            search.set_needle(&mut terminal, b"hit").unwrap();
            search.run(&mut terminal).unwrap();
            assert!(search.select_next(&mut terminal).unwrap());
            terminal.scrollbar().unwrap().offset
        };

        assert_eq!(select(Scroll::None), bottom);
        assert_eq!(select(Scroll::IfNeeded), 0);
    }

    #[test]
    fn match_storage_is_reused_across_snapshots() {
        let mut terminal = Terminal::new(20, 4).unwrap();
        terminal.vt_write(b"hit hit hit");
        let mut search = Search::new(&mut terminal).unwrap();
        search.set_needle(&mut terminal, b"hit").unwrap();
        search.run(&mut terminal).unwrap();

        let mut storage = MatchBuffer::new();
        assert_eq!(
            search
                .snapshot(&mut terminal)
                .unwrap()
                .matches(&mut storage)
                .unwrap()
                .len(),
            3
        );
        let allocation = storage.inner.as_ptr();

        search.set_needle(&mut terminal, b"").unwrap();
        assert_eq!(
            search
                .snapshot(&mut terminal)
                .unwrap()
                .matches(&mut storage)
                .unwrap()
                .len(),
            0
        );

        search.set_needle(&mut terminal, b"hit").unwrap();
        search.run(&mut terminal).unwrap();
        {
            let snapshot = search.snapshot(&mut terminal).unwrap();
            let viewport: Vec<_> = snapshot.viewport_matches(&mut storage).unwrap().collect();
            assert_eq!(viewport.len(), 3);
            for selected in &viewport {
                selected.start().cell().unwrap();
                selected.end().cell().unwrap();
            }
        }
        assert_eq!(storage.inner.as_ptr(), allocation);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn history_search<'t>(terminal: &mut Terminal<'t, '_>) -> Search<'static, 't> {
        for _ in 0..5000 {
            terminal.vt_write(b"hit\r\n");
        }
        let mut search = Search::new(terminal).unwrap();
        search.set_needle(terminal, b"hit").unwrap();
        search.feed(terminal).unwrap();
        assert_eq!(search.status().unwrap(), Status::Running);
        search
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn ticking_after_terminal_drop_is_rejected() {
        let alloc = crate::alloc::testing::Guard::allocator();

        let mut terminal = Terminal::new_with_alloc(&alloc, 10, 3).unwrap();
        let mut search = history_search(&mut terminal);
        assert_eq!(search.tick().unwrap(), Status::FeedRequired);
        drop(search);
        drop(terminal);

        let mut terminal = Terminal::new_with_alloc(&alloc, 10, 3).unwrap();
        let mut search = history_search(&mut terminal);
        drop(terminal);
        assert!(matches!(search.tick(), Err(Error::InvalidValue)));
        assert_eq!(search.status().unwrap(), Status::Running);
        assert_eq!(search.needle().unwrap(), Some(&b"hit"[..]));
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn search_can_be_dropped_from_an_effect_callback() {
        let alloc = crate::alloc::testing::Guard::allocator();
        let slot = std::cell::RefCell::new(None);
        let mut terminal = Terminal::new_with_alloc(&alloc, 10, 3).unwrap();
        terminal
            .on_bell(|_| drop(slot.borrow_mut().take()))
            .unwrap();
        *slot.borrow_mut() = Some(history_search(&mut terminal));

        let mut input = b"\x07".to_vec();
        for _ in 0..5000 {
            input.extend_from_slice(b"hit\r\n");
        }
        terminal.vt_write(&input);
        assert!(slot.borrow().is_none());

        let mut search = Search::new(&mut terminal).unwrap();
        search.set_needle(&mut terminal, b"hit").unwrap();
        search.run(&mut terminal).unwrap();
        assert!(search.total_matches().unwrap() > 5000);
    }
}
