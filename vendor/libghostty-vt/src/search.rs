//! Search terminal contents, including scrollback, for a string.
//!
//! Search a terminal for a string, covering the active area and scrollback of
//! both the primary and alternate screens.
//!
//! A [`Search`] searches the terminal it was created with for a needle set
//! with [`Search::set_needle`]. It handles the hard parts of terminal search
//! internally: results stay in sync with the live screens, survive
//! primary/alternate screen switches (entering and leaving a fullscreen app
//! such as vim does not restart a scrollback search), and recover from
//! resize, reflow, resets, and scrollback pruning.
//!
//! A search starts idle. Setting the needle starts the search, changing it
//! restarts the search from scratch, and clearing it returns the search to
//! idle. Matching is byte-exact except ASCII letters, which compare
//! case-insensitively.
//!
//! # Driving a search
//!
//! Searching a large scrollback takes time, so the work is split into small
//! steps the caller drives so that the caller can control performance more
//! directly:
//!
//! - [`Search::tick`] makes a bounded amount of progress on data the search
//!   has already copied, without borrowing the terminal.
//! - [`Search::feed`] reads the terminal to copy in more data and pick up
//!   terminal changes. Feeding is the only way the search learns that the
//!   terminal changed, so keep feeding periodically while the search is in
//!   use. This requires exclusive terminal access.
//! - [`Search::run`] is a blocking convenience that feeds and ticks until the
//!   search is caught up.
//!
//! [`Status::Complete`] means the search is caught up with the terminal as of
//! the last feed. It never means finished forever, since later terminal
//! writes require another feed to be seen.
//!
//! # Matches are selections
//!
//! Every match is returned as a [`Selection`] that is not rectangular, so the
//! existing selection APIs all work on matches: format the matched text,
//! position highlight rectangles with [`Terminal::point_from_grid_ref`], hit
//! test, or make a match the terminal's selection.
//!
//! Returned matches are only valid until the next operation that modifies the
//! terminal, including [`Terminal::vt_write`], resize, reset, and drop. The
//! borrow checker enforces this: matches are read from a [`Snapshot`], which
//! borrows both the search and its terminal. Read matches after a feed, use
//! them before the terminal changes again, and re-read them rather than
//! caching them. The selected match is kept accurate internally across
//! terminal changes, so the safe way to follow a match is to re-read
//! [`Snapshot::selected_match`] after each feed.
//!
//! # Lifetime
//!
//! The search is bound to the terminal it was created with, but only borrows
//! it during operations that need it, so the terminal can be used normally in
//! between. Any number of searches, alongside other terminal readers such as
//! formatters and render states, may share one terminal.
//!
//! The search and its terminal can be dropped in either order. Dropping the
//! search first releases tracked state it holds within the terminal. If the
//! terminal is dropped first, the search detects this: calls that need the
//! terminal, and [`Search::tick`], return [`Error::InvalidValue`], reads
//! return whatever the search last saw, and dropping the search releases only
//! search-owned memory. A search cannot be rebound, so searching another
//! terminal means creating a new search.
//!
//! # Example
//!
//! ```
//! use libghostty_vt::{Terminal, search::{MatchBuffer, Search}};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut terminal = Terminal::new(80, 24)?;
//! terminal.vt_write(b"hello world");
//!
//! let mut search = Search::new(&mut terminal)?;
//! search.set_needle(&mut terminal, b"hello")?;
//! search.run(&mut terminal)?;
//!
//! // Keep the match storage around to reuse it between frames.
//! let mut storage = MatchBuffer::new();
//! let snapshot = search.snapshot(&mut terminal)?;
//! for selected in snapshot.matches(&mut storage)? {
//!     selected.start().cell()?;
//! }
//!
//! // Once the matches are no longer used, the terminal can change again.
//! terminal.vt_write(b"\r\nhello again");
//! # Ok(())
//! # }
//! ```

use std::marker::PhantomData;

use crate::{
    Terminal,
    alloc::{Allocator, Object},
    error::{Error, Result, from_optional_result, from_result},
    ffi,
    selection::Selection,
};

/// A search bound to one terminal.
///
/// See the [module documentation](self) for how to drive a search.
///
/// Every operation that needs the terminal takes it as an argument, which
/// both borrows it for the duration of the operation and checks that it is the
/// terminal this search was created with. Passing any other terminal returns
/// [`Error::InvalidValue`]. Moving the original terminal is fine.
///
/// Match snapshots prevent writes while their selections are in use:
///
/// ```compile_fail,E0499
/// use libghostty_vt::{Terminal, search::Search};
/// let mut terminal = Terminal::new(8, 2).unwrap();
/// let mut search = Search::new(&mut terminal).unwrap();
/// let snapshot = search.snapshot(&mut terminal).unwrap();
/// let selected = snapshot.selected_match().unwrap().unwrap();
/// terminal.vt_write(b"\x1bc");
/// selected.start().cell().unwrap();
/// ```
///
/// They also prevent replacing and freeing the terminal:
///
/// ```compile_fail,E0499
/// use libghostty_vt::{Terminal, search::Search};
/// let mut terminal = Terminal::new(8, 2).unwrap();
/// let mut search = Search::new(&mut terminal).unwrap();
/// let snapshot = search.snapshot(&mut terminal).unwrap();
/// let selected = snapshot.selected_match().unwrap().unwrap();
/// let original = std::mem::replace(&mut terminal, Terminal::new(8, 2).unwrap());
/// drop(original);
/// selected.start().cell().unwrap();
/// ```
///
/// Other searches cannot mutate the same terminal while a snapshot is borrowed:
///
/// ```compile_fail,E0499
/// use libghostty_vt::{Terminal, search::Search};
/// let mut terminal = Terminal::new(8, 2).unwrap();
/// let mut first = Search::new(&mut terminal).unwrap();
/// let mut second = Search::new(&mut terminal).unwrap();
/// let snapshot = first.snapshot(&mut terminal).unwrap();
/// second.select_next(&mut terminal).unwrap();
/// snapshot.selected_match().unwrap();
/// ```
///
/// `'term` is the lifetime of the terminal's allocator. libghostty keeps the
/// search and the terminal pointing at each other until one of them is freed,
/// so the search may not outlive the memory of the terminal, even one that was
/// leaked with [`std::mem::forget`] and never freed:
///
#[cfg_attr(
    feature = "allocator_api",
    doc = r"```compile_fail,E0505
use libghostty_vt::{Terminal, alloc::Allocator, search::Search};
let arena = allocator_api2::alloc::Global;
let alloc = Allocator::from(&arena);
let mut terminal = Terminal::new_with_alloc(&alloc, 8, 2).unwrap();
let mut search = Search::new(&mut terminal).unwrap();
std::mem::forget(terminal);
drop(alloc); // Would free the terminal's memory under the search.
search.tick().unwrap();
```"
)]
///
/// Likewise, the search's own memory must outlive the terminal, which detaches
/// the search when it is freed, even if the search was leaked:
///
#[cfg_attr(
    feature = "allocator_api",
    doc = r"```compile_fail,E0505
use libghostty_vt::{Terminal, alloc::Allocator, search::Search};
let arena = allocator_api2::alloc::Global;
let alloc = Allocator::from(&arena);
let mut terminal = Terminal::new(8, 2).unwrap();
let search = Search::new_with_alloc(&alloc, &mut terminal).unwrap();
std::mem::forget(search);
drop(alloc); // Would free the search's memory before the terminal detaches it.
drop(terminal);
```"
)]
#[derive(Debug)]
pub struct Search<'alloc: 'term, 'term> {
    inner: Object<'alloc, ffi::SearchImpl>,
    // The native terminal this search was created with. Comparing addresses is
    // enough to reject a different live terminal. A terminal allocated later at
    // the same address cannot be mistaken for the original either: freeing the
    // original detached this search, so libghostty rejects every call that
    // would touch the new one, ticks included, and every match is read after
    // such a call.
    terminal: ffi::Terminal,
    // Ties the search to the terminal's memory; see the type docs.
    _terminal: PhantomData<&'term ()>,
}

/// Progress state of a search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[repr(i32)]
pub enum Status {
    /// [`Search::tick`] can make progress without feeding.
    Running = ffi::SearchStatus::RUNNING,
    /// Blocked until [`Search::feed`]. This is also the state right after a
    /// needle is set, since the search has not yet seen the terminal.
    FeedRequired = ffi::SearchStatus::FEED_REQUIRED,
    /// Caught up with the terminal state as of the last feed. This never
    /// means finished forever, since later terminal writes require another
    /// feed to be seen. A search with no needle set also reports complete,
    /// since there is nothing to look for.
    Complete = ffi::SearchStatus::COMPLETE,
}

/// Scroll policy applied when a match becomes selected via
/// [`Search::select_next`] or [`Search::select_prev`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[repr(i32)]
pub enum Scroll {
    /// Scroll the viewport so the match is visible, only if it is not
    /// already visible. This is the default.
    #[default]
    IfNeeded = ffi::SearchScroll::IF_NEEDED,
    /// Never scroll the viewport.
    None = ffi::SearchScroll::NONE,
}

impl<'alloc: 'term, 'term> Search<'alloc, 'term> {
    /// Create a search bound to a terminal.
    ///
    /// The search starts idle with no needle: it reports [`Status::Complete`]
    /// and finds nothing. Set a needle with [`Self::set_needle`] to start
    /// searching.
    pub fn new(terminal: &mut Terminal<'term, '_>) -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(terminal, std::ptr::null()) }
    }

    /// Create a search bound to a terminal, with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(
        alloc: &'alloc Allocator<'ctx>,
        terminal: &mut Terminal<'term, '_>,
    ) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(terminal, alloc.to_raw()) }
    }

    unsafe fn new_inner(
        terminal: &mut Terminal<'term, '_>,
        alloc: *const ffi::Allocator,
    ) -> Result<Self> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: The exclusive terminal borrow serializes the registration
        // with all other access to the terminal.
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

    /// Set the needle to search for. The bytes are copied. Matching is
    /// byte-exact except ASCII letters, which compare case-insensitively.
    ///
    /// Changing the needle restarts the search from scratch and drops all
    /// results. As an exception, setting a needle equal to the current one
    /// (compared the same way as matching) keeps existing results, so find
    /// bars can resubmit freely. An empty needle clears it and returns the
    /// search to idle.
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
        // SAFETY: The needle is copied, and the exclusive terminal borrow
        // serializes releasing tracked state within the terminal.
        from_result(unsafe {
            ffi::ghostty_search_set(
                self.inner.as_raw(),
                ffi::SearchOption::NEEDLE,
                (&raw const raw).cast(),
            )
        })?;
        Ok(self)
    }

    /// The needle this search is looking for, or `None` when no needle is
    /// set.
    pub fn needle(&self) -> Result<Option<&[u8]>> {
        let mut raw = ffi::String::default();
        // SAFETY: Reading only touches search-owned memory.
        let result = unsafe {
            ffi::ghostty_search_get(
                self.inner.as_raw(),
                ffi::SearchData::NEEDLE,
                (&raw mut raw).cast(),
            )
        };
        // SAFETY: The bytes are borrowed from the search and remain valid
        // until the needle is changed, which needs `&mut self`.
        Ok(from_optional_result(result, raw)?.map(|raw| unsafe { raw.to_bytes() }))
    }

    /// Read the terminal to update the search.
    ///
    /// Each feed catches the search up with the terminal: it reconciles the
    /// tracked screens against the live ones, re-scans the active area,
    /// refreshes the viewport match list, gives the scrollback searcher its
    /// next chunk of data, and prunes results that scrollback eviction
    /// invalidated. Feeding is also the only way the search learns about
    /// terminal changes, so keep feeding periodically while the search is in
    /// use, even after it reports complete.
    ///
    /// Each call does a bounded amount of work.
    pub fn feed(&mut self, terminal: &mut Terminal<'_, '_>) -> Result<()> {
        self.check_terminal(terminal)?;
        // SAFETY: The exclusive terminal borrow serializes reading it.
        from_result(unsafe { ffi::ghostty_search_feed(self.inner.as_raw()) })
    }

    /// Make a bounded amount of search progress, returning the new status.
    ///
    /// This only works on data the search has already copied, so it doesn't
    /// need to borrow the terminal. Call it in a loop while the status is
    /// [`Status::Running`]. When the status becomes [`Status::FeedRequired`],
    /// call [`Self::feed`] to unblock it.
    ///
    /// Returns [`Error::InvalidValue`] after the terminal was dropped.
    pub fn tick(&mut self) -> Result<Status> {
        let mut status = ffi::SearchStatus::COMPLETE;
        // SAFETY: libghostty rejects ticking a search whose terminal was
        // freed, since ticking a scrollback search reads a pin the terminal
        // tracks. Otherwise, ticking only touches search-owned memory.
        from_result(unsafe { ffi::ghostty_search_tick(self.inner.as_raw(), &raw mut status) })?;
        status.try_into().map_err(|_| Error::InvalidValue)
    }

    /// Feed and tick until the search is caught up with the terminal.
    ///
    /// This is a blocking convenience for one-shot and single-threaded
    /// embedders. It always performs at least one feed, so it also picks up
    /// any terminal changes since the last feed, then loops until the status
    /// is [`Status::Complete`]. Searching a large scrollback can take a while,
    /// so interactive embedders should drive [`Self::tick`] and
    /// [`Self::feed`] themselves.
    pub fn run(&mut self, terminal: &mut Terminal<'_, '_>) -> Result<()> {
        self.check_terminal(terminal)?;
        // SAFETY: The exclusive terminal borrow serializes reading it.
        from_result(unsafe { ffi::ghostty_search_run(self.inner.as_raw()) })
    }

    // Only the explicitly typed accessors below choose an output type.
    fn get<T: Default>(&self, key: ffi::SearchData::Type) -> Result<T> {
        let mut value = T::default();
        // SAFETY: Reading only touches search-owned memory, and `T` matches
        // the output type documented for `key`.
        from_result(unsafe {
            ffi::ghostty_search_get(self.inner.as_raw(), key, (&raw mut value).cast())
        })?;
        Ok(value)
    }

    /// Current search status, as of the last feed or tick.
    pub fn status(&self) -> Result<Status> {
        self.get::<ffi::SearchStatus::Type>(ffi::SearchData::STATUS)?
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }

    /// Total matches found so far on the active screen. Zero until the first
    /// feed.
    ///
    /// Like all reads, this reflects the terminal's active screen as of the
    /// last feed. When the running application switches to the alternate
    /// screen, the next feed switches counts, matches, and selection to that
    /// screen's results. Primary screen results, including completed
    /// scrollback searches, are retained and restored on the way back.
    pub fn total_matches(&self) -> Result<usize> {
        self.get(ffi::SearchData::TOTAL_MATCHES)
    }

    /// Index of the selected match, or `None` when nothing is selected.
    ///
    /// This indexes the newest to oldest ordering of [`Snapshot::matches`],
    /// where 0 is the newest match, so a "k of n" find bar renders
    /// `index + 1` of [`Self::total_matches`].
    pub fn selected_index(&self) -> Result<Option<usize>> {
        let mut index = 0usize;
        // SAFETY: Reading only touches search-owned memory.
        let result = unsafe {
            ffi::ghostty_search_get(
                self.inner.as_raw(),
                ffi::SearchData::SELECTED_INDEX,
                (&raw mut index).cast(),
            )
        };
        from_optional_result(result, index)
    }

    /// Current scroll policy.
    pub fn scroll(&self) -> Result<Scroll> {
        self.get::<ffi::SearchScroll::Type>(ffi::SearchData::SELECT_SCROLL)?
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }

    /// Set the scroll policy applied by [`Self::select_next`] and
    /// [`Self::select_prev`]. The policy persists until changed.
    pub fn set_scroll(&mut self, scroll: Scroll) -> Result<&mut Self> {
        let raw: ffi::SearchScroll::Type = scroll.into();
        // SAFETY: This only modifies search-owned state.
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
        // SAFETY: The value must be NULL, and the exclusive terminal borrow
        // serializes reading and scrolling it.
        let result = unsafe { ffi::ghostty_search_set(self.inner.as_raw(), key, std::ptr::null()) };
        Ok(from_optional_result(result, ())?.is_some())
    }

    /// Select the next match, moving toward older content: from the bottom
    /// of the screen upward into history, the direction a search from the
    /// prompt usually wants. Wraps around past the oldest match.
    ///
    /// This catches up with the terminal first, so it is safe to call at any
    /// time relative to feeds. The viewport scrolls to the newly selected
    /// match according to [`Self::set_scroll`]. Returns `false` when there
    /// are no matches.
    pub fn select_next(&mut self, terminal: &mut Terminal<'_, '_>) -> Result<bool> {
        self.select(terminal, ffi::SearchOption::SELECT_NEXT)
    }

    /// Select the previous match, moving toward newer content, wrapping
    /// around past the newest match. Otherwise identical to
    /// [`Self::select_next`].
    pub fn select_prev(&mut self, terminal: &mut Terminal<'_, '_>) -> Result<bool> {
        self.select(terminal, ffi::SearchOption::SELECT_PREV)
    }

    /// Feed the search, then borrow its matches together with the terminal.
    ///
    /// The snapshot keeps the terminal borrowed, so the terminal cannot be
    /// modified while any match read from it is still in use.
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
        // SAFETY: libghostty releases tracked state within a live terminal,
        // or only search-owned memory once the terminal was freed.
        //
        // Releasing that state must be serialized with all other access to
        // the terminal. The terminal is not shared across threads, so the
        // only other access that can be in progress is a libghostty call that
        // is running Rust code, when the search is dropped from inside one:
        // an effect callback during a VT write, reset or resize, or the
        // reader or writer of a streaming call such as snapshot encoding and
        // decoding or continuation export. That is fine too: releasing only
        // unregisters the search from the terminal and untracks its pins.
        // Effect callbacks are invoked by the stream handler while it handles
        // a sequence, never from within a page list operation, and streaming
        // calls only hand bytes to their reader or writer, so the terminal
        // isn't iterating its searches or tracked pins at any of these points,
        // and it never holds on to the pins this search owns. Upstream likewise only forbids re-entering VT
        // writes from callbacks, and expects other terminal calls, such as
        // updating a render state, to be made from them. It doesn't document
        // freeing a search from a callback, though, so this relies on how the
        // pinned version is implemented.
        unsafe { ffi::ghostty_search_free(self.inner.as_raw()) };
    }
}

/// Matches of a [`Search`], borrowed together with their terminal.
///
/// Created by [`Search::snapshot`].
#[derive(Debug)]
pub struct Snapshot<'s, 'alloc: 'term, 'term, 'ta: 'cb, 'cb> {
    search: &'s Search<'alloc, 'term>,
    terminal: &'s Terminal<'ta, 'cb>,
}

impl<'ta: 'cb, 'cb> Snapshot<'_, '_, '_, 'ta, 'cb> {
    /// The terminal that produced these matches.
    #[must_use]
    pub fn terminal(&self) -> &Terminal<'ta, 'cb> {
        self.terminal
    }

    /// The selected match, or `None` when nothing is selected.
    pub fn selected_match(&self) -> Result<Option<Selection<'_>>> {
        let mut raw = ffi::sized!(ffi::Selection);
        // SAFETY: Reading only touches search-owned memory.
        let result = unsafe {
            ffi::ghostty_search_get(
                self.search.inner.as_raw(),
                ffi::SearchData::SELECTED_MATCH,
                (&raw mut raw).cast(),
            )
        };
        // SAFETY: The selection points into terminal pages, which cannot
        // change while this snapshot borrows the terminal.
        Ok(from_optional_result(result, raw)?.map(|raw| unsafe { Selection::from_raw(raw) }))
    }

    fn read_matches<'s>(
        &'s self,
        storage: &'s mut MatchBuffer,
        key: ffi::SearchData::Type,
    ) -> Result<Matches<'s>> {
        loop {
            let mut buffer = ffi::SelectionBuffer {
                // An empty buffer queries the required capacity, which
                // libghostty expects to be requested with a NULL pointer.
                ptr: if storage.inner.is_empty() {
                    std::ptr::null_mut()
                } else {
                    storage.inner.as_mut_ptr()
                },
                cap: storage.inner.len(),
                len: 0,
            };
            // SAFETY: Reading only touches search-owned memory, and `buffer`
            // describes `cap` initialized selections.
            let result = unsafe {
                ffi::ghostty_search_get(self.search.inner.as_raw(), key, (&raw mut buffer).cast())
            };
            if result == ffi::Result::OUT_OF_SPACE {
                // Growing is only needed when the storage is too small, so
                // existing capacity is reused on later snapshots.
                // libghostty reports the required length, which is larger.
                storage
                    .inner
                    .resize(buffer.len, ffi::sized!(ffi::Selection));
                continue;
            }
            from_result(result)?;
            // Never expose stale entries left over from an earlier, longer
            // read. On success libghostty wrote `len <= cap` entries.
            let values = &storage.inner[..buffer.len];
            return Ok(Matches {
                inner: values.iter(),
            });
        }
    }

    /// All matches on the active screen, ordered newest to oldest, from the
    /// bottom of the active area up through scrollback.
    ///
    /// The matches are read into `storage`, which is reused across calls.
    /// Both this snapshot and the storage stay borrowed while the matches are
    /// in use.
    pub fn matches<'s>(&'s self, storage: &'s mut MatchBuffer) -> Result<Matches<'s>> {
        self.read_matches(storage, ffi::SearchData::MATCHES)
    }

    /// Matches on the pages covering the viewport, for drawing highlight
    /// rectangles. The list reflects the viewport as of the last feed.
    ///
    /// Matches are found a page at a time, so the list can include matches
    /// slightly outside the visible viewport when they share a page with it.
    /// Ghostty's own renderer behaves the same way. Converting each match to
    /// viewport coordinates with [`Terminal::point_from_grid_ref`] clips this
    /// naturally: skip matches that fail the conversion or whose row is
    /// beyond the visible row count.
    ///
    /// Storage is reused and borrowed like with [`Self::matches`].
    pub fn viewport_matches<'s>(&'s self, storage: &'s mut MatchBuffer) -> Result<Matches<'s>> {
        self.read_matches(storage, ffi::SearchData::VIEWPORT_MATCHES)
    }
}

/// Reusable storage for reading search matches.
///
/// Keep one buffer across snapshots to avoid reallocating it every frame. Its
/// contents can only be read through the [`Matches`] of the snapshot that
/// last filled it.
#[derive(Debug, Default)]
pub struct MatchBuffer {
    inner: Vec<ffi::Selection>,
}

impl MatchBuffer {
    /// Create empty storage. It grows as needed when matches are read.
    #[must_use]
    pub const fn new() -> Self {
        Self { inner: Vec::new() }
    }
}

/// An iterator over matches read from a [`Snapshot`].
///
/// A match cannot outlive its terminal:
///
/// ```compile_fail,E0505
/// use libghostty_vt::{Terminal, search::{Search, MatchBuffer}};
/// let mut terminal = Terminal::new(8, 2).unwrap();
/// let mut search = Search::new(&mut terminal).unwrap();
/// let mut storage = MatchBuffer::new();
/// let snapshot = search.snapshot(&mut terminal).unwrap();
/// let selected = snapshot.matches(&mut storage).unwrap().next().unwrap();
/// drop(terminal);
/// selected.start().cell().unwrap();
/// ```
///
/// Reusing the storage cannot overwrite a match that is still in use:
///
/// ```compile_fail,E0499
/// use libghostty_vt::{Terminal, search::{Search, MatchBuffer}};
/// let mut terminal = Terminal::new(8, 2).unwrap();
/// let mut search = Search::new(&mut terminal).unwrap();
/// let mut storage = MatchBuffer::new();
/// let snapshot = search.snapshot(&mut terminal).unwrap();
/// let selected = snapshot.matches(&mut storage).unwrap().next().unwrap();
/// snapshot.viewport_matches(&mut storage).unwrap();
/// selected.start().cell().unwrap();
/// ```
#[derive(Debug)]
pub struct Matches<'s> {
    inner: std::slice::Iter<'s, ffi::Selection>,
}

impl<'s> Iterator for Matches<'s> {
    type Item = Selection<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: Only `Snapshot::read_matches` creates this iterator, so the
        // selections point into terminal pages that cannot change while the
        // snapshot is borrowed for `'s`.
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
        // SAFETY: Ditto
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

    /// Drive a search with feeds and ticks, as an interactive embedder would.
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

        // An idle search is complete and finds nothing.
        assert_eq!(search.status().unwrap(), Status::Complete);
        assert_eq!(search.needle().unwrap(), None);
        assert!(!search.select_next(&mut terminal).unwrap());

        // A new needle needs a feed before it can make progress.
        search.set_needle(&mut terminal, b"HELLO").unwrap();
        assert_eq!(search.needle().unwrap(), Some(&b"HELLO"[..]));
        assert_eq!(search.status().unwrap(), Status::FeedRequired);
        assert_eq!(search.total_matches().unwrap(), 0);

        drive(&mut search, &mut terminal);
        assert_eq!(search.total_matches().unwrap(), 2);

        // Later writes are only seen after another feed.
        terminal.vt_write(b"\r\nhello");
        assert_eq!(search.total_matches().unwrap(), 2);
        drive(&mut search, &mut terminal);
        assert_eq!(search.total_matches().unwrap(), 3);

        // Clearing the needle returns the search to idle.
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
        // Next moves toward older content and wraps past the oldest match.
        assert_eq!(indices, [0, 1, 2, 0]);
        // Previous moves toward newer content and wraps past the newest.
        assert!(search.select_prev(&mut terminal).unwrap());
        assert_eq!(search.selected_index().unwrap(), Some(2));

        // The selected match is the newest-to-oldest entry at that index.
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

        // Equal as compared by matching, so results and selection survive.
        search.set_needle(&mut terminal, b"HIT").unwrap();
        assert_eq!(search.selected_index().unwrap(), Some(1));
        assert_eq!(search.total_matches().unwrap(), 2);

        // A different needle restarts from scratch.
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

        // Primary screen results are restored on the way back.
        terminal.vt_write(b"\x1b[?1049l");
        search.run(&mut terminal).unwrap();
        assert_eq!(search.total_matches().unwrap(), 1);
    }

    #[test]
    fn selecting_scrolls_according_to_policy() {
        // Push the only match into scrollback, below a viewport at the bottom.
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

        // A shorter result never exposes stale entries from the longer one.
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
        // Reading the same number of matches again didn't reallocate.
        assert_eq!(storage.inner.as_ptr(), allocation);
    }

    /// Fill a terminal with enough matching lines that the search has
    /// scrollback history to search, not just the active area.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn history_search<'t>(terminal: &mut Terminal<'t, '_>) -> Search<'static, 't> {
        for _ in 0..5000 {
            terminal.vt_write(b"hit\r\n");
        }
        let mut search = Search::new(terminal).unwrap();
        search.set_needle(terminal, b"hit").unwrap();
        // Feeding searches the active area and hands the history searcher
        // its first page, which the next tick searches.
        search.feed(terminal).unwrap();
        assert_eq!(search.status().unwrap(), Status::Running);
        search
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn ticking_after_terminal_drop_is_rejected() {
        let alloc = crate::alloc::testing::Guard::allocator();

        // Make sure the search really has history to tick through. Without
        // it, ticking would complete without touching anything, instead of
        // blocking on the next history feed.
        let mut terminal = Terminal::new_with_alloc(&alloc, 10, 3).unwrap();
        let mut search = history_search(&mut terminal);
        assert_eq!(search.tick().unwrap(), Status::FeedRequired);
        drop(search);
        drop(terminal);

        // Ticking the history search reads a pin tracked by the terminal, so
        // it must not happen once the terminal is gone.
        let mut terminal = Terminal::new_with_alloc(&alloc, 10, 3).unwrap();
        let mut search = history_search(&mut terminal);
        drop(terminal);
        assert!(matches!(search.tick(), Err(Error::InvalidValue)));
        // Everything else still works, reporting what the search last saw.
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

        // Release the search's tracked pins in the middle of a write that
        // keeps growing the scrollback.
        let mut input = b"\x07".to_vec();
        for _ in 0..5000 {
            input.extend_from_slice(b"hit\r\n");
        }
        terminal.vt_write(&input);
        assert!(slot.borrow().is_none());

        // The terminal's remaining tracked state is intact.
        let mut search = Search::new(&mut terminal).unwrap();
        search.set_needle(&mut terminal, b"hit").unwrap();
        search.run(&mut terminal).unwrap();
        assert!(search.total_matches().unwrap() > 5000);
    }
}
