//! Types and functions around terminal state management.

use std::{io::Write, marker::PhantomData, mem::MaybeUninit, num::NonZeroU16, ptr::NonNull};

use crate::{
    alloc::{Allocator, Bytes, Object},
    error::{
        Error, Result, from_optional_result, from_optional_result_uninit,
        from_optional_result_with_len, from_result, from_result_with_len,
    },
    ffi::{self, TerminalData as Data, TerminalOption as Opt},
    key, mouse, osc,
    screen::{GridRef, Screen, TrackedGridRef},
    style::{self, Palette, RawPalette, RgbColor},
};

#[doc(inline)]
pub use ffi::{SizeReportSize, TerminalScrollbar as Scrollbar};

/// Complete terminal emulator state and rendering.
///
/// A terminal instance manages the full emulator state including the screen,
/// scrollback, cursor, styles, modes, and VT stream processing.
///
/// Once a terminal session is up and running, you can configure a key encoder
/// to write keyboard input via [`key::Encoder::set_options_from_terminal`].
///
/// ## Example: VT stream processing
///
/// ```
/// use libghostty_vt::Terminal;
///
/// // Create a terminal
/// let mut terminal = Terminal::new(80, 24).unwrap();
///
/// // Feed VT data into the terminal
/// terminal.vt_write(b"Hello, World!\r\n");
///
/// // ANSI color codes: ESC[1;32m = bold green, ESC[0m = reset
/// terminal.vt_write(b"\x1b[1;32mGreen Text\x1b[0m\r\n");
///
/// // Cursor positioning: ESC[1;1H = move to row 1, column 1
/// terminal.vt_write(b"\x1b[1;1HTop-left corner\r\n");
///
/// // Cursor movement: ESC[5B = move down 5 lines
/// terminal.vt_write(b"\x1b[5B");
/// terminal.vt_write(b"Moved down!\r\n");
///
/// // Erase line: ESC[2K = clear entire line
/// terminal.vt_write(b"\x1b[2K");
/// terminal.vt_write(b"New content\r\n");
///
/// // Multiple lines
/// terminal.vt_write(b"Line A\r\nLine B\r\nLine C\r\n");
/// ```
///
/// # Effects
///
/// By default, the terminal sequence processing with [`Terminal::vt_write`]
/// only process sequences that directly affect terminal state and ignores
/// sequences that have side effect behavior or require responses. These
/// sequences include things like bell characters, title changes, device
/// attributes queries, and more. To handle these sequences, the user
/// must configure "effects."
///
/// Effects are callbacks that the terminal invokes, mostly in response to VT
/// sequences processed during [`Terminal::vt_write`]. They let the embedding
/// application react to terminal-initiated events such as bell characters,
/// title changes, device status report responses, and more.
///
/// Each effect is registered with its corresponding `Terminal::on_<effect>`
/// function, which accepts a closure with access to the terminal state and
/// possibly other parameters. Some examples include [`Terminal::on_bell`]
/// and [`Terminal::on_pty_write`].
///
/// All callbacks are invoked synchronously, mostly during
/// [`Terminal::vt_write`]. A few also fire from [`Terminal::reset`] and
/// [`Terminal::resize`], such as [`Terminal::on_render_hold`] and the
/// in-band size report sent through [`Terminal::on_pty_write`].
/// Callbacks must be very careful to not block for too long or perform
/// expensive operations, since they are blocking further IO processing.
///
/// ## Shared state
///
/// **Unlike the C API**, you *cannot* specify arbitrary user data that's
/// shared between all callbacks, mainly because a safe, idiomatic Rust
/// equivalent of this pattern is very difficult to implement and use
/// due to Rust's much stricter safety guarantees. In turn, we use the
/// user data internally for callback dispatch purposes.
///
/// You should instead use types that allow safe *interior mutability*
/// (e.g. [`Cell`](std::cell::Cell) or [`RefCell`](std::cell::RefCell))
/// and pass a shared reference into each effect handler that needs to mutate
/// the shared state. Note that reference counting mechanisms like
/// [`Rc`](std::rc::Rc) and [`Arc`](std::sync::Arc) are optional.
///
/// ## Example: Registering effects and processing VT data
///
/// ```rust
/// use std::cell::Cell;
/// use libghostty_vt::Terminal;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Set up a simple bell counter.
/// //
/// // `usize` is a simple, `Copy`able type, which means `Cell`s are
/// // perfectly suitable here. More complex, non-`Copy` types should
/// // use `RefCell`s instead.
/// //
/// // This has to be done before the terminal is created, since
/// // its effect handlers will continue to refer to the bell counter
/// // during the lifetime of the terminal.
/// let bell_count = Cell::new(0usize);
///
/// let mut terminal = Terminal::new(80, 24)?;
/// terminal
///     .on_pty_write(|_term, data| {
///         println!("Replying {} bytes to the PTY", data.len());
///     })?
///    .on_bell({
///        // Explicitly borrow the bell count, or otherwise `move`
///        // will attempt to capture the entire `Cell` and cause a
///        // compiler error
///        let bell_count = &bell_count;
///        move |_term| {
///            bell_count.update(|v| v + 1);
///            println!("Bell! (count = {})", bell_count.get())
///        }
///     })?
///    .on_title_changed(|term| {
///        // Query the cursor position to confirm the terminal processed the
///        // title change (the title itself is tracked by the embedder via the
///        // OSC parser or its own state).
///        let col = term.cursor_x().unwrap();
///        println!("Title changed! (cursor at col {col})");
///    })?;
///
/// // Feed VT data that triggers effects:
/// // 1. Bell (BEL = 0x07)
/// terminal.vt_write(b"\x07");
/// // 2. Title change (OSC 2 ; <title> ST)
/// terminal.vt_write(b"\x1b]2;Hello Effects\x1b\\");
/// // 3. Device status report (DECRQM for wraparound mode ?7)
/// //    triggers write_pty with the response
/// terminal.vt_write(b"\x1B[?7$p");
/// // 4. Another bell to show the counter increments
/// terminal.vt_write(b"\x07");
///
/// assert_eq!(bell_count.get(), 2);
/// # Ok(())}
/// ```
///
/// # Color theme
///
/// The terminal maintains a set of colors used for rendering: a foreground
/// color, a background color, a cursor color, and a 256-color palette. Each
/// of these has two layers: a **default** value set by the embedder, and an
/// **override** value that programs running in the terminal can set via OSC
/// escape sequences (e.g. OSC 10/11/12 for foreground/background/cursor,
/// OSC 4 for individual palette entries).
///
/// ## Default colors
///
/// Use [`Terminal::set_default_fg_color`], [`Terminal::set_default_bg_color`],
/// [`Terminal::set_default_cursor_color`] and [`Terminal::set_default_color_palette`]
/// to configure the default colors. These represent the theme or configuration
/// chosen by the embedder. Passing `None` clears the default, leaving the color
/// unset.
///
/// For the palette, passing `None` resets to the built-in default palette.
/// The palette set operation preserves any per-index OSC overrides that programs
/// have applied; only unmodified indices are updated.
///
/// ## Reading colors
///
/// Use functions like [`Terminal::default_cursor_color`],
/// [`Terminal::bg_color`], [`Terminal::default_color_palette`], etc. to read
/// colors. There are two variants for each color: the **effective** value
/// (which returns the OSC override if one is active, otherwise the default)
/// and the **default** value (which ignores any OSC overrides).
///
/// For foreground, background, and cursor colors, the getters return `Ok(None)`
/// if no color is configured (neither a default nor an OSC override).
/// The palette getters always succeed since the palette always has a value
/// (the built-in default if nothing else is set).
///
/// ## Setting a color theme
///
/// ```
/// use libghostty_vt::{
///     style::{RgbColor, PaletteIndex},
///     Error,
///     Terminal,
/// };
///
/// fn set_color_theme(terminal: &mut Terminal<'_, '_>) -> Result<(), Error> {
///     // Set default foreground (light gray) and background (dark)
///     terminal
///         .set_default_fg_color(Some(
///             RgbColor { r: 0xDD, g: 0xDD, b: 0xDD }
///         ))?
///         .set_default_bg_color(Some(
///             RgbColor { r: 0x1E, g: 0x1E, b: 0x2E }
///         ))?
///         .set_default_cursor_color(Some(
///             RgbColor { r: 0xF5, g: 0xE0, b: 0xDC }
///         ))?;
///     
///     // Set a custom palette — start from the built-in default and override
///     // the first 8 entries with a custom dark theme.
///     let mut palette = terminal.default_color_palette()?;
///     palette.set(PaletteIndex::BLACK, RgbColor { r: 0x45, g: 0x47, b: 0x5A });
///     palette.set(PaletteIndex::RED, RgbColor { r: 0xF3, g: 0x8B, b: 0xA8 });
///     palette.set(PaletteIndex::GREEN, RgbColor { r: 0xA6, g: 0xE3, b: 0xA1 });
///     palette.set(PaletteIndex::YELLOW, RgbColor { r: 0xF9, g: 0xE2, b: 0xAF });
///     palette.set(PaletteIndex::BLUE, RgbColor { r: 0x89, g: 0xB4, b: 0xFA });
///     palette.set(PaletteIndex::MAGENTA, RgbColor { r: 0xF5, g: 0xC2, b: 0xE7 });
///     palette.set(PaletteIndex::CYAN, RgbColor { r: 0x94, g: 0xE2, b: 0xD5 });
///     palette.set(PaletteIndex::WHITE, RgbColor { r: 0xBA, g: 0xC2, b: 0xDE });
///     
///     terminal.set_default_color_palette(Some(palette))?;
///     Ok(())
/// }
/// ```
///
#[derive(Debug)]
pub struct Terminal<'alloc: 'cb, 'cb> {
    pub(crate) inner: Object<'alloc, ffi::TerminalImpl>,
    // Own the allocation through a raw pointer so moving Terminal does not
    // retag a Box and invalidate the userdata pointer retained by C. Drop
    // reconstructs the Box only after freeing the native terminal.
    vtable: *mut VTable<'alloc, 'cb>,
    // Unique for the life of the process, unlike the handle's address, which
    // the allocator may hand to a later terminal. The incremental snapshot
    // decoder uses it to tell whether it still holds the terminal it decodes
    // into. Zero for the borrowed views passed to callbacks.
    pub(crate) id: u64,
}

/// Default visual style used when the cursor style is reset.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum CursorStyle {
    /// Bar cursor (DECSCUSR 5, 6).
    Bar = ffi::TerminalCursorStyle::BAR,
    /// Block cursor (DECSCUSR 1, 2).
    Block = ffi::TerminalCursorStyle::BLOCK,
    /// Underline cursor (DECSCUSR 3, 4).
    Underline = ffi::TerminalCursorStyle::UNDERLINE,
    /// Hollow block cursor.
    BlockHollow = ffi::TerminalCursorStyle::BLOCK_HOLLOW,
}

impl<'alloc: 'cb, 'cb> Terminal<'alloc, 'cb> {
    /// Create a new terminal instance.
    ///
    /// The terminal starts with various reasonable defaults e.g. around
    /// scrollback limits. Use the `Terminal::set_*` family of methods
    /// to change any options prior to using the terminal.
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null(), cols, rows) }
    }

    /// Create a new terminal instance with a custom allocator.
    ///
    /// The terminal starts with various reasonable defaults e.g. around
    /// scrollback limits. Use the `Terminal::set_*` family of methods
    /// to change any options prior to using the terminal.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(
        alloc: &'alloc Allocator<'ctx>,
        cols: u16,
        rows: u16,
    ) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw(), cols, rows) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator, cols: u16, rows: u16) -> Result<Self> {
        let mut raw: ffi::Terminal = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_terminal_new(alloc, &raw mut raw, cols, rows) };
        from_result(result)?;
        unsafe { Self::from_raw(raw) }
    }

    pub(crate) unsafe fn from_raw(raw: ffi::Terminal) -> Result<Self> {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Ok(Self {
            inner: Object::new(raw)?,
            vtable: Box::into_raw(Box::new(VTable::default())),
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        })
    }

    /// Write VT-encoded data to the terminal for processing.
    ///
    /// Feeds raw bytes through the terminal's VT stream parser, updating
    /// terminal state accordingly. By default, sequences that require output
    /// (queries, device status reports) are silently ignored.
    /// Use [`Terminal::on_pty_write`] to install a callback that receives
    /// response data.
    ///
    /// This never fails. Any erroneous input or errors in processing the input
    /// are logged internally but do not cause this function to fail because
    /// this input is assumed to be untrusted and from an external source; so
    /// the primary goal is to keep the terminal state consistent and not allow
    /// malformed input to corrupt or crash.    
    pub fn vt_write(&mut self, data: &[u8]) {
        unsafe { ffi::ghostty_terminal_vt_write(self.inner.as_raw(), data.as_ptr(), data.len()) }
    }

    /// Write VT-encoded data, but only the shortest prefix needed to reach
    /// ground.
    ///
    /// Ground is when the stream isn't in the middle of any type of sequence:
    /// UTF-8, ESC, CSI, OSC, etc. It is the stateless point of the stream.
    ///
    /// This is useful to know because it is a point at which you can safely
    /// insert out-of-band VT sequences. For example, while reading from a pty
    /// if you want to make your own changes, you can wait until the pty input
    /// reaches ground, then write yours.
    ///
    /// If the stream is already at ground then this consumes nothing and
    /// returns `Some(0)`. Otherwise it returns `Some(n)` with the number of
    /// bytes consumed before reaching ground, including the byte that reaches
    /// it, or `None` if the full slice was consumed without reaching ground.
    ///
    /// Like [`Self::vt_write`], the input is assumed to be untrusted.
    pub fn vt_write_until_ground(&mut self, data: &[u8]) -> Result<Option<usize>> {
        let mut stop = ffi::sized!(ffi::TerminalWriteStop);
        let result = unsafe {
            ffi::ghostty_terminal_vt_write_until(
                self.inner.as_raw(),
                data.as_ptr(),
                data.len(),
                ffi::TerminalBoundary::GROUND as u32,
                &raw mut stop,
            )
        };
        from_optional_result_with_len(result, stop.consumed)
    }

    /// Whether VT processing is at ground.
    ///
    /// See [`Self::vt_write_until_ground`] for what ground means and why it
    /// is useful.
    pub fn is_vt_ground(&self) -> Result<bool> {
        self.get(Data::VT_GROUND)
    }

    /// Resize the terminal to the given dimensions.
    ///
    /// Changes the number of columns and rows in the terminal. The primary
    /// screen will reflow content if wraparound mode is enabled; the alternate
    /// screen does not reflow. If the dimensions are unchanged, the grid is
    /// left as is, but everything below still applies.
    ///
    /// This also updates the terminal's pixel dimensions (used for image
    /// protocols and size reports), disables synchronized output mode (allowed
    /// by the spec so that resize results are shown immediately), and sends an
    /// in-band size report if mode 2048 is enabled.
    ///
    /// If synchronized output was enabled, the [render hold](Self::on_render_hold)
    /// callback is invoked to report that the hold ended.
    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_terminal_resize(
                self.inner.as_raw(),
                cols,
                rows,
                cell_width_px,
                cell_height_px,
            )
        };
        from_result(result)
    }

    /// Perform a full reset of the terminal (RIS).
    ///
    /// Resets all terminal state back to its initial configuration,
    /// including modes, scrollback, scrolling region, and screen contents.
    /// The terminal dimensions are preserved.
    ///
    /// If synchronized output was enabled, the [render hold](Self::on_render_hold)
    /// callback is invoked to report that the hold ended.
    pub fn reset(&mut self) {
        unsafe { ffi::ghostty_terminal_reset(self.inner.as_raw()) }
    }

    /// Clear the primary screen and scrollback without resetting modes or the VT parser.
    ///
    /// Returns true at a semantic prompt: the caller should send form feed (0x0C)
    /// to the shell to repaint its input. Otherwise, removes rows above the cursor
    /// and deletes every Kitty image, as Ghostty's own clear does.
    /// Does nothing on the alternate screen. Does not invoke the PTY callback.
    pub fn clear_screen(&mut self) -> bool {
        // SAFETY: The terminal is live and exclusively borrowed for this mutation.
        unsafe { ffi::ghostty_terminal_clear_screen(self.inner.as_raw()) }
    }

    /// Scroll the terminal viewport.
    ///
    /// Scrolls the terminal's viewport according to the given behavior.
    /// See [`ScrollViewport::DeltaPrompt`] for the navigation behavior.
    pub fn scroll_viewport(&mut self, scroll: ScrollViewport) {
        unsafe { ffi::ghostty_terminal_scroll_viewport(self.inner.as_raw(), scroll.into()) }
    }

    /// Resolve a point in the terminal grid to a grid reference.
    ///
    /// Resolves the given point (which can be in active, viewport, screen,
    /// or history coordinates) to a grid reference for that location. Use
    /// [`GridRef::cell`] and [`GridRef::row`] to extract the cell and row.
    ///
    /// Lookups in the active region and viewport are fast. Lookups in the
    /// screen and history may require traversing the full scrollback page
    /// list to resolve the y coordinate, so they can be expensive for large
    /// scrollback buffers.
    ///
    /// This function isn't meant to be used as the core of render loop. It
    /// isn't built to sustain the framerates needed for rendering large
    /// screens. Use the [render state API](crate::render::RenderState) for
    /// that. This API is instead meant for less strictly performance-sensitive
    /// use cases.
    pub fn grid_ref(&self, point: Point) -> Result<GridRef<'_>> {
        let mut grid_ref = ffi::sized!(ffi::GridRef);
        let result = unsafe {
            ffi::ghostty_terminal_grid_ref(self.inner.as_raw(), point.into(), &raw mut grid_ref)
        };
        from_result(result)?;
        Ok(unsafe { GridRef::from_raw(grid_ref) })
    }

    /// Create an owned tracked grid reference for a terminal point.
    ///
    /// This is the tracked variant of [`Terminal::grid_ref`]. The returned handle
    /// follows the referenced cell as the terminal's page list is modified:
    /// scrolling, pruning, resize/reflow, and other page-list operations update
    /// the tracked reference automatically.
    ///
    /// The reference is attached to the terminal screen/page-list that is
    /// active at creation time.
    ///
    /// If the point is outside the requested coordinate space, this returns
    /// `Err(Error::InvalidValue)`.
    ///
    /// If the tracked grid reference outlives this terminal, the handle remains
    /// valid, but it will always return `false` or `Ok(None)`.
    pub fn track_grid_ref(&self, point: Point) -> Result<TrackedGridRef> {
        let mut raw: ffi::TrackedGridRef = std::ptr::null_mut();
        let result = unsafe {
            ffi::ghostty_terminal_grid_ref_track(self.inner.as_raw(), point.into(), &raw mut raw)
        };
        from_result(result)?;

        let inner = NonNull::new(raw).ok_or(Error::InvalidValue)?;
        Ok(TrackedGridRef::new(inner, self.inner.ptr))
    }

    /// Convert a grid reference back to a point in the given coordinate system.
    ///
    /// This is the inverse of [`Terminal::grid_ref`]: given a grid reference, it
    /// returns the x/y coordinates in the requested coordinate system (active,
    /// viewport, screen, or history).
    ///
    /// The grid reference must have been obtained from the same terminal instance.
    /// Like all grid references, it is only valid until the next mutating
    /// terminal call.
    ///
    /// Not every grid reference is representable in every coordinate system.
    /// For example, a cell in scrollback history cannot be expressed in active
    /// coordinates, and a cell that has scrolled off the visible area cannot
    /// be expressed in viewport coordinates. In these cases, the function
    /// returns `Ok(None)`.
    pub fn point_from_grid_ref(
        &self,
        grid_ref: &GridRef<'_>,
        space: PointSpace,
    ) -> Result<Option<PointCoordinate>> {
        let mut point = MaybeUninit::<ffi::PointCoordinate>::zeroed();
        let result = unsafe {
            ffi::ghostty_terminal_point_from_grid_ref(
                self.inner.as_raw(),
                std::ptr::from_ref(&grid_ref.inner),
                space.into_raw(),
                point.as_mut_ptr(),
            )
        };

        from_optional_result_uninit(result, point).map(|value| value.map(Into::into))
    }

    /// Get the current value of a terminal mode.
    pub fn mode(&self, mode: Mode) -> Result<bool> {
        let mut mode = ffi::TerminalModeConfig {
            mode: mode.into(),
            value: false,
        };

        let result = unsafe {
            ffi::ghostty_terminal_get(
                self.inner.as_raw(),
                Data::MODE,
                (&raw mut mode).cast::<std::ffi::c_void>(),
            )
        };
        from_result(result)?;
        Ok(mode.value)
    }

    /// Set the current value of a terminal mode.
    ///
    /// This does not change the value restored by a full terminal reset (RIS).
    pub fn set_mode(&mut self, mode: Mode, value: bool) -> Result<&mut Self> {
        let mode = ffi::TerminalModeConfig {
            mode: mode.into(),
            value,
        };

        let result = unsafe {
            ffi::ghostty_terminal_set(
                self.inner.as_raw(),
                Opt::MODE,
                (&raw const mode).cast::<std::ffi::c_void>(),
            )
        };
        from_result(result)?;
        Ok(self)
    }

    /// Set the reset default for a terminal mode.
    ///
    /// This unconditionally updates both the current value and the value
    /// restored by a full terminal reset (RIS).
    ///
    /// Some recognized modes represent transitions or mirror additional
    /// terminal state and cannot safely be configured as reset defaults.
    /// Those modes return [`Error::InvalidValue`].
    pub fn set_default_mode(&mut self, mode: Mode, value: bool) -> Result<&mut Self> {
        let mode = ffi::TerminalModeConfig {
            mode: mode.into(),
            value,
        };

        let result = unsafe {
            ffi::ghostty_terminal_set(
                self.inner.as_raw(),
                Opt::MODE_DEFAULT,
                (&raw const mode).cast::<std::ffi::c_void>(),
            )
        };
        from_result(result)?;
        Ok(self)
    }

    /// Compress eligible terminal scrollback.
    ///
    /// Incremental mode performs bounded work suitable for an idle callback.
    /// A pending result means the application should invoke another step while
    /// the terminal remains idle. A complete result means no continuation is
    /// needed until `Terminal::compression_activity` changes. Full mode
    /// performs one synchronous scan and can stall on large scrollback buffers.
    ///
    /// Compression is opportunistic. Complete means the pass has finished,
    /// not that every page was compressed: pages may be unprofitable or
    /// encounter an allocation or reclamation failure. Compression changes
    /// only the terminal's storage representation and never its logical
    /// contents or scrollback limit. Accessing compressed history restores
    /// it transparently.
    ///
    /// This function is not thread-safe with other operations on the same
    /// terminal. The caller must serialize it with writes, rendering, searches,
    /// and other terminal access.
    pub fn compress(&mut self, mode: CompressionMode) -> Result<CompressionResult> {
        let mut value = ffi::TerminalCompressionResult::UNSUPPORTED;
        let result = unsafe {
            ffi::ghostty_terminal_compress(self.inner.as_raw(), mode.into(), &raw mut value)
        };
        from_result(result)?;
        value.try_into().map_err(|_| Error::InvalidValue)
    }

    /// Return the current compression activity token.
    ///
    /// The token is opaque and only equality comparisons are meaningful.
    /// An embedding application should cache it and restart its compression
    /// idle delay whenever the value changes. The value may wrap and changes
    /// in either direction have the same meaning.
    ///
    /// This function only observes terminal state.
    /// It does not perform or schedule compression.
    pub fn compression_activity(&self) -> Result<CompressionActivity> {
        let mut value = 0;
        let result = unsafe {
            ffi::ghostty_terminal_compression_activity(self.inner.as_raw(), &raw mut value)
        };
        from_result(result)?;
        Ok(CompressionActivity(value))
    }

    /// The configured maximum retained VT continuation size in bytes.
    ///
    /// A value of zero means continuation tracking is disabled. This reports
    /// the configured limit even when a current unfinished continuation is
    /// temporarily unavailable.
    pub fn continuation_max_bytes(&self) -> Result<usize> {
        self.get(Data::CONTINUATION_MAX_BYTES)
    }

    /// Set the maximum number of replay-safe VT continuation bytes retained.
    ///
    /// Continuation bytes reconstruct an escape sequence or UTF-8 codepoint
    /// which was unfinished at the end of the most recent [`Terminal::vt_write`]
    /// call. They are used automatically by terminal snapshots and may also be
    /// exported directly with the continuation APIs.
    ///
    /// Tracking is disabled by default. A nonzero value enables tracking and
    /// sets its byte limit. Passing zero disables tracking. Lowering the limit
    /// below an already-retained continuation, or enabling tracking while the
    /// parser is already unfinished, makes the current continuation unavailable
    /// because earlier bytes cannot be reconstructed. Tracking recovers
    /// automatically after a later write reaches the ground state or contains
    /// a fresh replay start.
    pub fn set_continuation_max_bytes(&mut self, v: usize) -> Result<&mut Self> {
        self.set(Opt::CONTINUATION_MAX_BYTES, &v)?;
        Ok(self)
    }

    /// Write the terminal's replay-safe VT continuation to a callback writer.
    ///
    /// The continuation is the exact byte suffix needed to reconstruct
    /// unfinished VT parser or UTF-8 decoder state in an equivalent terminal.
    /// It is empty when the stream is at ground. The callback is invoked
    /// synchronously and may be called more than once. It must not call
    /// terminal APIs with the same terminal handle.
    ///
    /// Continuation tracking must have been enabled by calling
    /// [`Terminal::set_continuation_max_bytes`] with a nonzero value before
    /// the input that produced the continuation was written.    
    ///
    /// # Errors
    ///
    /// This function returns [`Error::IoError`] if the callback rejects a
    /// write, [`Error::LimitExceeded`] if output accounting overflows, or
    /// [`Error::InvalidValue`] if an argument is invalid, tracking is disabled,
    /// or the current continuation is unavailable.
    pub fn continuation_write<W: Write>(&mut self, writer: &mut W) -> Result<()> {
        let writer = crate::io::to_writer(writer);
        let result =
            unsafe { ffi::ghostty_terminal_continuation_write(self.inner.as_raw(), writer) };
        from_result(result)
    }

    /// Return an allocated copy of the terminal's replay-safe VT continuation.
    ///
    /// The returned bytes are allocated with allocator, or the default allocator
    /// when allocator is `None`. An empty continuation is a successful result
    /// with empty [`Bytes`]; libghostty does not allocate for it.
    /// Continuation tracking must have been enabled by calling
    /// [`Terminal::set_continuation_max_bytes`] to a nonzero value before the
    /// input that produced the continuation was written.
    ///
    /// The caller must serialize this operation with all other access to the same
    /// terminal.
    ///
    /// # Errors
    ///
    /// This function returns [`Error::OutOfMemory`] on allocation failure, or
    /// [`Error::InvalidValue`] if an argument is invalid, tracking is disabled,
    /// or the current continuation is unavailable.
    pub fn continuation_alloc<'a, 'ctx: 'a>(
        &self,
        alloc: Option<&'a Allocator<'ctx>>,
    ) -> Result<Option<Bytes<'a>>> {
        let mut out = std::ptr::null_mut();
        let mut out_len = 0usize;
        let alloc = alloc.map_or(std::ptr::null(), super::alloc::Allocator::to_raw);

        let result = unsafe {
            ffi::ghostty_terminal_continuation_alloc(
                self.inner.as_raw(),
                alloc,
                &raw mut out,
                &raw mut out_len,
            )
        };

        let out = from_optional_result(result, out)?;
        // SAFETY: On success, libghostty hands over `out_len` bytes allocated
        // with `alloc`, or NULL for empty output.
        Ok(out.map(|ptr| unsafe { Bytes::from_raw_parts(ptr, out_len, alloc) }))
    }

    /// Copy the terminal's replay-safe VT continuation into a caller buffer.
    ///
    /// Pass an empty `buf` to query the required size. A size query returns
    /// [`Error::OutOfSpace`] with the required size, including zero when the
    /// stream is at ground. If a non-empty buffer is too small, the function
    /// has the same result and reports the full required size.
    ///
    /// Continuation tracking must have been enabled by calling
    /// [`Terminal::set_continuation_max_bytes`] to a nonzero value before the
    /// input that produced the continuation was written.
    ///
    /// The caller must serialize this operation with all other access to the same
    /// terminal.
    ///
    /// # Errors
    ///
    /// This function returns [`Error::OutOfSpace`] for a size query or
    /// insufficient buffer, or [`Error::InvalidValue`] if an argument is invalid,
    /// tracking is disabled, or the current continuation is unavailable.
    pub fn continuation_buf(&self, buf: &mut [u8]) -> Result<Option<usize>> {
        let mut written = 0usize;
        // The C API uses a NULL pointer to distinguish an explicit size query
        // from a zero-capacity destination. Rust empty slices have a non-NULL
        // dangling pointer, so translate that representation at this boundary.
        let buf_ptr = if buf.is_empty() {
            std::ptr::null_mut()
        } else {
            buf.as_mut_ptr()
        };

        let result = unsafe {
            ffi::ghostty_terminal_continuation_buf(
                self.inner.as_raw(),
                buf_ptr,
                buf.len(),
                &raw mut written,
            )
        };

        from_optional_result_with_len(result, written)
    }

    pub(crate) fn get<T>(&self, tag: ffi::TerminalData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_terminal_get(self.inner.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_result(result)?;
        // SAFETY: Value should be initialized after successful call.
        Ok(unsafe { value.assume_init() })
    }
    pub(crate) fn get_optional<T>(&self, tag: ffi::TerminalData::Type) -> Result<Option<T>> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_terminal_get(self.inner.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_optional_result_uninit(result, value)
    }
    pub(crate) fn set<T>(&self, tag: ffi::TerminalOption::Type, v: &T) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_terminal_set(self.inner.as_raw(), tag, std::ptr::from_ref(v).cast())
        };
        from_result(result)
    }
    /// Set an option whose ABI expects the pointer value itself, not a pointer
    /// to Rust storage containing that value.
    pub(crate) fn set_ptr(
        &self,
        tag: ffi::TerminalOption::Type,
        ptr: *const std::ffi::c_void,
    ) -> Result<()> {
        let result = unsafe { ffi::ghostty_terminal_set(self.inner.as_raw(), tag, ptr) };
        from_result(result)
    }
    pub(crate) fn set_optional<T>(
        &self,
        tag: ffi::TerminalOption::Type,
        v: Option<&T>,
    ) -> Result<()> {
        let ptr = if let Some(v) = v {
            std::ptr::from_ref(v)
        } else {
            std::ptr::null()
        };

        let result = unsafe { ffi::ghostty_terminal_set(self.inner.as_raw(), tag, ptr.cast()) };
        from_result(result)
    }

    /// Get the terminal width in cells.
    pub fn cols(&self) -> Result<u16> {
        self.get(Data::COLS)
    }
    /// Get the terminal height in cells.
    pub fn rows(&self) -> Result<u16> {
        self.get(Data::ROWS)
    }
    /// Get the total width of the terminal in pixels.
    ///
    /// This is `cols * cell_width_px` as set by [`Terminal::resize`].
    pub fn width_px(&self) -> Result<u32> {
        self.get(Data::WIDTH_PX)
    }
    /// Get the total height of the terminal in pixels.
    ///
    /// This is `rows * cell_height_px` as set by [`Terminal::resize`].
    pub fn height_px(&self) -> Result<u32> {
        self.get(Data::HEIGHT_PX)
    }

    /// The configured maximum scrollback allocation in bytes.
    ///
    /// This always reports the primary screen's configured value, including
    /// while an alternate screen is active.
    ///
    /// Returns `None` when the configured byte limit is unlimited.
    pub fn scrollback_max_bytes(&self) -> Result<Option<usize>> {
        self.get_optional(Data::SCROLLBACK_MAX_BYTES)
    }

    /// Set the maximum scrollback allocation in bytes.
    ///
    /// This is an estimate. Internally, libghostty only prunes bytes up
    /// to a "page"-granularity. A page is the minimum allocated unit of
    /// grid space within Ghostty. A page at the time of writing these docs
    /// is about 400KB, so the byte limit will be within this delta.
    ///
    /// This works alongside the line limit configuration. If both are set,
    /// the first-reached limit is used first. Both limits are dependent
    /// on external state (byte limit can be reached with less lines if
    /// more styles are used for example, line limit can be reached with
    /// a narrower terminal viewport). So, they are useful together.
    ///
    /// Lowering the limit immediately removes eligible complete historical
    /// pages. A value of zero disables scrollback and erases retained history.
    /// A `None` value removes the byte limit.
    pub fn set_scrollback_max_bytes(&mut self, v: Option<usize>) -> Result<&mut Self> {
        self.set_optional(Opt::SCROLLBACK_MAX_BYTES, v.as_ref())?;
        Ok(self)
    }

    /// The configured maximum number of physical scrollback lines.
    ///
    /// This always reports the primary screen's configured value, including
    /// while an alternate screen is active.
    ///
    /// Returns `None` when the configured line limit is unlimited.
    pub fn scrollback_max_lines(&self) -> Result<Option<usize>> {
        self.get_optional(Data::SCROLLBACK_MAX_LINES)
    }

    /// Set the maximum number of physical lines retained in scrollback.
    ///
    /// This is an estimate. Internally, libghostty only prunes lines up
    /// to a "page"-granularity. A page is the minimum allocated unit of
    /// grid space within Ghostty. As a result, the actual available scrollback
    /// lines will almost always be higher than configured. The magnitude
    /// of the difference depends on the number of used styles, graphemes, etc.
    /// since the row-count in a page is dynamic based on that. In general,
    /// it ranges from dozens to a hundred or so lines.
    ///
    /// This works alongside the byte limit configuration. If both are set,
    /// the first-reached limit is used first. Both limits are dependent
    /// on external state (byte limit can be reached with less lines if
    /// more styles are used for example, line limit can be reached with
    /// a narrower terminal viewport). So, they are useful together.
    ///
    /// Lowering the limit immediately removes eligible complete historical
    /// pages. A `None` value pointer removes the line limit.
    pub fn set_scrollback_max_lines(&mut self, v: Option<usize>) -> Result<&mut Self> {
        self.set_optional(Opt::SCROLLBACK_MAX_LINES, v.as_ref())?;
        Ok(self)
    }

    /// Get the cursor column position (0-indexed).
    pub fn cursor_x(&self) -> Result<u16> {
        self.get(Data::CURSOR_X)
    }
    /// Get the cursor row position within the active area (0-indexed).
    pub fn cursor_y(&self) -> Result<u16> {
        self.get(Data::CURSOR_Y)
    }
    /// Whether the cursor is currently at a semantic shell prompt or input
    /// area.
    ///
    /// This depends on semantic prompt markers such as OSC 133. Returns false
    /// when semantic prompt information is unavailable or the alternate
    /// screen is active.
    pub fn is_cursor_at_prompt(&self) -> Result<bool> {
        self.get(Data::CURSOR_AT_PROMPT)
    }
    /// Get whether the cursor has a pending wrap (next print will soft-wrap).
    pub fn is_cursor_pending_wrap(&self) -> Result<bool> {
        self.get(Data::CURSOR_PENDING_WRAP)
    }
    /// Get whether the cursor is visible (DEC mode 25).
    pub fn is_cursor_visible(&self) -> Result<bool> {
        self.get(Data::CURSOR_VISIBLE)
    }
    /// Get the current SGR style of the cursor.
    ///
    /// This is the style that will be applied to newly printed characters.
    pub fn cursor_style(&self) -> Result<style::Style> {
        self.get::<ffi::Style>(Data::CURSOR_STYLE)
            .and_then(std::convert::TryInto::try_into)
    }
    /// The mouse pointer shape requested by the application through OSC 22.
    ///
    /// Initially [`mouse::Shape::Text`]. An empty OSC 22 resets it to that,
    /// and a name libghostty doesn't know leaves it unchanged. Excludes host
    /// hover overrides.
    pub fn mouse_shape(&self) -> Result<mouse::Shape> {
        self.get::<ffi::MouseShape::Type>(Data::MOUSE_SHAPE)?
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }
    /// Get the current Kitty keyboard protocol flags.
    pub fn kitty_keyboard_flags(&self) -> Result<key::KittyKeyFlags> {
        self.get::<ffi::KittyKeyFlags>(Data::KITTY_KEYBOARD_FLAGS)
            .map(key::KittyKeyFlags::from_bits_retain)
    }

    /// Get the scrollbar state for the terminal viewport.
    ///
    /// This is amortized `O(1)`: the total is maintained incrementally as
    /// the terminal is modified and the viewport offset is cached. The
    /// first read after the viewport moves to an arbitrary position that
    /// isn't an absolute row (e.g. scrolling to a selection) may cost
    /// `O(pages)` to compute the offset, after which it is cached again.
    ///
    /// There is intentionally no change notification for scroll state.
    /// Callers building scrollbars should poll this once per frame or
    /// per write batch and diff the result to detect changes; this is
    /// what Ghostty's own renderer does.
    pub fn scrollbar(&self) -> Result<Scrollbar> {
        self.get(Data::SCROLLBAR)
    }
    /// Get the currently active screen.
    pub fn active_screen(&self) -> Result<Screen> {
        self.get::<ffi::TerminalScreen::Type>(Data::ACTIVE_SCREEN)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    /// Whether the viewport is currently pinned to the active area.
    ///
    /// This is true when the viewport is following the active terminal area,
    /// and false when the user has scrolled into history.
    pub fn viewport_active(&self) -> Result<bool> {
        self.get(Data::VIEWPORT_ACTIVE)
    }
    /// Get whether any mouse tracking mode is active.
    ///
    /// Returns true if any of the mouse tracking modes (X10, normal, button,
    /// or any-event) are enabled.
    pub fn is_mouse_tracking(&self) -> Result<bool> {
        self.get(Data::MOUSE_TRACKING)
    }
    /// Whether VT processing encountered a non-gracefully handled error that
    /// may have prevented a terminal-owned semantic update.
    ///
    /// Processing remains best-effort, and [`Terminal::reset`] does not clear
    /// this flag; it is purely informational. Gracefully handled protocol
    /// failures, configured limits, malformed or unsupported input, and
    /// failures limited to external effects or query responses do not set it.
    pub fn vt_processing_error(&self) -> Result<bool> {
        self.get(Data::VT_PROCESSING_ERROR)
    }
    /// The terminal title as set by escape sequences (e.g. OSC 0/2).
    ///
    /// Returns a borrowed string. It is valid until the next mutating terminal
    /// call (e.g. [`Terminal::vt_write`]). An empty string is returned when no
    /// title has been set.
    ///
    /// The bytes come from the program and are not guaranteed to be UTF-8.
    pub fn title(&self) -> Result<&[u8]> {
        let str = self.get::<ffi::String>(Data::TITLE)?;
        // SAFETY: libghostty documents the returned pointer as valid until the
        // next mutating terminal call. Not every mutating call takes
        // `&mut self` (`set_selection` doesn't), but libghostty only rewrites
        // the title and pwd storage when processing input, resetting, or
        // through the TITLE and PWD options. These bindings expose the first
        // two only through `&mut self` methods (`vt_write`,
        // `vt_write_until_ground`, `reset`), and the options only through
        // `set_title` and `set_pwd`, which take `&mut self` too. No UTF-8 or
        // NUL-termination is assumed; we only read `len` bytes.
        Ok(unsafe { str.to_bytes() })
    }

    /// The terminal's current working directory as set by escape sequences
    /// (e.g. OSC 7).
    ///
    /// Returns a borrowed string. It is valid until the next mutating terminal
    /// call (e.g. [`Terminal::vt_write`]). An empty string is returned when no
    /// pwd has been set.
    ///
    /// The terminal stores whatever bytes the shell emitted, without parsing.
    /// That means for OSC 7 the value is the raw URI (typically `file://...`);
    /// for OSC 9/OSC 1337 it is typically a bare path. The bytes are not
    /// guaranteed to be UTF-8.
    pub fn pwd(&self) -> Result<&[u8]> {
        let str = self.get::<ffi::String>(Data::PWD)?;
        // SAFETY: Ditto (see `title`): only `&mut self` methods can rewrite
        // the pwd storage, and only `len` bytes are read.
        Ok(unsafe { str.to_bytes() })
    }

    /// Set the terminal title manually.
    ///
    /// The string data is copied into the terminal. An empty title clears
    /// it. Read it back with [`Terminal::title`].
    ///
    /// On `Err(Error::OutOfMemory)`, the previous title is left unchanged.
    ///
    /// ```
    /// use libghostty_vt::Terminal;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut terminal = Terminal::new(80, 24)?;
    ///
    /// // Most titles are plain strings, which pass straight in.
    /// terminal.set_title("vim — main.rs")?;
    /// assert_eq!(terminal.title()?, "vim — main.rs".as_bytes());
    ///
    /// // Titles are bytes, not UTF-8 strings: this one is Latin-1.
    /// terminal.set_title(b"caf\xe9")?;
    /// assert_eq!(terminal.title()?, b"caf\xe9");
    ///
    /// // So the bytes `title` returns can be set again unchanged.
    /// let mut other = Terminal::new(80, 24)?;
    /// other.set_title(terminal.title()?)?;
    /// assert_eq!(other.title()?, b"caf\xe9");
    ///
    /// // An empty title clears it.
    /// terminal.set_title("")?;
    /// assert!(terminal.title()?.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_title(&mut self, title: impl AsRef<[u8]>) -> Result<&mut Self> {
        // Take `AsRef<[u8]>` rather than `&str`: strings are the common
        // case, but upstream accepts arbitrary bytes and `title` returns
        // bytes, so any value read back must be settable again as-is.
        let title = title.as_ref();
        // The `From` impl on `ffi::String` only takes `str`, but upstream
        // accepts arbitrary bytes, so build it from the slice by hand.
        // Upstream copies the bytes before returning, so the pointer only
        // has to live for this call. An empty slice may carry a dangling
        // (non-NULL) pointer; that's fine, since upstream reads `len` bytes
        // and clears the title for a zero length.
        let raw = ffi::String {
            ptr: title.as_ptr(),
            len: title.len(),
        };
        self.set(Opt::TITLE, &raw)?;
        Ok(self)
    }

    /// Set the terminal working directory manually.
    ///
    /// The string data is copied into the terminal. An empty pwd clears it.
    /// Read it back with [`Terminal::pwd`].
    ///
    /// On `Err(Error::OutOfMemory)`, the previous pwd is left unchanged.
    ///
    /// ```
    /// use libghostty_vt::Terminal;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut terminal = Terminal::new(80, 24)?;
    ///
    /// // Most paths are plain strings, which pass straight in.
    /// terminal.set_pwd("/home/user/src")?;
    /// assert_eq!(terminal.pwd()?, "/home/user/src".as_bytes());
    ///
    /// // The pwd is stored as-is: neither parsed as a URI nor required to be
    /// // UTF-8.
    /// terminal.set_pwd(b"/home/caf\xe9")?;
    /// assert_eq!(terminal.pwd()?, b"/home/caf\xe9");
    ///
    /// // An empty pwd clears it.
    /// terminal.set_pwd("")?;
    /// assert!(terminal.pwd()?.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_pwd(&mut self, pwd: impl AsRef<[u8]>) -> Result<&mut Self> {
        // Ditto (see `set_title`): `AsRef<[u8]>` so values from `pwd`
        // round-trip, and upstream copies the bytes, so the slice only has
        // to outlive this call.
        let pwd = pwd.as_ref();
        let raw = ffi::String {
            ptr: pwd.as_ptr(),
            len: pwd.len(),
        };
        self.set(Opt::PWD, &raw)?;
        Ok(self)
    }
    /// The total number of rows in the active screen including scrollback.
    pub fn total_rows(&self) -> Result<usize> {
        self.get(Data::TOTAL_ROWS)
    }
    ///  The number of scrollback rows (total rows minus viewport rows).
    pub fn scrollback_rows(&self) -> Result<usize> {
        self.get(Data::SCROLLBACK_ROWS)
    }

    /// The effective foreground color (override or default).
    pub fn fg_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_FOREGROUND)
            .map(|v| v.map(Into::into))
    }
    /// The default foreground color (ignoring any OSC override).
    pub fn default_fg_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_FOREGROUND_DEFAULT)
            .map(|v| v.map(Into::into))
    }
    /// Set the default foreground color.
    pub fn set_default_fg_color(&mut self, v: Option<RgbColor>) -> Result<&mut Self> {
        self.set_optional(Opt::COLOR_FOREGROUND, v.map(ffi::ColorRgb::from).as_ref())?;
        Ok(self)
    }

    /// The effective background color (override or default).
    pub fn bg_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_BACKGROUND)
            .map(|v| v.map(Into::into))
    }
    /// The default background color (ignoring any OSC override).
    pub fn default_bg_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_BACKGROUND_DEFAULT)
            .map(|v| v.map(Into::into))
    }
    /// Set the default background color.
    pub fn set_default_bg_color(&mut self, v: Option<RgbColor>) -> Result<&mut Self> {
        self.set_optional(Opt::COLOR_BACKGROUND, v.map(ffi::ColorRgb::from).as_ref())?;
        Ok(self)
    }

    /// The effective cursor color (override or default).
    pub fn cursor_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_CURSOR)
            .map(|v| v.map(Into::into))
    }
    /// The default cursor color (ignoring any OSC override).
    pub fn default_cursor_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_CURSOR_DEFAULT)
            .map(|v| v.map(Into::into))
    }
    /// Set the default cursor color.
    pub fn set_default_cursor_color(&mut self, v: Option<RgbColor>) -> Result<&mut Self> {
        self.set_optional(Opt::COLOR_CURSOR, v.map(ffi::ColorRgb::from).as_ref())?;
        Ok(self)
    }

    /// Set the default cursor style used by DECSCUSR reset (CSI 0 q).
    ///
    /// Passing `None` resets to libghostty's built-in block cursor default.
    pub fn set_default_cursor_style(&mut self, v: Option<CursorStyle>) -> Result<&mut Self> {
        self.set_optional(Opt::DEFAULT_CURSOR_STYLE, v.as_ref())?;
        Ok(self)
    }

    /// Set whether the default cursor blinks when reset by DECSCUSR (CSI 0 q).
    ///
    /// Passing `None` resets to libghostty's built-in non-blinking default.
    pub fn set_default_cursor_blink(&mut self, v: Option<bool>) -> Result<&mut Self> {
        self.set_optional(Opt::DEFAULT_CURSOR_BLINK, v.as_ref())?;
        Ok(self)
    }

    /// Set whether a resize may pull rows out of scrollback back into the
    /// active area.
    ///
    /// When true, growing rows reveals scrollback if the cursor is on the
    /// bottom row, and a column reflow that needs fewer rows reveals
    /// scrollback as well. When false, growing rows always appends blank rows
    /// at the bottom and a column reflow keeps the top of the active area on
    /// the same content, so a line that is fully in scrollback stays there. A
    /// soft-wrapped line with at least one row still in the active area may
    /// still unwrap back into view.
    ///
    /// Set this to false when the pty keeps its own screen buffer without
    /// scrollback, since it cannot pull rows back and will otherwise disagree
    /// with the terminal about the screen contents after a resize. Windows
    /// `ConPTY` is the motivating case.
    ///
    /// This is preserved across a full reset (RIS).
    ///
    /// Passing `None` resets to the built-in default of `true`.
    pub fn set_resize_pull_scrollback(&mut self, v: Option<bool>) -> Result<&mut Self> {
        self.set_optional(Opt::RESIZE_PULL_SCROLLBACK, v.as_ref())?;
        Ok(self)
    }

    /// The current 256-color palette.
    pub fn color_palette(&self) -> Result<Palette> {
        self.get::<RawPalette>(Data::COLOR_PALETTE)
            .map(Palette::from)
    }
    /// The default 256-color palette (ignoring any OSC overrides).
    pub fn default_color_palette(&self) -> Result<Palette> {
        self.get::<RawPalette>(Data::COLOR_PALETTE_DEFAULT)
            .map(Palette::from)
    }
    /// Set the default 256-color palette.
    pub fn set_default_color_palette(&mut self, v: Option<Palette>) -> Result<&mut Self> {
        self.set_optional::<RawPalette>(
            Opt::COLOR_PALETTE,
            v.map(std::convert::Into::into).as_ref(),
        )?;
        Ok(self)
    }

    /// Set the maximum bytes the APC handler will buffer for all protocols.
    ///
    /// This prevents malicious input from causing unbounded memory allocation.
    /// A `None` value removes all overrides, reverting to the built-in defaults.
    /// APC sequences no protocol implements are bounded separately, by
    /// [`set_unknown_max_bytes`](Self::set_unknown_max_bytes).
    pub fn set_apc_max_bytes(&mut self, max: Option<usize>) -> Result<&mut Self> {
        self.set_optional(Opt::APC_MAX_BYTES, max.as_ref())?;
        Ok(self)
    }

    /// Set the most bytes of each unsupported sequence to keep and pass to the
    /// [unknown sequence callback](Self::on_unknown_sequence). The same limit
    /// applies to APC and OSC sequences.
    ///
    /// Zero, the default, turns unsupported sequence reporting off.
    ///
    /// A sequence longer than the limit is still reported. Its content holds
    /// the first bytes up to the limit, and `truncated` is true.
    ///
    /// Choose a limit that fits the largest sequence you expect. Unknown OSC
    /// sequences up to 2048 bytes are kept in a buffer the terminal already
    /// owns, so limits up to 2048 add no memory allocations for OSC. Larger
    /// limits allocate memory for each unknown OSC sequence. Unknown APC
    /// sequences are always kept in allocated memory.
    pub fn set_unknown_max_bytes(&mut self, max: usize) -> Result<&mut Self> {
        self.set(Opt::UNKNOWN_MAX_BYTES, &max)?;
        Ok(self)
    }

    /// Set the name of the terminfo entry this terminal runs as, reported in
    /// response to an XTGETTCAP query for `TN` (e.g. `xterm-256color`).
    ///
    /// The name is copied into the terminal. An empty name clears it. A name
    /// longer than 128 bytes returns `Err(Error::InvalidValue)` and leaves the
    /// previous name unchanged.
    ///
    /// If this is unset then nothing is reported for an XTGETTCAP `TN` query,
    /// because libghostty doesn't know what the embedding terminal advertises
    /// itself as.
    pub fn set_terminfo_name(&mut self, name: &str) -> Result<&mut Self> {
        self.set(Opt::TERMINFO_NAME, &ffi::String::from(name))?;
        Ok(self)
    }

    /// The configured maximum decoded bytes per Kitty clipboard protocol
    /// (OSC 5522) write transaction.
    ///
    /// See [`Self::set_clipboard_write_max_bytes`].
    pub fn clipboard_write_max_bytes(&self) -> Result<usize> {
        self.get(Data::CLIPBOARD_WRITE_MAX_BYTES)
    }

    /// Set the maximum total decoded bytes a single Kitty clipboard protocol
    /// (OSC 5522) write transaction may accumulate. The limit is captured
    /// when a transaction begins; an in-flight transaction keeps the limit it
    /// started with.
    ///
    /// Data beyond the limit fails the whole transaction with EFBIG. The
    /// transaction is discarded, later write-related packets are ignored
    /// until a new write begins, and nothing reaches the
    /// [clipboard write callback](Self::on_clipboard_write).
    ///
    /// Transactions are buffered in memory, so this limit bounds how much
    /// memory a single write can make the terminal allocate. Pass
    /// `Some(usize::MAX)` to remove the limit. `None` reverts to the built-in
    /// default of 64 MiB, the minimum required by the protocol.
    ///
    /// This limit doesn't apply to OSC 52 writes, which are bounded by the
    /// maximum length of an escape sequence instead.
    pub fn set_clipboard_write_max_bytes(&mut self, limit: Option<usize>) -> Result<&mut Self> {
        self.set_optional(Opt::CLIPBOARD_WRITE_MAX_BYTES, limit.as_ref())?;
        Ok(self)
    }

    /// Enable or disable Glyph Protocol APC handling.
    ///
    /// Disabling the protocol makes the terminal ignore Glyph Protocol APC
    /// sequences and clears the session's glyph glossary.
    pub fn set_glyph_protocol_enabled(&mut self, enabled: bool) -> Result<&mut Self> {
        self.set(Opt::GLYPH_PROTOCOL, &enabled)?;
        Ok(self)
    }

    /// Enable window title reports in response to `CSI 21 t`.
    ///
    /// This is disabled by default because a running program can set a title
    /// and query it back into the pty input stream, potentially injecting
    /// commands that execute after user interaction.
    ///
    /// Passing `false` disables title reporting.
    pub fn set_title_report_enabled(&mut self, enabled: bool) -> Result<&mut Self> {
        self.set(Opt::TITLE_REPORT, &enabled)?;
        Ok(self)
    }

    /// Enable checksum reports in response to DECRQCRA
    /// (`CSI Pi ; Pg ; Pt ; Pl ; Pb ; Pr * y`).
    ///
    /// This is disabled by default because a running program can checksum the
    /// screen one cell at a time and so read back everything on it, including
    /// output from other programs. Passing `false` disables checksum
    /// reporting.
    ///
    /// While this is disabled, XTCHECKSUM (`CSI Ps # y`), which changes how
    /// the checksum is calculated, is ignored as well. See
    /// [`Self::set_xt_checksum_extension`].
    pub fn set_xt_checksum_report_enabled(&mut self, enabled: bool) -> Result<&mut Self> {
        self.set(Opt::XT_CHECKSUM_REPORT, &enabled)?;
        Ok(self)
    }

    /// Set how the DECRQCRA checksum is calculated after a reset (RIS or
    /// DECSTR). This also changes the current calculation.
    ///
    /// The value holds the same bits as XTCHECKSUM (`CSI Ps # y`) and xterm's
    /// `checksumExtension` resource, which a running program can still use to
    /// change the calculation until the next reset. See
    /// [`XtChecksumExtension`] for the bits.
    ///
    /// [`XtChecksumExtension::empty()`] is the calculation of a real DEC
    /// terminal. Values above 31, i.e. any bits outside
    /// [`XtChecksumExtension::all()`] kept by
    /// [`XtChecksumExtension::from_bits_retain`], return
    /// `Err(Error::InvalidValue)` and leave both the current and the reset
    /// calculation unchanged.
    ///
    /// Checksums are only reported once enabled with
    /// [`Self::set_xt_checksum_report_enabled`].
    pub fn set_xt_checksum_extension(&mut self, ext: XtChecksumExtension) -> Result<&mut Self> {
        // Upstream treats NULL and 0 the same, so there's no need for an
        // `Option` here: the empty set already means "the DEC default".
        self.set(Opt::XT_CHECKSUM_EXTENSION, &ext.bits())?;
        Ok(self)
    }
}
impl Drop for Terminal<'_, '_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_terminal_free(self.inner.as_raw()) }
        // SAFETY: This terminal owns the allocation from Box::into_raw.
        // The native handle can no longer use its userdata. Borrowed callback
        // views are ManuallyDrop and never reach this destructor.
        unsafe { drop(Box::from_raw(self.vtable)) }
    }
}

/// A point in the terminal grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    /// Active area where the cursor can move.
    Active(PointCoordinate),
    /// Visible viewport (changes when scrolled).
    Viewport(PointCoordinate),
    /// Full screen including scrollback.
    Screen(PointCoordinate),
    /// Scrollback history only (before active area).
    History(PointCoordinate),
}

impl From<Point> for ffi::Point {
    fn from(value: Point) -> Self {
        match value {
            Point::Active(coord) => Self {
                tag: ffi::PointTag::ACTIVE,
                value: ffi::PointValue {
                    coordinate: coord.into(),
                },
            },
            Point::Viewport(coord) => Self {
                tag: ffi::PointTag::VIEWPORT,
                value: ffi::PointValue {
                    coordinate: coord.into(),
                },
            },
            Point::Screen(coord) => Self {
                tag: ffi::PointTag::SCREEN,
                value: ffi::PointValue {
                    coordinate: coord.into(),
                },
            },
            Point::History(coord) => Self {
                tag: ffi::PointTag::HISTORY,
                value: ffi::PointValue {
                    coordinate: coord.into(),
                },
            },
        }
    }
}

/// A coordinate space for converting grid references back to points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointSpace {
    /// Active area where the cursor can move.
    Active,
    /// Visible viewport, which changes when scrolled.
    Viewport,
    /// Full screen including scrollback.
    Screen,
    /// Scrollback history only, before the active area.
    History,
}

impl PointSpace {
    pub(crate) fn into_raw(self) -> ffi::PointTag::Type {
        match self {
            Self::Active => ffi::PointTag::ACTIVE,
            Self::Viewport => ffi::PointTag::VIEWPORT,
            Self::Screen => ffi::PointTag::SCREEN,
            Self::History => ffi::PointTag::HISTORY,
        }
    }
}

/// A coordinate in the terminal grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointCoordinate {
    /// Column (0-indexed).
    pub x: u16,
    /// Row (0-indexed). May exceed page size for screen/history tags.
    pub y: u32,
}
impl From<PointCoordinate> for ffi::PointCoordinate {
    fn from(value: PointCoordinate) -> Self {
        let PointCoordinate { x, y } = value;
        Self { x, y }
    }
}
impl From<ffi::PointCoordinate> for PointCoordinate {
    fn from(value: ffi::PointCoordinate) -> Self {
        let ffi::PointCoordinate { x, y } = value;
        Self { x, y }
    }
}

/// Scroll viewport behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollViewport {
    /// Scroll to the top of the scrollback.
    Top,
    /// Scroll to the bottom (active area).
    Bottom,
    /// Scroll by a delta amount (up is negative).
    Delta(isize),
    /// Scroll to an absolute row offset from the top of the scrollback.
    Row(usize),
    /// Scroll by a number of prompts relative to the viewport top.
    ///
    /// Negative values move backward and positive values move forward.
    /// Requires semantic prompt markers from the shell. Zero or no matching
    /// prompt is a no-op. If fewer prompts remain than requested, move to
    /// the last matching prompt, clamped to the active area.
    ///
    /// Moving forward through a final prompt continuation can snap to the
    /// active area even when there is no newer prompt.
    DeltaPrompt(isize),
}
impl From<ScrollViewport> for ffi::TerminalScrollViewport {
    fn from(value: ScrollViewport) -> Self {
        match value {
            ScrollViewport::Top => Self {
                tag: ffi::TerminalScrollViewportTag::TOP,
                value: ffi::TerminalScrollViewportValue::default(),
            },
            ScrollViewport::Bottom => Self {
                tag: ffi::TerminalScrollViewportTag::BOTTOM,
                value: ffi::TerminalScrollViewportValue::default(),
            },
            ScrollViewport::Delta(delta) => Self {
                tag: ffi::TerminalScrollViewportTag::DELTA,
                value: {
                    let mut v = ffi::TerminalScrollViewportValue::default();
                    v.delta = delta;
                    v
                },
            },
            ScrollViewport::Row(row) => Self {
                tag: ffi::TerminalScrollViewportTag::ROW,
                value: {
                    let mut v = ffi::TerminalScrollViewportValue::default();
                    v.row = row;
                    v
                },
            },
            ScrollViewport::DeltaPrompt(delta_prompt) => Self {
                tag: ffi::TerminalScrollViewportTag::DELTA_PROMPT,
                value: {
                    let mut v = ffi::TerminalScrollViewportValue::default();
                    v.delta_prompt = delta_prompt;
                    v
                },
            },
        }
    }
}

/// A terminal mode consisting of its value and its kind (DEC/ANSI).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mode(pub ffi::Mode);

impl Mode {
    #![expect(missing_docs, reason = "no upstream documentation provided")]
    const ANSI_BIT: u16 = 1 << 15;

    /// Create a new mode from its numeric value and its kind.
    #[must_use]
    pub const fn new(v: u16, kind: ModeKind) -> Self {
        match kind {
            ModeKind::Ansi => Self(v | Self::ANSI_BIT),
            ModeKind::Dec => Self(v),
        }
    }

    /// The numeric value of the mode.
    #[must_use]
    pub const fn value(self) -> u16 {
        (self.0) & 0x7fff
    }

    /// The kind of the mode (DEC/ANSI).
    #[must_use]
    pub const fn kind(self) -> ModeKind {
        if (self.0) & Self::ANSI_BIT > 0 {
            ModeKind::Ansi
        } else {
            ModeKind::Dec
        }
    }

    pub const KAM: Self = Self::new(2, ModeKind::Ansi);
    pub const INSERT: Self = Self::new(4, ModeKind::Ansi);
    pub const SRM: Self = Self::new(12, ModeKind::Ansi);
    pub const LINEFEED: Self = Self::new(20, ModeKind::Ansi);

    pub const DECCKM: Self = Self::new(1, ModeKind::Dec);
    pub const _132_COLUMN: Self = Self::new(3, ModeKind::Dec);
    pub const SLOW_SCROLL: Self = Self::new(4, ModeKind::Dec);
    pub const REVERSE_COLORS: Self = Self::new(5, ModeKind::Dec);
    pub const ORIGIN: Self = Self::new(6, ModeKind::Dec);
    pub const WRAPAROUND: Self = Self::new(7, ModeKind::Dec);
    pub const AUTOREPEAT: Self = Self::new(8, ModeKind::Dec);
    pub const X10_MOUSE: Self = Self::new(9, ModeKind::Dec);
    pub const CURSOR_BLINKING: Self = Self::new(12, ModeKind::Dec);
    pub const CURSOR_VISIBLE: Self = Self::new(25, ModeKind::Dec);
    pub const ENABLE_MODE3: Self = Self::new(40, ModeKind::Dec);
    pub const REVERSE_WRAP: Self = Self::new(45, ModeKind::Dec);
    pub const ALT_SCREEN_LEGACY: Self = Self::new(47, ModeKind::Dec);
    pub const KEYPAD_KEYS: Self = Self::new(66, ModeKind::Dec);
    pub const LEFT_RIGHT_MARGIN: Self = Self::new(69, ModeKind::Dec);
    pub const NORMAL_MOUSE: Self = Self::new(1000, ModeKind::Dec);
    pub const BUTTON_MOUSE: Self = Self::new(1002, ModeKind::Dec);
    pub const ANY_MOUSE: Self = Self::new(1003, ModeKind::Dec);
    pub const FOCUS_EVENT: Self = Self::new(1004, ModeKind::Dec);
    pub const UTF8_MOUSE: Self = Self::new(1005, ModeKind::Dec);
    pub const SGR_MOUSE: Self = Self::new(1006, ModeKind::Dec);
    pub const ALT_SCROLL: Self = Self::new(1007, ModeKind::Dec);
    pub const URXVT_MOUSE: Self = Self::new(1015, ModeKind::Dec);
    pub const SGR_PIXELS_MOUSE: Self = Self::new(1016, ModeKind::Dec);
    pub const NUMLOCK_KEYPAD: Self = Self::new(1035, ModeKind::Dec);
    pub const ALT_ESC_PREFIX: Self = Self::new(1036, ModeKind::Dec);
    pub const ALT_SENDS_ESC: Self = Self::new(1039, ModeKind::Dec);
    pub const REVERSE_WRAP_EXT: Self = Self::new(1045, ModeKind::Dec);
    pub const ALT_SCREEN: Self = Self::new(1047, ModeKind::Dec);
    pub const SAVE_CURSOR: Self = Self::new(1048, ModeKind::Dec);
    pub const ALT_SCREEN_SAVE: Self = Self::new(1049, ModeKind::Dec);
    pub const BRACKETED_PASTE: Self = Self::new(2004, ModeKind::Dec);
    pub const SYNC_OUTPUT: Self = Self::new(2026, ModeKind::Dec);
    pub const GRAPHEME_CLUSTER: Self = Self::new(2027, ModeKind::Dec);
    pub const COLOR_SCHEME_REPORT: Self = Self::new(2031, ModeKind::Dec);
    pub const VISIBILITY_REPORT: Self = Self::new(2033, ModeKind::Dec);
    pub const IN_BAND_RESIZE: Self = Self::new(2048, ModeKind::Dec);
    pub const PASTE_EVENTS: Self = Self::new(5522, ModeKind::Dec);
}

/// The kind of a terminal mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModeKind {
    /// DEC terminal mode.
    Dec,
    /// ANSI terminal mode.
    Ansi,
}

impl From<Mode> for ffi::Mode {
    fn from(value: Mode) -> Self {
        value.0
    }
}

/// Device attributes response data for all three DA levels.
/// Filled by the [`Terminal::on_device_attributes`] callback in response
/// to CSI c, CSI > c, or CSI = c queries. The terminal uses whichever
/// sub-struct matches the request type.
#[derive(Debug, Clone, Copy)]
pub struct DeviceAttributes {
    /// Primary device attributes (DA1).
    pub primary: PrimaryDeviceAttributes,
    /// Secondary device attributes (DA2).
    pub secondary: SecondaryDeviceAttributes,
    /// Tertiary device attributes (DA3).
    pub tertiary: TertiaryDeviceAttributes,
}

impl From<DeviceAttributes> for ffi::DeviceAttributes {
    fn from(value: DeviceAttributes) -> Self {
        Self {
            primary: value.primary.into(),
            secondary: value.secondary.into(),
            tertiary: value.tertiary.into(),
        }
    }
}

/// Primary device attributes (DA1) response data.
///
/// Returned as part of [`DeviceAttributes`] in response to a CSI c query.
#[derive(Debug, Clone, Copy)]
pub struct PrimaryDeviceAttributes(ffi::DeviceAttributesPrimary);

impl PrimaryDeviceAttributes {
    /// Construct primary device attributes from a conformance level
    /// and an array of device attribute features.
    ///
    /// Prefer defining primary device attributes as a `const` when the feature
    /// list is statically known. That makes the 64-feature limit fail during
    /// compilation instead of panicking at runtime.
    ///
    /// # Panics
    ///
    /// **Panics** when more than 64 features are given.
    #[must_use]
    pub const fn new(
        conformance_level: ConformanceLevel,
        features: &[DeviceAttributeFeature],
    ) -> Self {
        assert!(features.len() <= 64);

        let mut f = [0u16; 64];
        let mut i = 0;
        while i < features.len() {
            f[i] = features[i].0;
            i += 1;
        }

        Self(ffi::DeviceAttributesPrimary {
            conformance_level: conformance_level.0,
            features: f,
            num_features: features.len(),
        })
    }
}

impl From<PrimaryDeviceAttributes> for ffi::DeviceAttributesPrimary {
    fn from(value: PrimaryDeviceAttributes) -> Self {
        value.0
    }
}

/// The level of conformance to the behavior of a specific or a family of
/// physical terminal models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConformanceLevel(pub u16);

impl ConformanceLevel {
    #![expect(clippy::doc_markdown, reason = "false positive")]
    #![expect(missing_docs, reason = "self-explanatory")]
    pub const VT100: Self = Self(ffi::DA_CONFORMANCE_VT100);
    pub const VT101: Self = Self(ffi::DA_CONFORMANCE_VT101);
    pub const VT102: Self = Self(ffi::DA_CONFORMANCE_VT102);
    pub const VT125: Self = Self(ffi::DA_CONFORMANCE_VT125);
    pub const VT131: Self = Self(ffi::DA_CONFORMANCE_VT131);
    pub const VT132: Self = Self(ffi::DA_CONFORMANCE_VT132);
    pub const VT220: Self = Self(ffi::DA_CONFORMANCE_VT220);
    pub const VT240: Self = Self(ffi::DA_CONFORMANCE_VT240);
    pub const VT320: Self = Self(ffi::DA_CONFORMANCE_VT320);
    pub const VT340: Self = Self(ffi::DA_CONFORMANCE_VT340);
    pub const VT420: Self = Self(ffi::DA_CONFORMANCE_VT420);
    pub const VT510: Self = Self(ffi::DA_CONFORMANCE_VT510);
    pub const VT520: Self = Self(ffi::DA_CONFORMANCE_VT520);
    pub const VT525: Self = Self(ffi::DA_CONFORMANCE_VT525);
    /// Equivalent to a VT2xx terminal.
    pub const LEVEL_2: Self = Self(ffi::DA_CONFORMANCE_LEVEL_2);
    /// Equivalent to a VT3xx terminal.
    pub const LEVEL_3: Self = Self(ffi::DA_CONFORMANCE_LEVEL_3);
    /// Equivalent to a VT4xx terminal.
    pub const LEVEL_4: Self = Self(ffi::DA_CONFORMANCE_LEVEL_4);
    /// Equivalent to a VT5xx terminal.
    pub const LEVEL_5: Self = Self(ffi::DA_CONFORMANCE_LEVEL_5);
}

/// A feature that a terminal can report to support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceAttributeFeature(pub u16);

impl DeviceAttributeFeature {
    #![expect(missing_docs, reason = "no upstream documentation provided")]
    pub const COLUMNS_132: Self = Self(ffi::DA_FEATURE_COLUMNS_132);
    pub const PRINTER: Self = Self(ffi::DA_FEATURE_PRINTER);
    pub const REGIS: Self = Self(ffi::DA_FEATURE_REGIS);
    pub const SIXEL: Self = Self(ffi::DA_FEATURE_SIXEL);
    pub const SELECTIVE_ERASE: Self = Self(ffi::DA_FEATURE_SELECTIVE_ERASE);
    pub const USER_DEFINED_KEYS: Self = Self(ffi::DA_FEATURE_USER_DEFINED_KEYS);
    pub const NATIONAL_REPLACEMENT: Self = Self(ffi::DA_FEATURE_NATIONAL_REPLACEMENT);
    pub const TECHNICAL_CHARACTERS: Self = Self(ffi::DA_FEATURE_TECHNICAL_CHARACTERS);
    pub const LOCATOR: Self = Self(ffi::DA_FEATURE_LOCATOR);
    pub const TERMINAL_STATE: Self = Self(ffi::DA_FEATURE_TERMINAL_STATE);
    pub const WINDOWING: Self = Self(ffi::DA_FEATURE_WINDOWING);
    pub const HORIZONTAL_SCROLLING: Self = Self(ffi::DA_FEATURE_HORIZONTAL_SCROLLING);
    pub const ANSI_COLOR: Self = Self(ffi::DA_FEATURE_ANSI_COLOR);
    pub const RECTANGULAR_EDITING: Self = Self(ffi::DA_FEATURE_RECTANGULAR_EDITING);
    pub const ANSI_TEXT_LOCATOR: Self = Self(ffi::DA_FEATURE_ANSI_TEXT_LOCATOR);
    pub const CLIPBOARD: Self = Self(ffi::DA_FEATURE_CLIPBOARD);
}

/// Secondary device attributes (DA2) response data.
///
/// Returned as part of [`DeviceAttributes`] in response to a CSI > c query.
/// Response format: CSI > Pp ; Pv ; Pc c
#[derive(Debug, Copy, Clone)]
pub struct SecondaryDeviceAttributes {
    /// Terminal type identifier (Pp).
    pub device_type: DeviceType,
    /// Firmware/patch version number (Pv).
    pub firmware_version: u16,
    /// ROM cartridge registration number (Pc). Always 0 for emulators.
    pub rom_cartridge: u16,
}

impl From<SecondaryDeviceAttributes> for ffi::DeviceAttributesSecondary {
    fn from(value: SecondaryDeviceAttributes) -> Self {
        Self {
            device_type: value.device_type.0,
            firmware_version: value.firmware_version,
            rom_cartridge: value.rom_cartridge,
        }
    }
}

/// The type of terminal device being emulated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceType(pub u16);

impl DeviceType {
    #![expect(missing_docs, reason = "self-explanatory")]
    pub const VT100: Self = Self(ffi::DA_DEVICE_TYPE_VT100);
    pub const VT220: Self = Self(ffi::DA_DEVICE_TYPE_VT220);
    pub const VT240: Self = Self(ffi::DA_DEVICE_TYPE_VT240);
    pub const VT330: Self = Self(ffi::DA_DEVICE_TYPE_VT330);
    pub const VT340: Self = Self(ffi::DA_DEVICE_TYPE_VT340);
    pub const VT320: Self = Self(ffi::DA_DEVICE_TYPE_VT320);
    pub const VT382: Self = Self(ffi::DA_DEVICE_TYPE_VT382);
    pub const VT420: Self = Self(ffi::DA_DEVICE_TYPE_VT420);
    pub const VT510: Self = Self(ffi::DA_DEVICE_TYPE_VT510);
    pub const VT520: Self = Self(ffi::DA_DEVICE_TYPE_VT520);
    pub const VT525: Self = Self(ffi::DA_DEVICE_TYPE_VT525);
}

/// Tertiary device attributes (DA3) response data.
///
/// Returned as part of [`DeviceAttributes`] in response to a CSI = c query.
/// Response format: DCS ! | D...D ST (DECRPTUI).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TertiaryDeviceAttributes {
    /// Unit ID encoded as 8 uppercase hex digits in the response.
    pub unit_id: u32,
}

impl From<TertiaryDeviceAttributes> for ffi::DeviceAttributesTertiary {
    fn from(value: TertiaryDeviceAttributes) -> Self {
        Self {
            unit_id: value.unit_id,
        }
    }
}

bitflags::bitflags! {
    /// How the DECRQCRA checksum is calculated.
    ///
    /// These are the same bits as XTCHECKSUM (`CSI Ps # y`) and xterm's
    /// `checksumExtension` resource. The empty set is the calculation of a
    /// real DEC terminal.
    ///
    /// See [`Terminal::set_xt_checksum_extension`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct XtChecksumExtension: u8 {
        /// Don't negate the result.
        const NO_NEGATE = 1;
        /// Don't add the video attributes of each cell.
        const NO_ATTRIBUTES = 2;
        /// Don't omit blanks.
        const NO_OMIT_BLANKS = 4;
        /// Count cells that were never written to as spaces.
        const UNWRITTEN_AS_SPACES = 8;
        /// Use full codepoints instead of the DEC 8-bit values.
        const FULL_CODEPOINTS = 16;
    }
}

/// Color scheme reported in response to a CSI ? 996 n query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
#[expect(missing_docs, reason = "self-explanatory")]
pub enum ColorScheme {
    Light = ffi::ColorScheme::LIGHT,
    Dark = ffi::ColorScheme::DARK,
}

impl ColorScheme {
    /// Encode a color scheme report into an escape sequence.
    ///
    /// Encodes a color scheme report into the provided buffer. Dark color
    /// schemes emit `ESC [ ? 997 ; 1 n`, and light color schemes emit
    /// `ESC [ ? 997 ; 2 n`. The encoded bytes are identical to the terminal's
    /// internal `CSI ? 996 n` query response.
    ///
    /// Hosts should gate unsolicited sends on mode 2031 being set, which can
    /// be checked via the mode getters.
    ///
    /// If the buffer is too small, returns [`Error::OutOfSpace`] with the
    /// required buffer size. The caller can then retry with a sufficiently
    /// sized buffer.
    pub fn encode_report(self, buf: &mut [u8]) -> Result<usize> {
        let mut written = 0;
        let result = unsafe {
            ffi::ghostty_color_scheme_report_encode(
                self.into(),
                buf.as_mut_ptr().cast(),
                buf.len(),
                &raw mut written,
            )
        };
        from_result_with_len(result, written)
    }
}

impl From<ColorScheme> for ffi::ColorScheme::Type {
    fn from(value: ColorScheme) -> Self {
        match value {
            ColorScheme::Light => ffi::ColorScheme::LIGHT,
            ColorScheme::Dark => ffi::ColorScheme::DARK,
        }
    }
}

/// Amount of compression work to perform before returning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum CompressionMode {
    /// Perform one bounded compression step suitable for idle scheduling.
    Incremental = ffi::TerminalCompressionMode::INCREMENTAL,
    /// Synchronously inspect every currently eligible page.
    Full = ffi::TerminalCompressionMode::FULL,
}

/// Scheduling result from terminal compression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum CompressionResult {
    /// Retained-mapping reclamation is unavailable on this target.
    Unsupported = ffi::TerminalCompressionResult::UNSUPPORTED,
    /// More incremental compression work remains.
    Pending = ffi::TerminalCompressionResult::PENDING,
    /// The pass has no continuation to schedule.
    Complete = ffi::TerminalCompressionResult::COMPLETE,
}

/// Opaque token representing a terminal's current compression activity.
///
/// The token is opaque and only equality comparisons are meaningful.
/// An embedding application should cache it and restart its compression idle
/// delay whenever the value changes. The value may wrap and changes in either
/// direction have the same meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompressionActivity(u64);

/// A synchronous request to write clipboard contents.
///
/// The request, contents array, MIME strings, and data strings are all
/// borrowed and valid only for the callback duration.
///
/// All entries in [`contents`](Self::contents) are representations of the
/// same logical value and must be committed atomically. An empty `contents`
/// requests that the destination be cleared. This is distinct from a content
/// entry whose data has zero length.
///
/// The write is answered by calling [`reply`](Self::reply). This must happen
/// within the clipboard write request callback. Returning without replying
/// denies the write.
///
/// As the C API requires, only the fields within the size libghostty reports
/// for the request are read. Any that don't fit read as empty or false, and
/// [`reply`](Self::reply) then does nothing, which denies the write.
#[derive(Debug)]
pub struct ClipboardWrite<'t> {
    ptr: *const ffi::ClipboardWrite,
    _phan: PhantomData<&'t ()>,
}

impl<'t> ClipboardWrite<'t> {
    /// Name of the writing program for permission prompts, if the protocol
    /// carries one. Empty otherwise.
    #[must_use]
    pub fn name(&self) -> &'t [u8] {
        // SAFETY: The request and its strings live for the callback duration.
        unsafe {
            crate::sized_field!(self.ptr, ffi::ClipboardWrite, name).map_or(&[], |n| n.to_bytes())
        }
    }

    /// True if the terminal already holds a session grant for this request.
    /// The embedder should skip any permission prompt and perform the write.
    #[must_use]
    pub fn granted(&self) -> bool {
        // SAFETY: The request lives for the callback duration.
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardWrite, granted) }.unwrap_or(false)
    }

    /// True if the program supplied a session password, so the embedder may
    /// offer to remember the user's decision through the `remember` argument
    /// of [`reply`](Self::reply). When false, `remember` is ignored.
    #[must_use]
    pub fn can_remember(&self) -> bool {
        // SAFETY: The request lives for the callback duration.
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardWrite, can_remember) }.unwrap_or(false)
    }

    /// Answer the write.
    ///
    /// The result answers the program with the matching protocol status for
    /// protocols with a write acknowledgement (OSC 5522: DONE, EPERM, ENOSYS,
    /// EBUSY, EINVAL, EIO); protocols without one (OSC 52, OSC 1337 Copy)
    /// discard the reply.
    ///
    /// `remember` records a session grant so future requests from the same
    /// program skip the permission prompt. It is only honored on success when
    /// [`can_remember`](Self::can_remember) is set.
    pub fn reply(self, result: std::result::Result<(), ClipboardWriteError>, remember: bool) {
        let reply = ffi::ClipboardWriteReply {
            result: result.map_or_else(Into::into, |()| ffi::ClipboardWriteResult::SUCCESS),
            remember,
            ..ffi::sized!(ffi::ClipboardWriteReply)
        };
        // SAFETY: The request lives for the callback duration.
        if let Some(callback) =
            unsafe { crate::sized_field!(self.ptr, ffi::ClipboardWrite, reply) }.flatten()
        {
            // SAFETY: The reply only needs to outlive this synchronous call.
            unsafe { callback(self.ptr, &raw const reply) };
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the given pointer has the correct lifetime.
    unsafe fn from_raw(ptr: *const ffi::ClipboardWrite) -> Self {
        Self {
            ptr,
            _phan: PhantomData,
        }
    }

    /// Get the clipboard's destination.
    #[must_use]
    pub fn location(&self) -> ClipboardLocation {
        // SAFETY: We trust libghostty to give us a valid pointer
        // within the lifetime of the callback.
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardWrite, location) }
            .and_then(|location| location.try_into().ok())
            .unwrap_or(ClipboardLocation::Standard)
    }
    /// Get an iterator into a borrowed array of MIME representations.
    ///
    /// The iterator is empty for a write carrying no representations, which
    /// requests that the destination be cleared (e.g. OSC 52 with an empty
    /// payload).
    #[must_use]
    pub fn contents(&self) -> ClipboardContents<'t> {
        // SAFETY: We trust libghostty to give us a valid pointer
        // within the lifetime of the callback.
        let (ptr, len) = unsafe {
            (
                crate::sized_field!(self.ptr, ffi::ClipboardWrite, contents),
                crate::sized_field!(self.ptr, ffi::ClipboardWrite, contents_len),
            )
        };
        // The C API declares `contents` optional and sends null for a write
        // carrying no representations (the "clear the clipboard" shape);
        // `from_raw_parts` requires a non-null pointer even at length zero.
        let contents: &'t [ffi::ClipboardContent] = match (ptr, len) {
            (Some(ptr), Some(len)) if !ptr.is_null() => {
                // SAFETY: We trust libghostty to give us a valid pointer and
                // length within the lifetime of the callback.
                unsafe { std::slice::from_raw_parts(ptr, len) }
            }
            _ => &[],
        };
        ClipboardContents(contents.iter())
    }
}

/// An iterator into a borrowed array of MIME representations.
#[derive(Clone, Debug)]
pub struct ClipboardContents<'t>(std::slice::Iter<'t, ffi::ClipboardContent>);

impl<'t> Iterator for ClipboardContents<'t> {
    type Item = ClipboardContent<'t>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|v| unsafe { ClipboardContent::from_raw(v) })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl DoubleEndedIterator for ClipboardContents<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|v| unsafe { ClipboardContent::from_raw(v) })
    }
}
impl ExactSizeIterator for ClipboardContents<'_> {}
impl std::iter::FusedIterator for ClipboardContents<'_> {}

/// One MIME representation in a clipboard write.
///
/// Both strings are borrowed and valid only for the duration of the callback.
/// The data is binary-safe and has already been decoded from any protocol-level
/// encoding. A zero-length data string is an explicit empty representation; it
/// does not clear the clipboard.
#[derive(Clone, Copy, Debug)]
pub struct ClipboardContent<'t> {
    /// MIME type of the representation.
    ///
    /// libghostty does not promise the MIME type is UTF-8, so it is exposed as
    /// raw bytes.
    pub mime: &'t [u8],
    /// Decoded, binary-safe representation data.
    pub data: &'t [u8],
}
impl ClipboardContent<'_> {
    /// # Safety
    ///
    /// Caller must guarantee that the given raw value is valid within
    /// the given lifetime.
    unsafe fn from_raw(value: &ffi::ClipboardContent) -> Self {
        // SAFETY: Upheld by caller
        unsafe {
            Self {
                // The C API does not promise UTF-8 for the MIME type, so
                // hand the bytes over as-is and let callers validate.
                mime: value.mime.to_bytes(),
                // The data is binary-safe per the C API (e.g. an image/png
                // representation), so it must not be exposed as `str`.
                data: value.data.to_bytes(),
            }
        }
    }
}

/// Clipboard a clipboard write targets or a clipboard read reads from.
///
/// Protocol-specific destination identifiers are normalized to these values
/// before the clipboard write callback is invoked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum ClipboardLocation {
    /// The standard system clipboard.
    Standard = ffi::ClipboardLocation::STANDARD,
    /// The selection clipboard.
    Selection = ffi::ClipboardLocation::SELECTION,
    /// The primary selection clipboard.
    Primary = ffi::ClipboardLocation::PRIMARY,
}

/// Result of a clipboard write reply.
///
/// Protocols with a write acknowledgement (OSC 5522) answer the program with
/// the matching status; protocols without one (OSC 52, OSC 1337 Copy) discard
/// the reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum ClipboardWriteError {
    /// The clipboard write was denied by policy or the user.
    Denied = ffi::ClipboardWriteResult::DENIED,
    /// The destination or one or more representations are unsupported.
    Unsupported = ffi::ClipboardWriteResult::UNSUPPORTED,
    /// The clipboard is temporarily unavailable.
    Busy = ffi::ClipboardWriteResult::BUSY,
    /// One or more representations contain invalid data.
    InvalidData = ffi::ClipboardWriteResult::INVALID_DATA,
    /// The clipboard write failed due to an I/O error.
    IoError = ffi::ClipboardWriteResult::IO_ERROR,
}

/// A synchronous request to read clipboard contents.
///
/// The request is borrowed and valid only for the callback duration.
///
/// The read is answered by calling [`reply`](Self::reply). This must happen
/// before the callback returns. Returning without replying answers the
/// program with an empty clipboard (OSC 52) or EPERM (OSC 5522).
///
/// As the C API requires, only the fields within the size libghostty reports
/// for the request are read. Any that don't fit read as empty or false, and
/// [`reply`](Self::reply) then does nothing.
#[derive(Debug)]
pub struct ClipboardRead<'t> {
    ptr: *const ffi::ClipboardRead,
    _phan: PhantomData<&'t ()>,
}

impl<'t> ClipboardRead<'t> {
    /// # Safety
    ///
    /// Caller must ensure that the given pointer is valid for `'t`.
    unsafe fn from_raw(ptr: *const ffi::ClipboardRead) -> Self {
        Self {
            ptr,
            _phan: PhantomData,
        }
    }

    /// Clipboard to read.
    ///
    /// Locations this version of the bindings does not know about are
    /// reported as [`ClipboardLocation::Standard`]. That only happens when
    /// linking a newer libghostty than these bindings were generated for.
    #[must_use]
    pub fn location(&self) -> ClipboardLocation {
        // SAFETY: The request lives for the callback duration.
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, location) }
            .and_then(|location| location.try_into().ok())
            .unwrap_or(ClipboardLocation::Standard)
    }

    /// The MIME types the program wants, in order of preference. Protocols
    /// that only carry text (OSC 52) request `text/plain`.
    ///
    /// The values come straight from the program, so they are exposed as
    /// raw bytes.
    #[must_use]
    pub fn mimes(&self) -> impl ExactSizeIterator<Item = &'t [u8]> + use<'t> {
        // SAFETY: The request lives for the callback duration.
        let (ptr, len) = unsafe {
            (
                crate::sized_field!(self.ptr, ffi::ClipboardRead, mimes),
                crate::sized_field!(self.ptr, ffi::ClipboardRead, mimes_len),
            )
        };
        // `mimes` is NULL when `mimes_len` is zero (a targets-only OSC 5522
        // read), and `from_raw_parts` requires a non-null pointer even at
        // length zero. Check both, as `ClipboardWrite::contents` does, so
        // a NULL pointer can never reach it.
        let mimes: &'t [ffi::String] = match (ptr, len) {
            (Some(ptr), Some(len)) if !ptr.is_null() && len > 0 => {
                // SAFETY: libghostty provides `mimes_len` strings that live
                // for the callback duration.
                unsafe { std::slice::from_raw_parts(ptr, len) }
            }
            _ => &[],
        };
        // SAFETY: Each string lives for the callback duration.
        mimes.iter().map(|mime| unsafe { mime.to_bytes() })
    }

    /// True if the program also wants the list of MIME types available on
    /// the clipboard, delivered through the `available` argument of
    /// [`reply`](Self::reply).
    #[must_use]
    pub fn list(&self) -> bool {
        // SAFETY: The request lives for the callback duration.
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, list) }.unwrap_or(false)
    }

    /// Name of the requesting program for permission prompts, if the protocol
    /// carries one. Empty otherwise.
    #[must_use]
    pub fn name(&self) -> &'t [u8] {
        // SAFETY: The request and its strings live for the callback duration.
        unsafe {
            crate::sized_field!(self.ptr, ffi::ClipboardRead, name).map_or(&[], |n| n.to_bytes())
        }
    }

    /// True if the terminal already holds a session grant for this request
    /// (kitty clipboard protocol passwords). The embedder should skip any
    /// permission prompt and serve the read.
    ///
    /// Always false when [`mimes`](Self::mimes) is empty: such a request is
    /// served without a prompt (see [`Terminal::on_clipboard_read`]), so the
    /// terminal never consults grants for it and a one-time password is
    /// preserved for the follow-up data read.
    #[must_use]
    pub fn granted(&self) -> bool {
        // SAFETY: The request lives for the callback duration.
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, granted) }.unwrap_or(false)
    }

    /// True if the program supplied a session password, so the embedder may
    /// offer to remember the user's decision through the `remember` argument
    /// of [`reply`](Self::reply). When false, `remember` is ignored.
    #[must_use]
    pub fn can_remember(&self) -> bool {
        // SAFETY: The request lives for the callback duration.
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, can_remember) }.unwrap_or(false)
    }

    /// Answer the read.
    ///
    /// Any error answers the program with an empty clipboard (OSC 52) or the
    /// matching protocol status (OSC 5522: EPERM, ENOSYS, EBUSY, EIO); the
    /// other arguments are ignored in that case. On success, `contents`
    /// should carry one representation per requested MIME type
    /// ([`mimes`](Self::mimes)) that the clipboard has; unrequested
    /// representations are ignored. Protocols that carry a single text value
    /// (OSC 52) use the first entry with a text MIME type such as
    /// `text/plain`.
    ///
    /// `available` lists all MIME types available on the clipboard. It is
    /// only used when [`list`](Self::list) is set.
    ///
    /// `remember` records a session grant so future requests from the same
    /// program skip the permission prompt. It is only honored on success when
    /// [`can_remember`](Self::can_remember) is set.
    ///
    /// All arguments are borrowed only for the duration of this call.
    ///
    /// The answer is written to the pty before this returns, so the
    /// [pty write callback](Terminal::on_pty_write) runs inside this call. If
    /// both callbacks share state through a [`RefCell`](std::cell::RefCell),
    /// don't keep it borrowed across `reply`: the pty write callback's borrow
    /// would panic, and a panic in a callback aborts the process.
    pub fn reply(
        self,
        result: std::result::Result<&[ClipboardReplyContent<'_>], ClipboardReadError>,
        available: &[ClipboardMime<'_>],
        remember: bool,
    ) {
        let (result, contents) = match result {
            Ok(contents) => (ffi::ClipboardReadResult::SUCCESS, contents),
            Err(error) => (error.into(), &[][..]),
        };
        // Both wrapper types are `repr(transparent)` over their C
        // counterparts, so the slices can be handed over without copying.
        let reply = ffi::ClipboardReadReply {
            result,
            contents: contents.as_ptr().cast(),
            contents_len: contents.len(),
            available: available.as_ptr().cast(),
            available_len: available.len(),
            remember,
            ..ffi::sized!(ffi::ClipboardReadReply)
        };
        // SAFETY: The request lives for the callback duration.
        if let Some(callback) =
            unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, reply) }.flatten()
        {
            // SAFETY: The reply and the buffers it points to only need to
            // outlive this synchronous call.
            unsafe { callback(self.ptr, &raw const reply) };
        }
    }
}

/// One MIME representation in a [`ClipboardRead::reply`].
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct ClipboardReplyContent<'a> {
    raw: ffi::ClipboardContent,
    _phan: PhantomData<&'a [u8]>,
}

impl<'a> ClipboardReplyContent<'a> {
    /// A representation of the clipboard contents with the given MIME type.
    ///
    /// The data is binary-safe.
    #[must_use]
    pub const fn new(mime: &'a str, data: &'a [u8]) -> Self {
        Self {
            raw: ffi::ClipboardContent {
                mime: ffi::String {
                    ptr: mime.as_ptr(),
                    len: mime.len(),
                },
                data: ffi::String {
                    ptr: data.as_ptr(),
                    len: data.len(),
                },
            },
            _phan: PhantomData,
        }
    }
}

/// A MIME type passed to libghostty without copying, as listed in a
/// [`ClipboardRead::reply`] or offered to [`Terminal::paste`].
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct ClipboardMime<'a> {
    raw: ffi::String,
    _phan: PhantomData<&'a str>,
}

impl<'a> ClipboardMime<'a> {
    /// A MIME type such as `text/plain`.
    #[must_use]
    pub const fn new(mime: &'a str) -> Self {
        Self {
            raw: ffi::String {
                ptr: mime.as_ptr(),
                len: mime.len(),
            },
            _phan: PhantomData,
        }
    }
}

/// Errors that can be returned in a [`ClipboardRead::reply`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum ClipboardReadError {
    /// The clipboard read was denied by policy or the user.
    Denied = ffi::ClipboardReadResult::DENIED,
    /// The embedder cannot read this clipboard.
    Unsupported = ffi::ClipboardReadResult::UNSUPPORTED,
    /// The clipboard is temporarily unavailable.
    Busy = ffi::ClipboardReadResult::BUSY,
    /// Reading the clipboard failed due to an I/O error.
    IoError = ffi::ClipboardReadResult::IO_ERROR,
}

/// An unsupported terminal sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnknownSequence<'t> {
    /// Application Program Command (APC).
    ///
    /// The content is borrowed and valid only for the callback duration. It
    /// contains the bytes between the sequence introducer and terminator, may
    /// contain arbitrary binary data, and is not null-terminated.
    #[non_exhaustive]
    Apc {
        /// Retained sequence content.
        content: &'t [u8],
        /// Whether content was shortened by the byte limit or allocation
        /// failure.
        truncated: bool,
    },
    /// An OSC sequence whose number libghostty-vt does not implement.
    ///
    /// OSC sequences start with `ESC ]`, followed by a number that identifies
    /// the command, usually a `;`, and then the command's data. The sequence
    /// ends with either BEL or ESC followed by a backslash. For example, a
    /// program might write:
    ///
    /// ```text
    /// ESC ] 7400;status=busy BEL
    /// ```
    ///
    /// For that sequence, `content` is `7400;status=busy` and `terminator`
    /// is [`osc::Terminator::Bel`]. See [`Terminal::on_unknown_sequence`]
    /// for a complete example.
    #[non_exhaustive]
    Osc {
        /// Everything between `ESC ]` and the terminator, including the number
        /// at the start. The bytes are not null-terminated and are only valid
        /// until the callback returns. Copy them if you need them later.
        content: &'t [u8],
        /// True if the sequence was longer than
        /// [`Terminal::set_unknown_max_bytes`], or memory ran out while
        /// reading it. In that case `content` holds only the beginning of the
        /// sequence.
        truncated: bool,
        /// How the program ended the sequence. If you send a reply, end it the
        /// same way.
        terminator: osc::Terminator,
    },
}

impl UnknownSequence<'_> {
    /// Reads a sequence, or gives `None` for a kind or OSC terminator these
    /// bindings don't know.
    ///
    /// This takes a pointer rather than a reference on purpose: upstream
    /// writes only the active union field, and for a kind without a payload
    /// it may leave the whole union unwritten. Referencing or copying such a
    /// union relies on validity rules Rust hasn't settled yet. So we only
    /// read the tag, and then only the union field the tag says is active.
    ///
    /// # Safety
    ///
    /// Caller must guarantee that `raw` is valid for reads and properly
    /// aligned, that its `tag` is initialized, that the union field named by
    /// `tag` is initialized, and that the content it points to is valid for
    /// the lifetime `'t`.
    unsafe fn from_raw<'t>(
        raw: *const ffi::TerminalUnknownSequence,
    ) -> Option<UnknownSequence<'t>> {
        // SAFETY: Upheld by caller. This reads just the tag field through
        // the raw pointer, without making a reference to the whole struct.
        match unsafe { (*raw).tag } {
            ffi::TerminalUnknownSequenceTag::APC => {
                // SAFETY: Upheld by caller; the tag says `apc` is the active
                // union field.
                let apc = unsafe { (*raw).value.apc };
                Some(UnknownSequence::Apc {
                    // SAFETY: Upheld by caller; the content is valid for `'t`.
                    content: unsafe { apc.content.to_bytes() },
                    truncated: apc.truncated,
                })
            }
            ffi::TerminalUnknownSequenceTag::OSC => {
                // SAFETY: Upheld by caller; the tag says `osc` is the active
                // union field.
                let osc = unsafe { (*raw).value.osc };
                // A terminator these bindings don't know has no Rust value
                // either, so skip the sequence (like `osc::Command`, which
                // reports it as `Invalid`) instead of inventing one.
                let terminator = osc.terminator.try_into().ok()?;
                Some(UnknownSequence::Osc {
                    // SAFETY: Ditto
                    content: unsafe { osc.content.to_bytes() },
                    truncated: osc.truncated,
                    terminator,
                })
            }
            // Upstream asks callbacks to ignore tags they don't handle.
            // Sequence kinds added upstream later have no Rust value, so they
            // are skipped rather than misreported; the enum is non-exhaustive
            // to grow with them.
            _ => None,
        }
    }
}

/// A request to show a desktop notification.
#[derive(Debug, Copy, Clone)]
pub struct DesktopNotification<'t> {
    ptr: *const ffi::TerminalDesktopNotification,
    _phan: PhantomData<&'t ()>,
}

impl<'t> DesktopNotification<'t> {
    unsafe fn from_raw(raw: *const ffi::TerminalDesktopNotification) -> Self {
        Self {
            ptr: raw,
            _phan: PhantomData,
        }
    }

    /// Notification title, or empty when the protocol omits it.
    ///
    /// The bytes come straight from the program and libghostty does not
    /// validate them, so they are not guaranteed to be UTF-8.
    #[must_use]
    pub fn title(self) -> &'t [u8] {
        // SAFETY: The notification and its strings live for the callback
        // duration.
        unsafe {
            crate::sized_field!(self.ptr, ffi::TerminalDesktopNotification, title)
                .map_or(&[], |title| title.to_bytes())
        }
    }

    /// Notification body.
    ///
    /// Like the title, these are the program's bytes, not necessarily UTF-8.
    #[must_use]
    pub fn body(self) -> &'t [u8] {
        // SAFETY: The notification and its strings live for the callback
        // duration.
        unsafe {
            crate::sized_field!(self.ptr, ffi::TerminalDesktopNotification, body)
                .map_or(&[], |body| body.to_bytes())
        }
    }
}

/// A progress report emitted by the running program.
#[derive(Debug, Copy, Clone)]
pub struct ProgressReport<'t> {
    ptr: *const ffi::TerminalProgressReport,
    _phan: PhantomData<&'t ()>,
}

impl ProgressReport<'_> {
    unsafe fn from_raw(raw: *const ffi::TerminalProgressReport) -> Self {
        Self {
            ptr: raw,
            _phan: PhantomData,
        }
    }

    /// Literal progress state reported by the running program.
    pub fn state(self) -> Result<ProgressState> {
        // SAFETY: We trust libghostty to give us a valid underlying ptr
        unsafe { *self.ptr }
            .state
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }

    /// Progress percentage from 0 through 100, or `None` when omitted.
    #[must_use]
    pub fn progress(self) -> Option<u8> {
        // SAFETY: We trust libghostty to give us a valid underlying ptr
        // Negative means omitted.
        u8::try_from(unsafe { *self.ptr }.progress).ok()
    }
}

/// State of a terminal progress report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum ProgressState {
    /// Remove any visible progress indication.
    Remove = ffi::TerminalProgressState::REMOVE,
    /// Show determinate progress.
    Set = ffi::TerminalProgressState::SET,
    /// Show a failed progress state.
    Error = ffi::TerminalProgressState::ERROR,
    /// Show indeterminate progress.
    Indeterminate = ffi::TerminalProgressState::INDETERMINATE,
    /// Show paused progress.
    Pause = ffi::TerminalProgressState::PAUSE,
}

/// A program status report (OSC 7501).
///
/// The program status protocol lets a program tell the terminal what it is
/// doing: idle, working, done, waiting on the user, or failed, and why. It
/// is meant for long-running work like builds, deploys, and coding agents,
/// where the user is often looking at something else and wants to know when
/// the work finishes or needs them. The protocol only describes state. How
/// to show it, if at all, is up to your application.
///
/// The full specification is at
/// <https://www.superlogical.com/rex/docs/build/program-status>
///
/// For example, a program waiting for the user to approve a change sends
/// this, where ST is the string terminator (ESC \ or BEL):
///
/// ```text
/// ESC ] 7501 ; state=blocked:kind=permission:app=terraform:msg=QXBwbHk/ ST
/// ```
///
/// The callback then receives a report with:
///
/// - [`state`](Self::state): [`ProgramStatusState::Blocked`]
/// - [`kind`](Self::kind): [`ProgramStatusKind::Permission`]
/// - [`progress`](Self::progress): `None`, because the program didn't send
///   one, so the work is indeterminate
/// - [`id`](Self::id): empty, because this is the root record
/// - [`app`](Self::app): `"terraform"`
/// - [`title`](Self::title): empty
/// - [`message`](Self::message): `"Apply?"`, decoded from the base64 in `msg`
///
/// Only reports that pass every check in the specification reach the
/// callback. Text that the program didn't send is an empty string. All
/// strings are only valid during the callback, so copy any you want to keep.
/// The lifetime `'t` enforces this: the report cannot escape the
/// [`Terminal::on_program_status`] callback it was handed to.
#[derive(Debug, Copy, Clone)]
pub struct ProgramStatus<'t> {
    ptr: *const ffi::TerminalProgramStatus,
    _phan: PhantomData<&'t ()>,
}

impl<'t> ProgramStatus<'t> {
    /// # Safety
    ///
    /// Caller must guarantee that `raw` and the strings it points to are
    /// valid for the lifetime `'t`.
    unsafe fn from_raw(raw: *const ffi::TerminalProgramStatus) -> Self {
        Self {
            ptr: raw,
            _phan: PhantomData,
        }
    }

    /// The raw report.
    ///
    /// Every field has been present since the struct was introduced, so
    /// reading them doesn't need to consult `size` (unlike fields added
    /// later, which go through `sized_field!`).
    fn raw(self) -> &'t ffi::TerminalProgramStatus {
        // SAFETY: We trust libghostty to give us a valid underlying ptr,
        // which lives for the callback duration, i.e. `'t`.
        unsafe { &*self.ptr }
    }

    /// What the program is doing.
    ///
    /// A state value from a newer libghostty that these bindings don't know
    /// yet gives [`Error::InvalidValue`].
    pub fn state(self) -> Result<ProgramStatusState> {
        self.raw().state.try_into().map_err(|_| Error::InvalidValue)
    }

    /// What the program needs from the user. Only set for
    /// [`ProgramStatusState::Blocked`]. It is [`ProgramStatusKind::None`]
    /// for other states, when the program didn't say, or when it sent a kind
    /// this version of libghostty doesn't know.
    ///
    /// A kind value from a newer libghostty that these bindings don't know
    /// yet gives [`Error::InvalidValue`].
    pub fn kind(self) -> Result<ProgramStatusKind> {
        self.raw().kind.try_into().map_err(|_| Error::InvalidValue)
    }

    /// How far along the work is, from 0 through 100, or `None` when there
    /// is no percentage. Only set for [`ProgramStatusState::Working`] and
    /// [`ProgramStatusState::Blocked`]. For those states, `None` means the
    /// work is indeterminate: the program is busy but has no percentage to
    /// report. It is also `None` for other states and when the program sent
    /// a value outside that range.
    #[must_use]
    pub fn progress(self) -> Option<u8> {
        // Upstream uses -1 as the "no percentage" sentinel and otherwise
        // only ever sends 0 through 100, so every negative value is `None`.
        u8::try_from(self.raw().progress).ok()
    }

    /// Which record this report is about. Empty for the root record.
    ///
    /// A program that only reports on itself leaves this empty. A program
    /// that reports on several things at once gives each its own id, such as
    /// `"us-east"` and `"eu-west"` for a deploy to two regions. A `/` makes
    /// one record the child of another, so `"build/test"` is a child of
    /// `"build"`. The parent record doesn't have to exist.
    #[must_use]
    pub fn id(self) -> &'t [u8] {
        // SAFETY: The report and its strings live for the callback
        // duration, i.e. `'t`.
        unsafe { self.raw().id.to_bytes() }
    }

    /// A stable name for the program that a machine can match on, such as
    /// `"cargo"`, `"terraform"`, or `"claude-code"`. Use it as a key for
    /// grouping, filtering, or choosing an icon. It is not a label. The label
    /// is [`title`](Self::title). Empty when the report didn't include one.
    /// See [`Terminal::on_program_status`] for how an empty app is filled in
    /// from a parent record.
    #[must_use]
    pub fn app(self) -> &'t [u8] {
        // SAFETY: Ditto.
        unsafe { self.raw().app.to_bytes() }
    }

    /// A short label for the record, meant for people. Programs that report
    /// several records use this to tell them apart.
    #[must_use]
    pub fn title(self) -> &'t [u8] {
        // SAFETY: Ditto.
        unsafe { self.raw().title.to_bytes() }
    }

    /// One line of text for people, saying what the record is doing,
    /// waiting for, or has finished. You may shorten it to fit, but don't
    /// try to read meaning into it.
    #[must_use]
    pub fn message(self) -> &'t [u8] {
        // SAFETY: Ditto.
        unsafe { self.raw().message.to_bytes() }
    }
}

/// What a program says it is doing, in a program status report (OSC 7501).
///
/// See [`ProgramStatus`] for an overview of the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum ProgramStatusState {
    /// At rest, waiting for the user's next instruction. For example, an
    /// interactive tool sitting at its own prompt.
    Idle = ffi::ProgramStatusState::IDLE,
    /// Running on its own. The report may include a progress percentage.
    Working = ffi::ProgramStatusState::WORKING,
    /// Finished a piece of work, and the result is ready for the user to
    /// look at.
    Done = ffi::ProgramStatusState::DONE,
    /// Can't continue until the user does something.
    /// [`kind`](ProgramStatus::kind) says what the program needs and
    /// [`message`](ProgramStatus::message) says why. The report may include
    /// a progress percentage.
    Blocked = ffi::ProgramStatusState::BLOCKED,
    /// Failed and stopped.
    Error = ffi::ProgramStatusState::ERROR,
    /// Not a real state. Remove the record with this report's id and every
    /// record beneath it. If the id is empty, remove every record.
    Clear = ffi::ProgramStatusState::CLEAR,
}

/// What a blocked program needs from the user, in a program status report
/// (OSC 7501).
#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum ProgramStatusKind {
    /// The program didn't say, or the state isn't
    /// [`ProgramStatusState::Blocked`].
    None = ffi::ProgramStatusKind::NONE,
    /// Approval to do something, such as "Apply these changes?".
    Permission = ffi::ProgramStatusKind::PERMISSION,
    /// An answer the user has to type.
    Question = ffi::ProgramStatusKind::QUESTION,
    /// A login, password, token, or other credential.
    Auth = ffi::ProgramStatusKind::AUTH,
}

/// A shell integration event, passed to the
/// [`Terminal::on_semantic_prompt`] callback.
///
/// [`kind`](Self::kind) says which step of the command this is. The other
/// fields only carry information for the kinds listed on each field, and are
/// zero or empty otherwise.
///
/// Strings are only valid during the callback. Copy them if you need them
/// later. The lifetime `'t` enforces this: the event cannot escape the
/// [`Terminal::on_semantic_prompt`] callback it was handed to.
#[derive(Debug, Copy, Clone)]
pub struct SemanticPrompt<'t> {
    ptr: *const ffi::TerminalSemanticPrompt,
    _phan: PhantomData<&'t ()>,
}

impl<'t> SemanticPrompt<'t> {
    /// # Safety
    ///
    /// Caller must guarantee that `raw` and the strings it points to are
    /// valid for the lifetime `'t`.
    unsafe fn from_raw(raw: *const ffi::TerminalSemanticPrompt) -> Self {
        Self {
            ptr: raw,
            _phan: PhantomData,
        }
    }

    /// The raw event.
    ///
    /// Upstream promises every field we read has been present since the
    /// struct was introduced, so reading them doesn't need to consult
    /// `size`. Fields added later (upstream names a command identifier and
    /// a "redrawn prompt" flag as likely) must go through `sized_field!`.
    fn raw(self) -> &'t ffi::TerminalSemanticPrompt {
        // SAFETY: We trust libghostty to give us a valid underlying ptr,
        // which lives for the callback duration, i.e. `'t`.
        unsafe { &*self.ptr }
    }

    /// Which step of the command this event reports.
    ///
    /// A kind value from a newer libghostty that these bindings don't know
    /// yet gives [`Error::InvalidValue`].
    pub fn kind(self) -> Result<SemanticPromptKind> {
        self.raw().kind.try_into().map_err(|_| Error::InvalidValue)
    }

    /// Which prompt is starting. Set for
    /// [`SemanticPromptKind::PromptStart`]. Always [`PromptKind::Primary`]
    /// for other kinds.
    ///
    /// A prompt kind value from a newer libghostty that these bindings don't
    /// know yet gives [`Error::InvalidValue`].
    pub fn prompt_kind(self) -> Result<PromptKind> {
        self.raw()
            .prompt_kind
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }

    /// The command's exit code, or `None` if the shell didn't report one.
    /// Only ever `Some` for [`SemanticPromptKind::CommandEnd`].
    ///
    /// Exit codes can be negative, for example on Windows.
    #[must_use]
    pub fn exit_code(self) -> Option<i32> {
        // Upstream says to trust `has_exit_code` rather than any special
        // `exit_code` value (it writes 0 when there is none, which is also a
        // perfectly good exit code), so that's the only thing we look at.
        let raw = self.raw();
        raw.has_exit_code.then_some(raw.exit_code)
    }

    /// The command line that is about to run, for
    /// [`SemanticPromptKind::OutputStart`]. The shell sends it encoded, and
    /// this is the decoded text. Empty if the shell didn't send one or it
    /// couldn't be decoded.
    #[must_use]
    pub fn command(self) -> &'t [u8] {
        // Upstream decodes the command line but never promises the result
        // is UTF-8 (percent-encoding can produce any bytes), so hand out
        // the raw bytes and let callers validate.
        //
        // SAFETY: The event and its strings live for the callback
        // duration, i.e. `'t`.
        unsafe { self.raw().command.to_bytes() }
    }

    /// A description of what went wrong, for
    /// [`SemanticPromptKind::CommandEnd`] when the shell sent one. Empty
    /// otherwise. Few shells send this. The [exit code](Self::exit_code) is
    /// the usual way to tell whether a command failed.
    #[must_use]
    pub fn error(self) -> &'t [u8] {
        // SAFETY: Ditto.
        unsafe { self.raw().error.to_bytes() }
    }
}

/// The step of a command that a shell integration event reports.
///
/// More kinds may be added in later versions, so ignore any kind you don't
/// handle.
// Upstream's `GHOSTTY_SEMANTIC_PROMPT_INVALID` (0) is left out on purpose:
// it is "never reported" and only exists so a zeroed struct isn't mistaken
// for a real event. If it ever shows up, `kind()` gives `Err` like any other
// unknown value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum SemanticPromptKind {
    /// The shell started drawing a prompt.
    /// [`prompt_kind`](SemanticPrompt::prompt_kind) says which one.
    PromptStart = ffi::SemanticPromptKind::PROMPT_START,
    /// The prompt is drawn and the user can start typing a command.
    InputStart = ffi::SemanticPromptKind::INPUT_START,
    /// The user submitted the command and it started running. Anything the
    /// terminal receives after this is the command's output.
    OutputStart = ffi::SemanticPromptKind::OUTPUT_START,
    /// The command finished running.
    CommandEnd = ffi::SemanticPromptKind::COMMAND_END,
}

/// Which prompt a [`SemanticPromptKind::PromptStart`] event starts.
///
/// Most shells only draw a primary prompt. Some also draw a prompt on the
/// right side of the line, or a prompt at the start of each extra line
/// when a command spans several lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum PromptKind {
    /// The main prompt shown before each command. This is used when the
    /// shell doesn't say which prompt it is drawing.
    Primary = ffi::SemanticPromptPromptKind::PRIMARY,
    /// A prompt drawn at the right edge of the line, such as zsh's
    /// RPROMPT.
    Right = ffi::SemanticPromptPromptKind::RIGHT,
    /// A prompt at the start of an extra line of a command that spans
    /// several lines.
    Continuation = ffi::SemanticPromptPromptKind::CONTINUATION,
    /// Another prompt for an extra line of input, such as bash's PS2.
    /// Shells differ in whether they report extra lines as continuation or
    /// secondary prompts, so most applications should treat the two the
    /// same.
    Secondary = ffi::SemanticPromptPromptKind::SECONDARY,
}

/// A request from the running program to resize the window.
///
/// Passed to [`Terminal::on_resize_window`].
///
/// New kinds may be added in later versions. Match with a wildcard arm and
/// ignore any kind you don't handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResizeWindow {
    /// Resize the window's text area to a size in rows and columns
    /// (CSI 8 ; rows ; columns t).
    Grid {
        /// Requested height in rows, or `None` to keep the current height.
        rows: Option<NonZeroU16>,
        /// Requested width in columns, or `None` to keep the current width.
        columns: Option<NonZeroU16>,
    },
}

impl ResizeWindow {
    /// Reads a request, or gives `None` for a kind these bindings don't know.
    ///
    /// This takes a pointer rather than the struct by value on purpose: for
    /// a kind without a payload upstream may leave the whole union
    /// unwritten, and copying an entirely uninitialized union relies on
    /// validity rules Rust hasn't settled yet. So we only read the tag, and
    /// then only the union field the tag says is active.
    ///
    /// # Safety
    ///
    /// Caller must guarantee that `raw` is valid for reads and properly
    /// aligned, that its `tag` is initialized, and that the union field named
    /// by `tag` is initialized.
    unsafe fn from_raw(raw: *const ffi::TerminalResizeWindow) -> Option<Self> {
        // SAFETY: Upheld by caller. This reads just the tag field through
        // the raw pointer, without making a reference to the whole struct.
        match unsafe { (*raw).tag } {
            ffi::TerminalResizeWindowTag::GRID => {
                // SAFETY: Upheld by caller; the tag says `grid` is the
                // active union field.
                let ffi::TerminalResizeWindowGrid { rows, columns } = unsafe { (*raw).value.grid };
                // Upstream uses 0 as "keep the current size", which is
                // exactly what `None` means here.
                Some(Self::Grid {
                    rows: NonZeroU16::new(rows),
                    columns: NonZeroU16::new(columns),
                })
            }
            // Upstream may add kinds without an ABI break. We can't read
            // their payload, so there's no Rust value for them.
            _ => None,
        }
    }
}

//---------------------------------------
// Callbacks
//---------------------------------------

/// You might be wondering just what the heck this is.
///
/// Truth to be told, you don't need to understand how it works
/// in order to use it. It does a bunch of voodoo behind the scenes
/// that make sure all the invariants of the C API are upheld, while
/// providing a convenient API for Rust users.
///
/// Each handler is defined in this following format:
/// ```ignore
/// pub fn on_foobar(
///     &mut self,
///     // The corresponding GhosttyTerminalOption
///     tag = FOOBAR,
///
///     // The name of the original function type in C,
///     // along with the extra C parameters and the expected C return type
///     from = TerminalFoobarFn(foo: *const u8, bar: usize) -> bool,
///
///     // The name of mapped Rust function type,
///     // along with the Rust parameters and return type.
///     //
///     // `<'t>` is used to tie the return value to the lifetime of the
///     // terminal. The name is arbitrary - any lifetime marker will do.
///     to = <'t>FoobarFn(&'t [u8]) -> bool,
/// ) |term, func| {
///     // `term` is the terminal and `func` is the Rust callback.
///     // Both names are arbitrary.
///
///     // Convert the raw parameters into Rust types.
///     // This is just to illustrate how.
///     let slice = unsafe { std::slice::from_raw_parts(foo, bar) };
///
///     // Call into user logic and return.
///     func(&terminal, slice)
/// }
/// ```
macro_rules! handlers {
    {
        $(
            $(#[$fmeta:meta])*
            $vis:vis fn $name:ident(
                &mut self,
                tag = $tag:ident,
                from = $rawfnty:ident( $($rfname:ident: $rfty:ty),*$(,)? ) $(-> $rawrty:ty)?,
                $(#[$tmeta:meta])*
                to = $(<$lf:lifetime>)? $fnty:ident( $($fty:ty),*$(,)? ) $(-> $rty:ty)?,
            ) |$t:ident, $func:ident| $block:block
        )*
    } => {
        /// Methods for registering [effect handlers](#effects).
        impl<'alloc, 'cb> $crate::terminal::Terminal<'alloc, 'cb> {$(
            $(#[$fmeta])*
            ///
            /// See [#Effects](Terminal#effects) for more details.
            $vis fn $name(&mut self, f: impl $fnty<'alloc, 'cb>) -> $crate::error::Result<&mut Self> {
                unsafe extern "C" fn callback(
                    t: $crate::ffi::Terminal,
                    ud: *mut std::ffi::c_void,
                    $($rfname: $rfty),*
                ) $(-> $rawrty)? {
                    // SAFETY: USERDATA is the owning raw VTable pointer
                    // returned by Box::into_raw, unchanged by Terminal moves,
                    // before the callback is registered. ghostty invokes
                    // callbacks synchronously from vt_write, reset, resize
                    // and paste (which writes to the pty). All four take
                    // `&mut self`, so the VTable outlives this call and
                    // nothing outside the callbacks touches it meanwhile.
                    // Callbacks only get a `&Terminal`, so they can't reach
                    // any of those entry points.
                    //
                    // Dispatch nests in one place: `ClipboardRead::reply`
                    // writes the answer to the pty right away, so the pty
                    // write callback runs inside the clipboard read callback.
                    // That is still sound, since neither trampoline keeps a
                    // reference into the VTable across the user's closure:
                    // `vtable` is only used to pick out the closure, which
                    // lives in its own box, and different callbacks use
                    // different boxes.
                    let vtable = unsafe { &mut *ud.cast::<VTable<'_, '_>>() };

                    let obj = $crate::alloc::Object::new(t).expect("received null terminal ptr in callback - this is a bug!");
                    // Build a temporary borrowed Terminal view for the callback
                    // without taking ownership of the underlying ghostty terminal.
                    let term = ::core::mem::ManuallyDrop::new($crate::terminal::Terminal::<'_, '_> {
                        inner: obj,
                        vtable: ud.cast(),
                        id: 0,
                    });
                    let $t: &$crate::terminal::Terminal = &term;
                    let $func = vtable.$name.as_deref_mut()
                        .expect("no handler set but callback is still called - this is a bug!");
                    $block
                }

                // SAFETY: Registration has exclusive access to the terminal;
                // no callback is active and the VTable allocation is live.
                unsafe { (*self.vtable).$name = Some(::std::boxed::Box::new(f)); }

                // USERDATA is a raw pointer option: pass the heap allocation
                // itself, not the address of the Box smart pointer field stored
                // inline in Terminal.
                //
                // Reuse the owning pointer, not a pointer derived from a
                // temporary mutable reference into the VTable.
                let userdata = self.vtable.cast::<::std::ffi::c_void>().cast_const();
                self.set_ptr($crate::ffi::TerminalOption::USERDATA, userdata)?;

                // The callback must be coerced into a function *pointer*
                // and not a function *item* (which is a ZST whose address is meaningless).
                // :)
                // Type-check against the generated C callback alias so ABI changes
                // cannot silently pass through the type-erased option setter.
                let _: $crate::ffi::$rawfnty = Some(callback);

                let callback_ptr: unsafe extern "C" fn(
                    $crate::ffi::Terminal,
                    *mut ::std::ffi::c_void,
                    $($rfty),*
                ) $(-> $rawrty)? = callback;

                let result = unsafe {
                    $crate::ffi::ghostty_terminal_set(
                        self.inner.as_raw(),
                        $crate::ffi::TerminalOption::$tag,
                        callback_ptr as *const ::std::ffi::c_void
                    )
                };
                $crate::error::from_result(result)?;
                Ok(self)
            }
        )*}
        $(
            #[doc = concat!(
                "[Effect](Terminal#effects) callback type for [`Terminal::",
                stringify!($name),
                "`](Terminal::",
                stringify!($name),
                ").\n"
            )]
            $(#[$tmeta])*
            pub trait $fnty<'alloc, 'cb>:
                $(for<$lf>)? FnMut(
                    &$($lf)? $crate::terminal::Terminal<'alloc, 'cb>,
                    $($fty),*
                ) $(-> $rty)? + 'cb {}

            impl<'alloc, 'cb, F> $fnty<'alloc, 'cb> for F
            where
                F: $(for<$lf>)? FnMut(
                    &$($lf)? $crate::terminal::Terminal<'alloc, 'cb>,
                    $($fty),*
                ) $(-> $rty)? + 'cb
            {}
        )*

        struct VTable<'alloc, 'cb> {
            $($name: Option<::std::boxed::Box<dyn $fnty<'alloc, 'cb>>>),*
        }

        impl ::core::fmt::Debug for VTable<'_, '_> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                f.write_str("VTable {..}")
            }
        }

        impl ::core::default::Default for VTable<'_, '_> {
            fn default() -> Self {
                Self {
                    $($name: None),*
                }
            }
        }
    };
}

handlers! {
    /// Call the given function when the terminal needs to write data back
    /// to the pty (e.g. in response to a DECRQM query, device status report,
    /// or VT-driven mode 2048 enable).
    pub fn on_pty_write(
        &mut self,
        tag = WRITE_PTY,
        from = TerminalWritePtyFn(ptr: *const u8, len: usize),
        to = <'t>PtyWriteFn(&'t [u8]),
    ) |term, func| {
        // SAFETY: We trust libghostty to return valid memory given we
        // uphold all lifetime invariants (e.g. no `vt_write` calls
        // during this callback, which is guaranteed via the mutable reference).
        let data = unsafe { std::slice::from_raw_parts(ptr, len) };
        func(term, data);
    }

    /// Call the given function when the terminal receives
    /// a BEL character (0x07).
    pub fn on_bell(
        &mut self,
        tag = BELL,
        from = TerminalBellFn(),
        to = BellFn(),
    ) |term, func| {
        func(term);
    }

    /// Call the given function when the running program asks the terminal to
    /// stop updating the screen, and again when it lets the screen update
    /// again. We call the time in between a "render hold".
    ///
    /// Programs use a hold to avoid flicker. A full-screen program usually
    /// redraws in several steps: clear, draw the text, move the cursor. If
    /// the screen is drawn halfway through, the user sees a broken frame. To
    /// prevent that, the program starts a hold, draws everything, and then
    /// releases the hold. The screen should keep showing the last finished
    /// frame the whole time and then switch to the new one all at once.
    ///
    /// Today the only way a program can start a hold is synchronized output
    /// ([`Mode::SYNC_OUTPUT`], DEC private mode 2026). The callback is named
    /// for what the embedder should do rather than for that mode so that
    /// other sources of holds can be added later.
    ///
    /// # When it is called
    ///
    /// With `held` set to `true` when the program sets mode 2026.
    ///
    /// With `held` set to `false` when the hold ends, which happens when:
    ///
    /// - the program resets mode 2026
    /// - the terminal is fully reset, by the program (RIS) or by
    ///   [`Terminal::reset`]
    /// - the terminal is resized with [`Terminal::resize`]
    ///
    /// The two calls always come in pairs. Setting the mode while a hold is
    /// already active does nothing, and neither does resetting it when there
    /// is no hold. Changing the mode yourself with [`Terminal::set_mode`]
    /// never invokes the callback.
    ///
    /// # What to do
    ///
    /// When a hold begins, the terminal contains exactly the frame the
    /// program wants left on screen. Nothing after the start of the hold has
    /// been processed yet, even if more bytes follow in the same
    /// [`Terminal::vt_write`] call. Capture that frame by calling
    /// [`RenderState::update`](crate::RenderState::update) from within the
    /// callback, then stop updating the render state until the hold ends. You
    /// can keep drawing the render state in the meantime. It won't change.
    ///
    /// <div class="warning">
    ///
    /// The callback runs inside an `extern "C"` function, so a panic in it
    /// aborts the process. If the callback updates the render state, don't
    /// keep the render state borrowed (e.g. through a
    /// [`Snapshot`](crate::render::Snapshot) or a
    /// [`RefMut`](std::cell::RefMut)) across [`Terminal::vt_write`],
    /// [`Terminal::reset`] or [`Terminal::resize`], since any of them can
    /// invoke the callback. Use a non-panicking borrow such as
    /// [`RefCell::try_borrow_mut`](std::cell::RefCell::try_borrow_mut)
    /// inside the callback.
    ///
    /// </div>
    ///
    /// ```rust
    /// use std::cell::{Cell, RefCell};
    /// use std::time::{Duration, Instant};
    /// use libghostty_vt::{RenderState, Terminal, terminal::Mode};
    ///
    /// struct Renderer {
    ///     render_state: RefCell<RenderState<'static>>,
    ///     held: Cell<bool>,
    ///     hold_started: Cell<Instant>,
    /// }
    ///
    /// fn draw(r: &Renderer, terminal: &mut Terminal<'static, '_>) -> libghostty_vt::error::Result<()> {
    ///     // Give up on a program that holds the screen for too long.
    ///     if r.held.get() && r.hold_started.get().elapsed() >= Duration::from_secs(1) {
    ///         terminal.set_mode(Mode::SYNC_OUTPUT, false)?;
    ///         r.held.set(false);
    ///     }
    ///
    ///     // During a hold, skip the update and draw the captured frame.
    ///     if !r.held.get() {
    ///         r.render_state.borrow_mut().update(terminal)?;
    ///     }
    ///     // draw_render_state(&r.render_state);
    ///     Ok(())
    /// }
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let renderer = Renderer {
    ///     render_state: RefCell::new(RenderState::new()?),
    ///     held: Cell::new(false),
    ///     hold_started: Cell::new(Instant::now()),
    /// };
    ///
    /// let mut terminal = Terminal::new(80, 24)?;
    /// terminal.on_render_hold(|term, held| {
    ///     if held {
    ///         // Capture the frame the program wants left on screen. Don't
    ///         // panic if that fails, e.g. because the render state is
    ///         // borrowed elsewhere; keep drawing the previous frame instead.
    ///         let _ = renderer
    ///             .render_state
    ///             .try_borrow_mut()
    ///             .map(|mut state| state.update(term).map(|_| ()));
    ///         renderer.hold_started.set(Instant::now());
    ///     }
    ///     renderer.held.set(held);
    /// })?;
    ///
    /// terminal.vt_write(b"\x1b[?2026h");
    /// assert!(renderer.held.get());
    /// draw(&renderer, &mut terminal)?;
    ///
    /// terminal.vt_write(b"\x1b[?2026l");
    /// assert!(!renderer.held.get());
    /// draw(&renderer, &mut terminal)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Timeouts
    ///
    /// The terminal has no clock, so it never ends a hold on its own. A
    /// program that crashes or forgets to release its hold would freeze the
    /// screen forever, so you need a timeout like the one above. One second
    /// is a common choice. When it expires, reset the mode yourself and go
    /// back to updating normally. Because setting the mode again during a
    /// hold does nothing, a program can't keep pushing your deadline back.
    ///
    /// # Why a callback
    ///
    /// You could instead check [`Mode::SYNC_OUTPUT`] before each draw and
    /// skip the update when it is set. That is simpler, but it has two
    /// problems. First, the frame left on screen is whatever you happened to
    /// draw last, which can be older than what the program intended or even
    /// a half-drawn frame. Second, if the program releases a hold and starts
    /// the next one between two of your draws, you never see the mode turn
    /// off and the finished frame in between is lost. A program that draws
    /// continuously can then appear frozen. Capturing the frame when each
    /// hold begins avoids both.
    ///
    /// # Other notes
    ///
    /// You are free to ignore a hold whenever showing live content matters
    /// more, such as when the user scrolls or starts a selection.
    ///
    /// Like every callback, this runs on the thread that called
    /// [`Terminal::vt_write`], [`Terminal::reset`] or [`Terminal::resize`].
    /// Since neither a terminal nor a render state can be sent to another
    /// thread, the update in the callback needs no locking.
    pub fn on_render_hold(
        &mut self,
        tag = RENDER_HOLD,
        from = TerminalRenderHoldFn(held: bool),
        to = RenderHoldFn(bool),
    ) |term, func| {
        func(term, held);
    }

    /// Call the given function when the terminal receives
    /// an ENQ character (0x05).
    pub fn on_enquiry(
        &mut self,
        tag = ENQUIRY,
        from = TerminalEnquiryFn() -> ffi::String,
        to = <'t>EnquiryFn() -> Option<&'t str>,
    ) |term, func| {
        func(term).unwrap_or("").into()
    }

    /// Call the given function when the terminal receives an XTVERSION
    /// query (CSI > q), and respond with the resulting version string
    /// (e.g. "myterm 1.0").
    pub fn on_xtversion(
        &mut self,
        tag = XTVERSION,
        from = TerminalXtversionFn() -> ffi::String,
        to = <'t>XtversionFn() -> Option<&'t str>,
    ) |term, func| {
        func(term).unwrap_or("").into()
    }

    /// Call the given function when the terminal title changes
    /// via escape sequences (e.g. OSC 0 or OSC 2).
    ///
    /// The new title can be queried from the terminal after
    /// the callback returns.
    pub fn on_title_changed(
        &mut self,
        tag = TITLE_CHANGED,
        from = TerminalTitleChangedFn(),
        to = TitleChangedFn(),
    ) |term, func| {
        func(term);
    }

    /// Call the given function when the terminal current working directory
    /// changes via escape sequences (e.g. OSC 7, OSC 9, or OSC 1337).
    ///
    /// The new working directory can be queried from the terminal after
    /// the callback returns.
    pub fn on_pwd_changed(
        &mut self,
        tag = PWD_CHANGED,
        from = TerminalPwdChangedFn(),
        to = PwdChangedFn(),
    ) |term, func| {
        func(term);
    }

    /// Call the given function in response to XTWINOPS size queries
    /// (CSI 14/16/18 t) and when VT input enables in-band size reports (mode
    /// 2048). Return the current terminal geometry, or `None` to suppress the
    /// report.
    pub fn on_size(
        &mut self,
        tag = SIZE,
        from = TerminalSizeFn(out: *mut ffi::SizeReportSize) -> bool,
        to = SizeFn() -> Option<SizeReportSize>,
    ) |term, func| {
        if let Some(size) = func(term) {
            // SAFETY: Out pointer is assumed to be valid.
            unsafe { *out = size };
            true
        } else {
            false
        }
    }

    /// Call the given function in response to a color scheme
    /// device status report query (CSI ? 996 n).
    ///
    /// Return `Some` to report the current color scheme,
    /// or return `None` to silently ignore.
    pub fn on_color_scheme(
        &mut self,
        tag = COLOR_SCHEME,
        from = TerminalColorSchemeFn(out: *mut ffi::ColorScheme::Type) -> bool,
        to = ColorSchemeFn() -> Option<ColorScheme>,
    ) |term, func| {
        if let Some(size) = func(term) {
            // SAFETY: Out pointer is assumed to be valid.
            unsafe { *out = size.into() };
            true
        } else {
            false
        }
    }

    /// Call the given function in response to a device attributes query
    /// (CSI c, CSI > c, or CSI = c).
    ///
    /// Return `Some` with the response data,
    /// or return `None` to silently ignore.
    pub fn on_device_attributes(
        &mut self,
        tag = DEVICE_ATTRIBUTES,
        from = TerminalDeviceAttributesFn(out: *mut ffi::DeviceAttributes) -> bool,
        to = DeviceAttributesFn() -> Option<DeviceAttributes>,
    ) |term, func| {
        if let Some(size) = func(term) {
            // SAFETY: Out pointer is assumed to be valid.
            unsafe { *out = size.into() };
            true
        } else {
            false
        }
    }

    /// Call the given function when the running program performs a clipboard write.
    ///
    /// Protocol details such as OSC 52 selectors, base64 encoding, multipart
    /// chunks, aliases, and terminators are normalized before this callback is
    /// invoked. OSC 52, iTerm2 OSC 1337 Copy, and Kitty clipboard (OSC 5522)
    /// writes therefore use the same callback shape. Without this callback,
    /// clipboard writes are ignored and Kitty clipboard writes are refused
    /// with ENOSYS.
    ///
    /// The embedder may ask for permission to write or perform the write
    /// async, but the callback itself is synchronous and
    /// [`ClipboardWrite::reply`] must be called before it returns. While this
    /// callback is active the VT stream is paused. Returning without a reply
    /// denies the write.
    ///
    /// The request may carry an optional program name requesting the write
    /// and the state of prior permission granted. If
    /// [`ClipboardWrite::can_remember`] is set the reply may set `remember`,
    /// and future requests from this same program will be
    /// [granted](ClipboardWrite::granted) so the embedder can skip permission
    /// requests.
    ///
    /// Clipboard read requests (OSC 52 `?` and OSC 5522 reads) are delivered
    /// to [`Self::on_clipboard_read`] instead.
    pub fn on_clipboard_write(
        &mut self,
        tag = CLIPBOARD_WRITE,
        from = TerminalClipboardWriteFn(
            write: *const ffi::ClipboardWrite
        ),
        to = <'t>ClipboardWriteFn(ClipboardWrite<'t>),
    ) |term, func| {
        // SAFETY: The request is only borrowed for the callback duration,
        // which `ClipboardWrite`'s lifetime enforces.
        func(term, unsafe { ClipboardWrite::from_raw(write) });
    }

    /// Call the given function when the running program requests clipboard
    /// contents via OSC 52 with a `?` payload or a Kitty clipboard (OSC 5522)
    /// read.
    ///
    /// Answering lets the program read the user's clipboard, so the embedder
    /// is expected to mediate consent. Because the read is synchronous, an
    /// embedder that needs to ask the user must block (for example by running
    /// a modal prompt) until it has an answer; the VT stream waits until the
    /// callback returns.
    ///
    /// Answer by calling [`ClipboardRead::reply`] before returning. See
    /// [`ClipboardRead`] for the full contract.
    ///
    /// OSC 5522 requests carry the program's MIME list, name, and password
    /// grant state; a reply that sets `remember` records a session grant so
    /// later requests with the same password arrive with
    /// [`granted`](ClipboardRead::granted) set. Kitty itself serves a request
    /// for only the targets listing ([`list`](ClipboardRead::list) with no
    /// [`mimes`](ClipboardRead::mimes)) without prompting, and embedders are
    /// expected to do the same; the terminal never consults grants for such
    /// requests (`granted` is false and one-time passwords are not consumed).
    ///
    /// Without this callback, OSC 52 read requests are ignored and OSC 5522
    /// reads are refused with EPERM.
    ///
    /// Installing this callback also enables Kitty paste events (mode 5522):
    /// [`Terminal::paste`] sends the program an event instead of the text, and
    /// the program's follow-up read arrives here with `granted` set since the
    /// user already pasted.
    pub fn on_clipboard_read(
        &mut self,
        tag = CLIPBOARD_READ,
        from = TerminalClipboardReadFn(read: *const ffi::ClipboardRead),
        to = <'t>ClipboardReadFn(ClipboardRead<'t>),
    ) |term, func| {
        // SAFETY: The request is only borrowed for the callback duration,
        // which `ClipboardRead`'s lifetime enforces.
        func(term, unsafe { ClipboardRead::from_raw(read) });
    }

    /// Callback invoked when the running program requests a desktop
    /// notification via OSC 9 or OSC 777.
    pub fn on_desktop_notification(
        &mut self,
        tag = DESKTOP_NOTIFICATION,
        from = TerminalDesktopNotificationFn(
            notif: *const ffi::TerminalDesktopNotification
        ),
        to = <'t>DesktopNotificationFn(DesktopNotification<'t>),
    ) |term, func| {
        func(term, unsafe { DesktopNotification::from_raw(notif) });
    }

    /// Call the given function when the running program reports progress
    /// via OSC 9;4.
    pub fn on_progress_report(
        &mut self,
        tag = PROGRESS_REPORT,
        from = TerminalProgressReportFn(
            progress: *const ffi::TerminalProgressReport
        ),
        to = <'t>ProgressReportFn(ProgressReport<'t>),
    ) |term, func| {
        func(term, unsafe { ProgressReport::from_raw(progress) });
    }

    /// Call the given function when the running program sends a program
    /// status report via OSC 7501.
    ///
    /// Called synchronously each time the running program sends a valid
    /// report. See [`ProgramStatus`] for what a report contains.
    ///
    /// The terminal doesn't store reports, so your application keeps them.
    /// To follow the specification, keep one record per id. A report with an
    /// empty id is about the root record, the program itself. The records
    /// follow these rules:
    ///
    /// - A report replaces its record completely. A value the report leaves
    ///   out is gone from the record afterwards. It doesn't keep its old
    ///   value.
    /// - A [`ProgramStatusState::Clear`] report removes the record with its
    ///   id and every record beneath it, so clearing `"build"` also removes
    ///   `"build/test"`. A clear report with an empty id removes every
    ///   record.
    /// - A record without an app takes it from its nearest ancestor that has
    ///   one. If the root record has app `"deploy"` and the record
    ///   `"us-east"` has none, show `"us-east"` with app `"deploy"` too.
    /// - When a new shell prompt starts ([`SemanticPromptKind::PromptStart`]
    ///   from the [`Terminal::on_semantic_prompt`] callback) or the program
    ///   running in the terminal exits, remove `working` and
    ///   `blocked` records. You may remove `idle` records too. Keep `done`
    ///   and `error` records until the user has seen them, for example until
    ///   they next focus the terminal.
    /// - Keep at most 256 records, and allow at least 64. When a new record
    ///   would go over your limit, remove the one that was updated longest
    ///   ago.
    ///
    /// A full reset (RIS, `ESC c`) removes every record. When that happens,
    /// the terminal calls this with a [`ProgramStatusState::Clear`] report
    /// and an empty id. This only happens when `ESC c` is processed by
    /// [`Terminal::vt_write`]. [`Terminal::reset`] doesn't call this, so
    /// clear your records yourself after calling it.
    ///
    /// [`title`](ProgramStatus::title) and
    /// [`message`](ProgramStatus::message) are already decoded and contain
    /// no control characters, but they are still untrusted text from the
    /// program. Don't treat them as markup. If you show them outside the
    /// terminal, such as in a tab or a notification, remove invisible
    /// formatting characters like text direction overrides, and say which
    /// terminal the text came from so a program can't pretend to be one
    /// running elsewhere.
    ///
    /// Example, where `records` stands in for your application's own
    /// storage:
    ///
    /// ```rust
    /// use std::cell::RefCell;
    /// use std::collections::HashMap;
    /// use libghostty_vt::{Terminal, terminal::ProgramStatusState};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // One record per id. This one keeps only the state and message.
    /// let records = RefCell::new(HashMap::<Vec<u8>, (ProgramStatusState, Vec<u8>)>::new());
    /// # let pty = RefCell::new(Vec::new());
    ///
    /// let mut terminal = Terminal::new(80, 24)?;
    /// // Set a pty write callback too, so programs that check for support
    /// // get a reply.
    /// terminal
    ///     .on_pty_write(|_term, bytes| {
    ///         // Write `bytes` back to the pty.
    /// #       pty.borrow_mut().extend_from_slice(bytes);
    ///     })?
    ///     .on_program_status(|_term, report| {
    ///         // A panic in the callback aborts the process, so ignore a
    ///         // state these bindings don't know instead of unwrapping.
    ///         let Ok(state) = report.state() else { return };
    ///         let mut records = records.borrow_mut();
    ///
    ///         if state == ProgramStatusState::Clear {
    ///             // Remove this record and every record beneath it. An empty
    ///             // id removes every record.
    ///             let id = report.id();
    ///             records.retain(|key, _| {
    ///                 let beneath = key
    ///                     .strip_prefix(id)
    ///                     .is_some_and(|rest| rest.is_empty() || rest.starts_with(b"/"));
    ///                 !(id.is_empty() || beneath)
    ///             });
    ///             return;
    ///         }
    ///
    ///         // Replace the whole record. The strings are only valid during
    ///         // this call, so copy them.
    ///         records.insert(report.id().to_vec(), (state, report.message().to_vec()));
    ///     })?;
    /// # let ids = || {
    /// #     let mut ids: Vec<_> = records.borrow().keys().cloned().collect();
    /// #     ids.sort();
    /// #     ids
    /// # };
    ///
    /// // A root record and a few beneath it. "QnVpbGRpbmc=" is "Building".
    /// terminal.vt_write(b"\x1b]7501;state=working:app=cargo:msg=QnVpbGRpbmc=\x1b\\");
    /// terminal.vt_write(b"\x1b]7501;state=working:id=build\x1b\\");
    /// terminal.vt_write(b"\x1b]7501;state=blocked:kind=question:id=build/test\x1b\\");
    /// terminal.vt_write(b"\x1b]7501;state=done:id=buildx\x1b\\");
    /// # assert_eq!(ids(), [&b""[..], b"build", b"build/test", b"buildx"]);
    /// # assert_eq!(
    /// #     records.borrow()[&b""[..]],
    /// #     (ProgramStatusState::Working, b"Building".to_vec()),
    /// # );
    ///
    /// // A report replaces its record completely, so the message is gone.
    /// terminal.vt_write(b"\x1b]7501;state=done\x1b\\");
    /// # assert_eq!(records.borrow()[&b""[..]], (ProgramStatusState::Done, Vec::new()));
    ///
    /// // Clearing "build" also removes "build/test", but not "buildx".
    /// terminal.vt_write(b"\x1b]7501;state=clear:id=build\x1b\\");
    /// # assert_eq!(ids(), [&b""[..], b"buildx"]);
    ///
    /// // A program checking for support gets a reply through the pty.
    /// terminal.vt_write(b"\x1b]7501;?\x1b\\");
    /// # assert!(pty.borrow().starts_with(b"\x1b]7501;?:states="));
    ///
    /// // A full reset removes every record.
    /// terminal.vt_write(b"\x1bc");
    /// # assert!(records.borrow().is_empty());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Programs may ask whether the terminal supports the protocol by
    /// sending `OSC 7501 ; ?`. While this callback is set, the terminal
    /// answers through [`Terminal::on_pty_write`] with `?` followed by the
    /// states and kinds it accepts:
    ///
    /// ```text
    /// program:  ESC ] 7501 ; ? ST
    /// terminal: ESC ] 7501 ; ?:states=idle,working,done,blocked,error:kinds=permission,question,auth ST
    /// ```
    ///
    /// While this callback is not set, the query gets no reply, so a program
    /// that asks sees the protocol as unsupported. Asking is optional, so
    /// programs may send reports anyway. Those are dropped. Set a pty write
    /// callback too, or programs never see the reply.
    pub fn on_program_status(
        &mut self,
        tag = PROGRAM_STATUS,
        from = TerminalProgramStatusFn(
            report: *const ffi::TerminalProgramStatus
        ),
        to = <'t>ProgramStatusFn(ProgramStatus<'t>),
    ) |term, func| {
        // SAFETY: The report and its strings are only valid during the
        // callback, which `ProgramStatus`'s lifetime enforces.
        func(term, unsafe { ProgramStatus::from_raw(report) });
    }

    /// Call the given function when the shell reports a step of a command:
    /// a prompt starts, input starts, output starts, or the command ends.
    ///
    /// Each command goes through four steps, in this order: the prompt
    /// starts, input starts, output starts, and the command ends. Then the
    /// next prompt starts. See [`SemanticPromptKind`].
    ///
    /// Shells differ in what they report. Many don't send the command line
    /// or the exit code, and some skip steps, so handle each event on its
    /// own instead of expecting a strict order. A shell may also start the
    /// same prompt more than once, for example when it redraws the prompt
    /// after a resize, so treat a repeated
    /// [`SemanticPromptKind::PromptStart`] as harmless.
    ///
    /// The terminal has already updated its screen when this is called. A
    /// sequence the terminal rejects as malformed is never reported.
    ///
    /// The event and its strings are only valid during the call.
    ///
    /// Example, logging each command's result:
    ///
    /// ```rust
    /// use std::cell::RefCell;
    /// use libghostty_vt::{Terminal, terminal::SemanticPromptKind};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let results = RefCell::new(Vec::new());
    ///
    /// let mut terminal = Terminal::new(80, 24)?;
    /// terminal.on_semantic_prompt(|_term, event| {
    ///     // A panic in the callback aborts the process, so ignore a kind
    ///     // these bindings don't know instead of unwrapping.
    ///     if let Ok(SemanticPromptKind::CommandEnd) = event.kind() {
    ///         // Many shells don't report the exit code, so this may be
    ///         // `None` even for a command that ran.
    ///         results.borrow_mut().push(event.exit_code());
    ///     }
    /// })?;
    ///
    /// // The shell reports a command that exited with 1, then one that
    /// // didn't say how it went.
    /// terminal.vt_write(b"\x1b]133;D;1\x07");
    /// terminal.vt_write(b"\x1b]133;D\x07");
    /// assert_eq!(*results.borrow(), [Some(1), None]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn on_semantic_prompt(
        &mut self,
        tag = SEMANTIC_PROMPT,
        from = TerminalSemanticPromptFn(
            event: *const ffi::TerminalSemanticPrompt
        ),
        to = <'t>SemanticPromptFn(SemanticPrompt<'t>),
    ) |term, func| {
        // SAFETY: The event and its strings are only valid during the
        // callback, which `SemanticPrompt`'s lifetime enforces.
        func(term, unsafe { SemanticPrompt::from_raw(event) });
    }

    /// Call the given function when the running program asks to resize the
    /// window, such as with CSI 8 ; rows ; columns t.
    ///
    /// More kinds of requests may be reported in later versions; see
    /// [`ResizeWindow`]. Requests of a kind these bindings don't know are not
    /// passed to the closure. The terminal is not resized by this: the embedder
    /// decides whether to allow the request, and if so resizes its window and
    /// the terminal (with [`Terminal::resize`]). Letting any program resize
    /// the window is a nuisance at best, so consider making this opt-in for
    /// users.
    ///
    /// Without this callback, resize requests are ignored.
    ///
    /// ```rust
    /// use std::{cell::Cell, num::NonZeroU16};
    /// use libghostty_vt::{Terminal, terminal::ResizeWindow};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let requested = Cell::new(None);
    ///
    /// let mut terminal = Terminal::new(80, 24)?;
    /// terminal.on_resize_window(|_term, request| match request {
    ///     ResizeWindow::Grid { rows, columns } => requested.set(Some((rows, columns))),
    ///     // Ignore kinds of requests you don't handle.
    ///     _ => {}
    /// })?;
    ///
    /// terminal.vt_write(b"\x1b[8;30;100t");
    /// # assert_eq!(requested.get(), Some((NonZeroU16::new(30), NonZeroU16::new(100))));
    ///
    /// // The terminal itself keeps its size. Apply the request yourself
    /// // (`None` means keep the current size of that dimension).
    /// # assert_eq!((terminal.cols()?, terminal.rows()?), (80, 24));
    /// if let Some((rows, columns)) = requested.take() {
    ///     let rows = rows.map_or(terminal.rows()?, NonZeroU16::get);
    ///     let cols = columns.map_or(terminal.cols()?, NonZeroU16::get);
    ///     terminal.resize(cols, rows, 8, 16)?;
    /// }
    /// # assert_eq!((terminal.cols()?, terminal.rows()?), (100, 30));
    /// # Ok(())
    /// # }
    /// ```
    pub fn on_resize_window(
        &mut self,
        tag = RESIZE_WINDOW,
        from = TerminalResizeWindowFn(request: *const ffi::TerminalResizeWindow),
        to = ResizeWindowFn(ResizeWindow),
    ) |term, func| {
        // SAFETY: We trust libghostty to give us a valid, non-NULL request
        // that lives for the callback duration, with the tag and its
        // matching payload written. For a payload-less kind the union may be
        // left unwritten, which is why `from_raw` checks the tag before
        // reading anything else, instead of us copying the struct out.
        let request = unsafe { ResizeWindow::from_raw(request) };
        // Upstream asks callbacks to ignore tags they don't handle. A tag
        // these bindings don't know has no Rust value, so the closure isn't
        // called for it.
        if let Some(request) = request {
            func(term, request);
        }
    }

    /// Called once for each complete sequence that libghostty-vt does not
    /// implement. Match on [`UnknownSequence`] first, because more kinds of
    /// sequences may be reported in later versions.
    ///
    /// Sequences of a kind these bindings don't know, and OSC sequences with
    /// an [`osc::Terminator`] these bindings don't know, are not passed to the
    /// closure.
    ///
    /// These are not reported:
    ///
    /// - Sequences the program cancelled partway through with CAN or SUB.
    /// - Sequences libghostty-vt implements, even when their contents are
    ///   malformed.
    /// - Supported protocols that the embedder turned off.
    ///
    /// The callback runs during [`Self::vt_write`]. It may write a reply to
    /// the pty, and that reply stays in order with the terminal's own
    /// replies. It must not call [`Self::vt_write`] on the same terminal.
    ///
    /// Nothing is reported until [`Self::set_unknown_max_bytes`] is also set
    /// to a nonzero value. Installing the callback by itself keeps no sequence
    /// data and allocates no sequence-capture buffers.
    ///
    /// For OSC, the content is everything between `ESC ]` and the terminator,
    /// including the number that identifies the sequence. As an example,
    /// suppose your application invents its own OSC 7400 so that programs can
    /// report their status. If a program writes `ESC ] 7400;status=busy BEL`,
    /// the callback receives the content `7400;status=busy` and the
    /// terminator [`osc::Terminator::Bel`]. Match on the number followed by
    /// `;`, so that `7400;` does not also match an unrelated `74000;`
    /// sequence.
    ///
    /// Only numbers libghostty-vt does not recognize are reported. A sequence
    /// that uses a number it does implement, such as OSC 2 for the window
    /// title, is never reported, even when its contents are malformed.
    ///
    /// Use the terminator from the request in your reply, since that is what
    /// the program expects.
    ///
    /// ```rust
    /// use std::cell::RefCell;
    /// use libghostty_vt::{Terminal, terminal::UnknownSequence};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let statuses = RefCell::new(Vec::new());
    ///
    /// let mut terminal = Terminal::new(80, 24)?;
    /// terminal
    ///     .on_unknown_sequence(|_term, sequence| {
    ///         let UnknownSequence::Osc { content, truncated, .. } = sequence else {
    ///             return;
    ///         };
    ///         // This protocol needs the whole sequence, so skip cut-off ones.
    ///         if truncated {
    ///             return;
    ///         }
    ///         // Only handle OSC 7400. Everything else is ignored.
    ///         if let Some(status) = content.strip_prefix(b"7400;") {
    ///             // The content is only valid during this call. Copy it if you need it
    ///             // later.
    ///             statuses.borrow_mut().push(status.to_vec());
    ///         }
    ///     })?
    ///     // Keep up to 4 KiB of each unknown sequence and report them.
    ///     .set_unknown_max_bytes(4096)?;
    ///
    /// terminal.vt_write(b"\x1b]7400;status=busy\x07");
    /// assert_eq!(*statuses.borrow(), [b"status=busy".to_vec()]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn on_unknown_sequence(
        &mut self,
        tag = UNKNOWN_SEQUENCE,
        from = TerminalUnknownSequenceFn(sequence: *const ffi::TerminalUnknownSequence),
        to = <'t>UnknownSequenceFn(UnknownSequence<'t>),
    ) |term, func| {
        // SAFETY: libghostty passes a valid, aligned sequence for the
        // callback duration, with the tag and the union field it names
        // written (`lib.TaggedUnion.cval`). Upstream documents the content
        // as valid only for the callback duration; the higher-ranked `'t` on
        // `UnknownSequenceFn` stops the closure from keeping it.
        if let Some(sequence) = unsafe { UnknownSequence::from_raw(sequence) } {
            func(term, sequence);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderState;
    use crate::render::CursorVisualStyle;
    use std::cell::{Cell, RefCell};
    use std::mem::ManuallyDrop;

    /// What a clipboard read callback observed about its request.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct SeenRead {
        location: Option<ClipboardLocation>,
        mimes: Vec<Vec<u8>>,
        list: bool,
        name: Vec<u8>,
        granted: bool,
        can_remember: bool,
    }

    /// Feed `input` to a terminal whose clipboard read callback records the
    /// request and hands it to `answer`, returning what was seen and what
    /// the terminal wrote back to the pty.
    ///
    /// Assertions happen outside the callback: a panic inside it would
    /// abort the whole test binary instead of failing the test.
    fn clipboard_read(
        input: &[u8],
        answer: impl Fn(ClipboardRead<'_>),
    ) -> (Vec<SeenRead>, Vec<u8>) {
        let seen = RefCell::new(Vec::new());
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal
            .on_clipboard_read(|_term, request| {
                seen.borrow_mut().push(SeenRead {
                    location: Some(request.location()),
                    mimes: request.mimes().map(<[u8]>::to_vec).collect(),
                    list: request.list(),
                    name: request.name().to_vec(),
                    granted: request.granted(),
                    can_remember: request.can_remember(),
                });
                answer(request);
            })
            .expect("callback should register");
        terminal.vt_write(input);
        drop(terminal);
        (seen.into_inner(), output.into_inner())
    }

    #[test]
    fn scroll_viewport_delta_prompt() {
        // Same transcript as upstream's `scroll_viewport delta_prompt` test:
        // prompts start on rows 0, 3, 6 and 9 of a 10-row history, and with
        // 2 visible rows the active area starts at row 8.
        let mut terminal = Terminal::new(80, 2).expect("terminal should initialize");
        terminal.vt_write(
            b"\x1b]133;P;k=i\x07p1\r\n\
              \x1b]133;B\x07cmd1\r\n\
              \x1b]133;C\x07out1\r\n\
              \x1b]133;P;k=i\x07p2\r\n\
              \x1b]133;B\x07cmd2\r\n\
              \x1b]133;C\x07out2\r\n\
              \x1b]133;P;k=i\x07p3\r\n\
              \x1b]133;B\x07cmd3\r\n\
              \x1b]133;C\x07out3\r\n\
              \x1b]133;P;k=i\x07p4",
        );
        let mut scroll = |scroll| {
            terminal.scroll_viewport(scroll);
            terminal.scrollbar().expect("scrollbar should exist").offset
        };

        // First, upstream's exact sequence.
        //
        // Counting is relative to the viewport top (row 8), so going backward
        // from the active area lands on p3, then p2. Forward returns to p3.
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(-1)), 6);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(-1)), 3);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(1)), 6);

        // Over-asking stops at the last matching prompt: p1 going backward
        // (from p3, so this walks past p2), and p4 going forward, which lies
        // in the active area and so is clamped to it. Zero is then a no-op.
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(isize::MIN)), 0);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(isize::MAX)), 8);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(0)), 8);

        // Extra steps beyond upstream's test: step back from the active area
        // to p3, check zero is a no-op away from the active area too, then
        // over-ask with ordinary counts in both directions.
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(-1)), 6);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(0)), 6);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(-100)), 0);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(100)), 8);
    }

    #[test]
    fn osc52_clipboard_read() {
        let (seen, output) = clipboard_read(b"\x1b]52;c;?\x1b\\", |request| {
            request.reply(
                Ok(&[ClipboardReplyContent::new("text/plain", b"hello")]),
                &[],
                false,
            );
        });
        assert_eq!(
            seen,
            [SeenRead {
                location: Some(ClipboardLocation::Standard),
                mimes: vec![b"text/plain".to_vec()],
                ..SeenRead::default()
            }]
        );
        assert_eq!(output, b"\x1b]52;c;aGVsbG8=\x1b\\");

        // Errors and missing replies both answer with an empty clipboard.
        let (_, output) = clipboard_read(b"\x1b]52;c;?\x1b\\", |request| {
            request.reply(Err(ClipboardReadError::Denied), &[], false);
        });
        assert_eq!(output, b"\x1b]52;c;\x1b\\");
        let (_, output) = clipboard_read(b"\x1b]52;c;?\x1b\\", |_request| {});
        assert_eq!(output, b"\x1b]52;c;\x1b\\");
    }

    #[test]
    fn osc5522_clipboard_read() {
        // Requests text/plain plus the listing ("."), from a program named
        // "app" without a password.
        let (seen, output) = clipboard_read(
            b"\x1b]5522;type=read:id=r1:name=YXBw;dGV4dC9wbGFpbiAu\x1b\\",
            |request| {
                request.reply(
                    Ok(&[
                        ClipboardReplyContent::new("text/plain", b"hello"),
                        // Not requested, so it must be ignored.
                        ClipboardReplyContent::new("image/png", b"png"),
                    ]),
                    &[
                        ClipboardMime::new("text/plain"),
                        ClipboardMime::new("image/png"),
                    ],
                    false,
                );
            },
        );
        assert_eq!(
            seen,
            [SeenRead {
                location: Some(ClipboardLocation::Standard),
                mimes: vec![b"text/plain".to_vec()],
                list: true,
                name: b"app".to_vec(),
                ..SeenRead::default()
            }]
        );
        assert_eq!(
            output,
            concat!(
                "\x1b]5522;type=read:status=OK:id=r1\x1b\\",
                // "text/plain image/png\n"
                "\x1b]5522;type=read:status=DATA:id=r1:mime=Lg==;dGV4dC9wbGFpbiBpbWFnZS9wbmcK\x1b\\",
                "\x1b]5522;type=read:status=DATA:id=r1:mime=dGV4dC9wbGFpbg==;aGVsbG8=\x1b\\",
                "\x1b]5522;type=read:status=DONE:id=r1\x1b\\",
            )
            .as_bytes()
        );

        // Errors map to protocol statuses, and a missing reply is EPERM.
        let (_, output) = clipboard_read(
            b"\x1b]5522;type=read:id=r2;dGV4dC9wbGFpbg==\x1b\\",
            |request| request.reply(Err(ClipboardReadError::Busy), &[], false),
        );
        assert_eq!(output, b"\x1b]5522;type=read:status=EBUSY:id=r2\x1b\\");
        let (_, output) = clipboard_read(
            b"\x1b]5522;type=read:id=r3;dGV4dC9wbGFpbg==\x1b\\",
            |_request| {},
        );
        assert_eq!(output, b"\x1b]5522;type=read:status=EPERM:id=r3\x1b\\");
    }

    #[test]
    fn osc5522_clipboard_read_remembers_password_grants() {
        // Two reads from "app" with the same password ("secret"): remembering
        // the first grant makes the second arrive already granted. Per the
        // spec, a password without a name is no password.
        let read = "\x1b]5522;type=read:id=a:name=YXBw:pw=c2VjcmV0;dGV4dC9wbGFpbg==\x1b\\";
        let (seen, _) = clipboard_read(read.repeat(2).as_bytes(), |request| {
            request.reply(
                Ok(&[ClipboardReplyContent::new("text/plain", b"hi")]),
                &[],
                true,
            );
        });
        let grants: Vec<_> = seen.iter().map(|s| (s.can_remember, s.granted)).collect();
        assert_eq!(grants, [(true, false), (true, true)]);
    }

    /// Feed `input` to a terminal configured by `setup`, returning what it
    /// wrote back to the pty.
    fn pty_output(setup: impl FnOnce(&mut Terminal<'_, '_>), input: &[u8]) -> Vec<u8> {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        setup(&mut terminal);
        terminal.vt_write(input);
        drop(terminal);
        output.into_inner()
    }

    #[test]
    fn vt_write_until_ground_stops_at_ground() {
        let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
        // Already at ground: nothing is consumed.
        assert!(terminal.is_vt_ground().unwrap());
        assert_eq!(terminal.vt_write_until_ground(b"hello").unwrap(), Some(0));
        assert_eq!(terminal.cursor_x().unwrap(), 0);

        // An incomplete CSI sequence is only finished, not followed.
        terminal.vt_write(b"\x1b[");
        assert!(!terminal.is_vt_ground().unwrap());
        assert_eq!(terminal.vt_write_until_ground(b"31").unwrap(), None);
        // The count includes the byte that reaches ground.
        assert_eq!(terminal.vt_write_until_ground(b"mhello").unwrap(), Some(1));
        assert!(terminal.is_vt_ground().unwrap());
        assert_eq!(terminal.cursor_x().unwrap(), 0);

        // The same applies to an incomplete UTF-8 sequence ("€" is E2 82 AC).
        terminal.vt_write(b"\xe2");
        assert!(!terminal.is_vt_ground().unwrap());
        assert_eq!(
            terminal.vt_write_until_ground(b"\x82\xac!").unwrap(),
            Some(2)
        );
        assert_eq!(terminal.cursor_x().unwrap(), 1);
    }

    #[test]
    fn cursor_at_prompt_follows_semantic_prompts() {
        let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
        assert!(!terminal.is_cursor_at_prompt().unwrap());
        terminal.vt_write(b"\x1b]133;A\x1b\\");
        assert!(terminal.is_cursor_at_prompt().unwrap());
        // The alternate screen never counts as a prompt.
        terminal.vt_write(b"\x1b[?1049h");
        assert!(!terminal.is_cursor_at_prompt().unwrap());
    }

    #[test]
    fn terminfo_name_answers_xtgettcap() {
        // XTGETTCAP query for "TN" (hex 544e).
        let query = b"\x1bP+q544e\x1b\\";
        // Unset names are not reported at all.
        assert_eq!(pty_output(|_| {}, query), b"");
        assert_eq!(
            pty_output(
                |terminal| {
                    terminal.set_terminfo_name("xterm-256color").unwrap();
                },
                query
            ),
            // "xterm-256color" in hex.
            b"\x1bP1+r544E=787465726D2D323536636F6C6F72\x1b\\"
        );
        // An empty name clears it again.
        assert_eq!(
            pty_output(
                |terminal| {
                    terminal.set_terminfo_name("xterm-256color").unwrap();
                    terminal.set_terminfo_name("").unwrap();
                },
                query
            ),
            b""
        );

        let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
        assert!(matches!(
            terminal.set_terminfo_name(&"x".repeat(129)),
            Err(Error::InvalidValue)
        ));
    }

    #[test]
    fn clipboard_write_max_bytes_bounds_kitty_writes() {
        let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
        let default = terminal.clipboard_write_max_bytes().unwrap();
        assert_eq!(default, 64 * 1024 * 1024);
        terminal.set_clipboard_write_max_bytes(Some(4)).unwrap();
        assert_eq!(terminal.clipboard_write_max_bytes().unwrap(), 4);
        terminal.set_clipboard_write_max_bytes(None).unwrap();
        assert_eq!(terminal.clipboard_write_max_bytes().unwrap(), default);

        // A 5-byte ("Hello") OSC 5522 write transaction.
        let write = concat!(
            "\x1b]5522;type=write:id=c1\x1b\\",
            "\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;SGVsbA==\x1b\\",
            "\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;bw==\x1b\\",
            "\x1b]5522;type=wdata\x1b\\",
        );
        let run = |limit| {
            let written = RefCell::new(Vec::new());
            let output = RefCell::new(Vec::new());
            let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
            terminal
                .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
                .unwrap()
                .on_clipboard_write(|_term, request| {
                    written
                        .borrow_mut()
                        .extend(request.contents().map(|c| c.data.to_vec()));
                    request.reply(Ok(()), false);
                })
                .unwrap()
                .set_clipboard_write_max_bytes(Some(limit))
                .unwrap();
            terminal.vt_write(write.as_bytes());
            drop(terminal);
            (output.into_inner(), written.into_inner())
        };

        // Exceeding the limit fails the transaction and never reaches the callback.
        let (output, written) = run(4);
        assert_eq!(output, b"\x1b]5522;type=write:status=EFBIG:id=c1\x1b\\");
        assert!(written.is_empty());

        let (output, written) = run(5);
        assert_eq!(output, b"\x1b]5522;type=write:status=DONE:id=c1\x1b\\");
        assert_eq!(written, [b"Hello".to_vec()]);
    }

    #[test]
    fn unknown_sequences_are_reported() {
        use osc::Terminator::{Bel, St};

        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal
            .on_unknown_sequence(|_term, sequence| {
                // Record rather than assert: a panic in a callback aborts the
                // whole test binary.
                seen.borrow_mut().push(match sequence {
                    UnknownSequence::Apc {
                        content, truncated, ..
                    } => (content.to_vec(), truncated, None),
                    UnknownSequence::Osc {
                        content,
                        truncated,
                        terminator,
                        ..
                    } => (content.to_vec(), truncated, Some(terminator)),
                });
            })
            .unwrap();
        // Registration alone leaves reporting disabled.
        terminal.vt_write(b"\x1b_unknown\x1b\\");
        terminal.set_unknown_max_bytes(4).unwrap();
        terminal.vt_write(b"\x1b_unknown\x1b\\");
        terminal.vt_write(b"\x1b]7400;status=busy\x07");
        terminal.set_unknown_max_bytes(64).unwrap();
        terminal.vt_write(b"\x1b_unknown\x1b\\");
        terminal.vt_write(b"\x1b]7400;status=busy\x1b\\");
        // Numbers libghostty-vt implements are never reported, even with no
        // callback for them: OSC 7501 is program status, OSC 2 the title.
        terminal.vt_write(b"\x1b]7501;state=working\x1b\\");
        terminal.vt_write(b"\x1b]2;title\x07");
        drop(terminal);
        assert_eq!(
            seen.into_inner(),
            [
                (b"unkn".to_vec(), true, None),
                (b"7400".to_vec(), true, Some(Bel)),
                (b"unknown".to_vec(), false, None),
                (b"7400;status=busy".to_vec(), false, Some(St)),
            ]
        );
    }

    #[test]
    fn osc5522_clipboard_read_targets_only() {
        // A read for only the targets listing (payload "." = "Lg==") carries
        // no MIME types, so libghostty hands the callback `mimes = NULL,
        // mimes_len = 0`. The first read records a grant for the password;
        // the listing-only read must still arrive ungranted (it is
        // prompt-exempt and never consults grants), and the follow-up data
        // read still sees the grant.
        let remember = "\x1b]5522;type=read:id=a:name=YXBw:pw=c2VjcmV0;dGV4dC9wbGFpbg==\x1b\\";
        let list = "\x1b]5522;type=read:id=l:name=YXBw:pw=c2VjcmV0;Lg==\x1b\\";
        let input = [remember, list, remember].concat();
        let (seen, output) = clipboard_read(input.as_bytes(), |request| {
            // Empty slices hand C dangling non-null pointers with length 0.
            request.reply(Ok(&[]), &[], true);
        });
        let observed: Vec<_> = seen
            .iter()
            .map(|s| (s.mimes.len(), s.list, s.granted))
            .collect();
        assert_eq!(
            observed,
            [(1, false, false), (0, true, false), (1, false, true)]
        );

        // An empty listing is a DATA packet with no payload.
        let listing = concat!(
            "\x1b]5522;type=read:status=OK:id=l\x1b\\",
            "\x1b]5522;type=read:status=DATA:id=l:mime=Lg==\x1b\\",
            "\x1b]5522;type=read:status=DONE:id=l\x1b\\",
        );
        let output = String::from_utf8(output).expect("responses are ASCII");
        assert!(output.contains(listing), "{output:?}");
    }

    #[test]
    fn clipboard_read_empty_reply_contents() {
        // Empty MIME and data strings hand C dangling non-null pointers with
        // length 0. An empty representation produces no DATA packets, which
        // is how the protocol reports an unavailable type.
        let (_, output) = clipboard_read(
            b"\x1b]5522;type=read:id=e;dGV4dC9wbGFpbg==\x1b\\",
            |request| {
                request.reply(
                    Ok(&[
                        ClipboardReplyContent::new("", b""),
                        ClipboardReplyContent::new("text/plain", b""),
                    ]),
                    &[ClipboardMime::new("")],
                    false,
                );
            },
        );
        assert_eq!(
            output,
            concat!(
                "\x1b]5522;type=read:status=OK:id=e\x1b\\",
                "\x1b]5522;type=read:status=DONE:id=e\x1b\\",
            )
            .as_bytes()
        );

        // OSC 52 has no text representation to use, so the clipboard is empty.
        let (_, output) = clipboard_read(b"\x1b]52;c;?\x1b\\", |request| {
            request.reply(Ok(&[ClipboardReplyContent::new("", b"")]), &[], false);
        });
        assert_eq!(output, b"\x1b]52;c;\x1b\\");
    }

    #[test]
    fn osc5522_clipboard_read_errors() {
        for (error, status) in [
            (ClipboardReadError::Denied, "EPERM"),
            (ClipboardReadError::Unsupported, "ENOSYS"),
            (ClipboardReadError::Busy, "EBUSY"),
            (ClipboardReadError::IoError, "EIO"),
        ] {
            let (_, output) = clipboard_read(
                b"\x1b]5522;type=read:id=x;dGV4dC9wbGFpbg==\x1b\\",
                |request| request.reply(Err(error), &[], false),
            );
            let expected = format!("\x1b]5522;type=read:status={status}:id=x\x1b\\");
            assert_eq!(output, expected.as_bytes(), "{error:?}");
        }
    }

    #[test]
    fn clipboard_read_primary_location() {
        let reply = |request: ClipboardRead<'_>| {
            request.reply(
                Ok(&[ClipboardReplyContent::new("text/plain", b"hello")]),
                &[],
                false,
            );
        };

        let (seen, output) = clipboard_read(b"\x1b]52;p;?\x1b\\", reply);
        assert_eq!(seen[0].location, Some(ClipboardLocation::Primary));
        assert_eq!(output, b"\x1b]52;p;aGVsbG8=\x1b\\");

        let (seen, output) = clipboard_read(
            b"\x1b]5522;type=read:loc=primary:id=p;dGV4dC9wbGFpbg==\x1b\\",
            reply,
        );
        assert_eq!(seen[0].location, Some(ClipboardLocation::Primary));
        assert_eq!(
            output,
            concat!(
                "\x1b]5522;type=read:status=OK:loc=primary:id=p\x1b\\",
                "\x1b]5522;type=read:status=DATA:id=p:mime=dGV4dC9wbGFpbg==;aGVsbG8=\x1b\\",
                "\x1b]5522;type=read:status=DONE:id=p\x1b\\",
            )
            .as_bytes()
        );
    }

    #[test]
    fn resize_pull_scrollback_controls_growing_rows() {
        // Apply `configure`, fill a 5-row terminal past its height so rows
        // land in scrollback and the cursor sits on the bottom row, then grow
        // it to 8 rows and report where the cursor ended up.
        fn cursor_row_after_growing(
            configure: impl FnOnce(&mut Terminal<'static, 'static>),
        ) -> u16 {
            let mut terminal = Terminal::new(10, 5).expect("terminal should initialize");
            configure(&mut terminal);
            terminal.vt_write(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8");
            assert_eq!(terminal.cursor_y().unwrap(), 4);
            terminal
                .resize(10, 8, 8, 16)
                .expect("resize should succeed");
            terminal.cursor_y().unwrap()
        }
        fn set(terminal: &mut Terminal<'static, 'static>, pull: Option<bool>) {
            terminal
                .set_resize_pull_scrollback(pull)
                .expect("option should be settable");
        }

        // Pulling scrollback back in moves the cursor's line down with it.
        // That is the default, both when never set and when set explicitly.
        assert_eq!(cursor_row_after_growing(|_| {}), 7);
        assert_eq!(cursor_row_after_growing(|t| set(t, Some(true))), 7);
        // Otherwise blank rows are appended below and the cursor stays put.
        assert_eq!(cursor_row_after_growing(|t| set(t, Some(false))), 4);
        // `None` has to actively restore the default, not just leave the
        // previous value in place.
        assert_eq!(
            cursor_row_after_growing(|t| {
                set(t, Some(false));
                set(t, None);
            }),
            7
        );
        // The setting survives a full reset, whether the program sends RIS
        // or the embedder resets the terminal.
        assert_eq!(
            cursor_row_after_growing(|t| {
                set(t, Some(false));
                t.vt_write(b"\x1bc");
            }),
            4
        );
        assert_eq!(
            cursor_row_after_growing(|t| {
                set(t, Some(false));
                t.reset();
            }),
            4
        );
    }

    #[test]
    fn render_hold_reports_start_and_end_in_pairs() {
        // Mirrors upstream's "set render_hold callback" test in
        // src/terminal/c/terminal.zig.
        let events = RefCell::new(Vec::new());
        let take = || std::mem::take(&mut *events.borrow_mut());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_render_hold(|_term, held| events.borrow_mut().push(held))
            .expect("callback should register");

        // A set during a hold and a reset without a hold are ignored.
        terminal.vt_write(b"\x1b[?2026h\x1b[?2026hA\x1b[?2026l\x1b[?2026l");
        assert_eq!(take(), [true, false]);

        // Neither a reset nor a resize reports anything without a hold.
        terminal.reset();
        terminal
            .resize(100, 30, 8, 16)
            .expect("resize should succeed");
        assert_eq!(take(), []);

        // Reset and resize end an active hold, and so does a resize that
        // keeps the dimensions: upstream turns synchronized output off
        // before it checks whether the grid size changed.
        terminal.vt_write(b"\x1b[?2026h");
        terminal.reset();
        terminal.reset();
        terminal.vt_write(b"\x1b[?2026h");
        terminal
            .resize(80, 24, 8, 16)
            .expect("resize should succeed");
        terminal.vt_write(b"\x1b[?2026h");
        terminal
            .resize(80, 24, 8, 16)
            .expect("resize should succeed");
        assert_eq!(take(), [true, false, true, false, true, false]);
        assert!(!terminal.mode(Mode::SYNC_OUTPUT).unwrap());

        // A resize that fails leaves the mode, and so the hold, in place.
        terminal.vt_write(b"\x1b[?2026h");
        assert!(terminal.resize(0, 24, 8, 16).is_err());
        assert!(terminal.mode(Mode::SYNC_OUTPUT).unwrap());
        terminal.vt_write(b"\x1b[?2026l");
        assert_eq!(take(), [true, false]);

        // Changing the mode ourselves, e.g. when a hold times out, is never
        // reported in either direction. The hold is simply over, so the
        // callback sees `true` without a matching `false`.
        terminal.vt_write(b"\x1b[?2026h");
        terminal
            .set_mode(Mode::SYNC_OUTPUT, false)
            .expect("mode should be settable");
        assert!(!terminal.mode(Mode::SYNC_OUTPUT).unwrap());
        terminal
            .set_mode(Mode::SYNC_OUTPUT, true)
            .expect("mode should be settable")
            .set_mode(Mode::SYNC_OUTPUT, false)
            .expect("mode should be settable");
        assert_eq!(take(), [true]);
        // ...and the program can start a new hold afterwards.
        terminal.vt_write(b"\x1b[?2026h");
        assert_eq!(take(), [true]);
    }

    /// Read the text of the first row of a render state snapshot.
    fn first_row_text(snapshot: &crate::render::Snapshot<'_, '_>) -> Result<String> {
        let mut rows = crate::render::RowIterator::new()?;
        let mut cells = crate::render::CellIterator::new()?;
        let mut row_iter = rows.update(snapshot)?;
        let Some(row) = row_iter.next() else {
            return Ok(String::new());
        };
        let mut cell_iter = cells.update(row)?;
        let mut text = String::new();
        while let Some(cell) = cell_iter.next() {
            text.extend(cell.graphemes()?);
        }
        Ok(text)
    }

    #[test]
    fn render_hold_captures_frame_before_hold() {
        // A renderer that refreshes its render state whenever a hold starts
        // or ends, recording the first row it captured each time. Failures
        // are recorded as `None` instead of panicking, since a panic here
        // would abort the whole test binary.
        let render_state =
            RefCell::new(RenderState::new().expect("render state should initialize"));
        let frames = RefCell::new(Vec::new());
        let take = || std::mem::take(&mut *frames.borrow_mut());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_render_hold(|term, held| {
                let text = render_state.try_borrow_mut().ok().and_then(|mut state| {
                    let snapshot = state.update(term).ok()?;
                    first_row_text(&snapshot).ok()
                });
                frames.borrow_mut().push((held, text));
            })
            .expect("callback should register");

        // The hold starts before `B` is processed, even though it's in the
        // same write, so the captured frame only has `A`.
        terminal.vt_write(b"A\x1b[?2026hB");
        assert_eq!(take(), [(true, Some("A".to_owned()))]);
        // The terminal itself has moved on, though.
        {
            let mut state = render_state.borrow_mut();
            let snapshot = state.update(&terminal).expect("render state should update");
            assert_eq!(first_row_text(&snapshot).unwrap(), "AB");
        }
        terminal.vt_write(b"\x1b[?2026l");
        assert_eq!(take(), [(false, Some("AB".to_owned()))]);

        // Updating the render state also works when the hold ends from
        // `resize` and `reset`, which invoke the callback while the outer
        // call holds `&mut Terminal`.
        terminal.vt_write(b"\x1b[?2026hC");
        terminal
            .resize(100, 30, 8, 16)
            .expect("resize should succeed");
        assert_eq!(
            take(),
            [
                (true, Some("AB".to_owned())),
                (false, Some("ABC".to_owned()))
            ]
        );
        terminal.vt_write(b"\x1b[?2026hD");
        terminal.reset();
        assert_eq!(
            take(),
            [(true, Some("ABC".to_owned())), (false, Some(String::new()))]
        );
    }

    #[inline(never)]
    fn build_terminal(callback_count: &RefCell<usize>) -> Terminal<'static, '_> {
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");

        terminal
            .on_device_attributes(move |_term| {
                *callback_count.borrow_mut() += 1;
                Some(DeviceAttributes {
                    primary: PrimaryDeviceAttributes::new(
                        ConformanceLevel::VT220,
                        &[DeviceAttributeFeature::ANSI_COLOR],
                    ),
                    secondary: SecondaryDeviceAttributes {
                        device_type: DeviceType::VT220,
                        firmware_version: 1,
                        rom_cartridge: 0,
                    },
                    tertiary: TertiaryDeviceAttributes { unit_id: 0 },
                })
            })
            .expect("callback should register");

        terminal
    }

    /// Move a value into distinct heap storage with an explicit byte-for-byte
    /// relocation so the test does not rely on optimizer or allocator behavior.
    fn relocate_into_new_box<T>(value: T) -> (Box<T>, usize, usize) {
        // Keep the source allocation alive without running T's destructor.
        // We need the bytes to remain initialized until after the copy.
        let src = Box::new(ManuallyDrop::new(value));
        let src_addr = std::ptr::from_ref(&**src).cast::<T>() as usize;

        unsafe {
            let dst_layout = std::alloc::Layout::new::<T>();
            let dst_ptr = std::alloc::alloc(dst_layout).cast::<T>();
            if dst_ptr.is_null() {
                std::alloc::handle_alloc_error(dst_layout);
            }

            let dst_addr = dst_ptr as usize;
            assert_ne!(
                src_addr, dst_addr,
                "test setup failed: source and destination storage unexpectedly match"
            );

            // SAFETY: src points to a fully initialized T wrapped in
            // ManuallyDrop, dst points to distinct uninitialized storage for
            // exactly one T, and the regions do not overlap.
            std::ptr::copy_nonoverlapping(std::ptr::from_ref(&**src).cast::<T>(), dst_ptr, 1);

            // SAFETY: src was allocated as Box<ManuallyDrop<T>> and must be
            // freed without dropping T because ownership was transferred by
            // the raw byte copy above.
            std::alloc::dealloc(
                Box::into_raw(src).cast::<u8>(),
                std::alloc::Layout::new::<ManuallyDrop<T>>(),
            );

            // SAFETY: We just initialized dst_ptr by copying a valid T into it,
            // so it now owns exactly one initialized T allocation.
            (Box::from_raw(dst_ptr), src_addr, dst_addr)
        }
    }

    /// Send an OSC 2 title sequence, then verify `term.title()` returns the
    /// correct value inside the `on_title_changed` callback.
    #[test]
    fn title_changed_callback_returns_correct_title() {
        // The callback bound on `on_title_changed` is `'cb`, not `'static`,
        // so the closure can borrow stack locals directly – no Rc needed.
        let captured_title: RefCell<Vec<u8>> = RefCell::new(Vec::new());
        let callback_count: Cell<usize> = Cell::new(0);

        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");

        terminal
            .on_title_changed(|term| {
                callback_count.set(callback_count.get() + 1);
                let title = term
                    .title()
                    .expect("title() should succeed inside callback");
                *captured_title.borrow_mut() = title.to_owned();
            })
            .expect("callback should register");

        // OSC 2 (set title) should invoke on_title_changed.
        terminal.vt_write(b"\x1b]2;Hello Effects\x1b\\");
        assert_eq!(callback_count.get(), 1);
        assert_eq!(*captured_title.borrow(), b"Hello Effects");

        // A second title change should fire the callback again.
        terminal.vt_write(b"\x1b]2;Second Title\x1b\\");
        assert_eq!(callback_count.get(), 2);
        assert_eq!(*captured_title.borrow(), b"Second Title");
    }

    /// Send an OSC 7 current-directory sequence, then verify `term.pwd()`
    /// returns the correct value inside the `on_pwd_changed` callback.
    #[test]
    fn pwd_changed_callback_returns_correct_pwd() {
        let captured_pwd: RefCell<Vec<u8>> = RefCell::new(Vec::new());
        let callback_count: Cell<usize> = Cell::new(0);

        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");

        terminal
            .on_pwd_changed(|term| {
                callback_count.set(callback_count.get() + 1);
                let pwd = term.pwd().expect("pwd() should succeed inside callback");
                *captured_pwd.borrow_mut() = pwd.to_owned();
            })
            .expect("callback should register");

        terminal.vt_write(b"\x1b]7;file://localhost/tmp/project\x1b\\");
        assert_eq!(callback_count.get(), 1);
        assert_eq!(*captured_pwd.borrow(), b"file://localhost/tmp/project");

        terminal.vt_write(b"\x1b]7;file://localhost/tmp/other\x1b\\");
        assert_eq!(callback_count.get(), 2);
        assert_eq!(*captured_pwd.borrow(), b"file://localhost/tmp/other");

        // libghostty stores the pwd bytes as-is, so non-UTF-8 input must come
        // back verbatim instead of being rejected.
        terminal.vt_write(b"\x1b]7;file://localhost/tmp/\xff\x1b\\");
        assert_eq!(callback_count.get(), 3);
        assert_eq!(*captured_pwd.borrow(), b"file://localhost/tmp/\xff");
    }

    #[test]
    fn default_cursor_reset_uses_configured_style_and_blink() {
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        let mut render_state = RenderState::new().expect("render state should initialize");

        terminal
            .set_default_cursor_style(Some(CursorStyle::Underline))
            .expect("default cursor style should update")
            .set_default_cursor_blink(Some(true))
            .expect("default cursor blink should update");

        terminal.vt_write(b"\x1b[0 q");
        let snapshot = render_state
            .update(&terminal)
            .expect("render state should update");

        assert_eq!(
            snapshot
                .cursor_visual_style()
                .expect("cursor style should be readable"),
            CursorVisualStyle::Underline
        );
        assert!(
            snapshot
                .cursor_blinking()
                .expect("cursor blink should be readable")
        );
    }

    #[test]
    fn glyph_protocol_enabled_setting_updates() {
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");

        terminal
            .set_glyph_protocol_enabled(false)
            .expect("glyph protocol should disable")
            .set_glyph_protocol_enabled(true)
            .expect("glyph protocol should enable");
    }

    /// Explicitly relocate the Terminal into distinct storage, then verify the
    /// callback still fires through the stable `VTable` userdata pointer.
    #[test]
    fn callbacks_survive_explicit_relocation() {
        let callback_count = RefCell::new(0usize);
        let terminal = build_terminal(&callback_count);
        let (mut terminal, addr_before, addr_after) = relocate_into_new_box(terminal);
        assert_ne!(addr_before, addr_after);

        // Primary DA request (CSI c) should invoke on_device_attributes.
        terminal.vt_write(b"\x1b[c");
        assert_eq!(*callback_count.borrow(), 1);
    }

    // The next two tests reach simdutf, which libghostty-vt DLLs on Windows
    // used to crash in, since their C++ global constructors never ran. Pure
    // ASCII never reaches it.

    #[test]
    fn clipboard_write_decodes_base64() {
        let written = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_clipboard_write(|_, request| {
                // Record rather than assert here: a panic in a callback
                // aborts the whole test binary.
                for content in request.contents() {
                    written.borrow_mut().push(content.data.to_vec());
                }
                request.reply(Ok(()), false);
            })
            .unwrap();

        terminal.vt_write(b"\x1b]52;c;//4=\x1b\\");
        assert_eq!(*written.borrow(), [vec![0xff, 0xfe]]);
    }

    #[test]
    fn multibyte_utf8_is_decoded() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        // "é" is 0xC3 0xA9. Both bytes must arrive in one write: a character
        // split across writes is finished by the scalar decoder, and never
        // reaches simdutf.
        terminal.vt_write(b"\xc3\xa9");
        let codepoint = terminal
            .grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
            .unwrap()
            .cell()
            .unwrap()
            .codepoint()
            .unwrap();
        assert_eq!(codepoint, 0xe9);
    }

    #[test]
    fn mouse_shape_follows_osc_22() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::Text);
        // OSC 22 names the shape with its W3C cursor name.
        terminal.vt_write(b"\x1b]22;pointer\x07");
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::Pointer);
        terminal.vt_write(b"\x1b]22;nwse-resize\x1b\\");
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::NwseResize);
        // A name libghostty doesn't know leaves the shape alone.
        terminal.vt_write(b"\x1b]22;not-a-shape\x07");
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::NwseResize);
        // An empty name gives the pointer back.
        terminal.vt_write(b"\x1b]22;\x1b\\");
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::Text);
    }

    /// OSC 9 and OSC 777 carry the program's raw bytes, which libghostty
    /// passes on unvalidated. They used to be exposed as `&str`.
    #[test]
    fn desktop_notifications_are_not_assumed_to_be_utf8() {
        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_desktop_notification(|_, notification| {
                seen.borrow_mut()
                    .push((notification.title().to_vec(), notification.body().to_vec()));
            })
            .unwrap();
        terminal.vt_write(b"\x1b]9;\xff\xfe\x07");
        terminal.vt_write(b"\x1b]777;notify;\xc3\x28;ok\x07");
        assert_eq!(
            *seen.borrow(),
            [
                (b"".to_vec(), b"\xff\xfe".to_vec()),
                (b"\xc3\x28".to_vec(), b"ok".to_vec()),
            ]
        );
    }

    /// What a program status callback observed about one report, copied
    /// out since the report is only valid during the callback.
    ///
    /// `state` and `kind` are `None` when the raw value didn't convert, so
    /// that shows up as a failed comparison outside the callback.
    #[derive(Debug, PartialEq, Eq)]
    struct SeenStatus {
        state: Option<ProgramStatusState>,
        kind: Option<ProgramStatusKind>,
        progress: Option<u8>,
        id: Vec<u8>,
        app: Vec<u8>,
        title: Vec<u8>,
        message: Vec<u8>,
    }

    impl SeenStatus {
        /// A report with only `state` set, as upstream fills in every
        /// value the program didn't send.
        fn bare(state: ProgramStatusState) -> Self {
            Self {
                state: Some(state),
                kind: Some(ProgramStatusKind::None),
                progress: None,
                id: Vec::new(),
                app: Vec::new(),
                title: Vec::new(),
                message: Vec::new(),
            }
        }
    }

    /// Feed `input` to a terminal with a program status callback, returning
    /// the reports it saw and what the terminal wrote back to the pty.
    ///
    /// Assertions happen outside the callback: a panic inside it would
    /// abort the whole test binary instead of failing the test.
    fn program_status(input: &[u8]) -> (Vec<SeenStatus>, Vec<u8>) {
        let seen = RefCell::new(Vec::new());
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_pty_write(|_, bytes| output.borrow_mut().extend_from_slice(bytes))
            .unwrap()
            .on_program_status(|_, report| {
                seen.borrow_mut().push(SeenStatus {
                    state: report.state().ok(),
                    kind: report.kind().ok(),
                    progress: report.progress(),
                    id: report.id().to_vec(),
                    app: report.app().to_vec(),
                    title: report.title().to_vec(),
                    message: report.message().to_vec(),
                });
            })
            .unwrap();
        terminal.vt_write(input);
        drop(terminal);
        (seen.into_inner(), output.into_inner())
    }

    #[test]
    fn program_status_delivers_every_field() {
        // title is "Plan" and msg is "Apply?", both base64.
        let (seen, output) = program_status(
            b"\x1b]7501;state=blocked:kind=permission:progress=40:id=a/b\
              :app=terraform:title=UGxhbg==:msg=QXBwbHk/\x07",
        );
        assert_eq!(
            seen,
            [SeenStatus {
                state: Some(ProgramStatusState::Blocked),
                kind: Some(ProgramStatusKind::Permission),
                progress: Some(40),
                id: b"a/b".to_vec(),
                app: b"terraform".to_vec(),
                title: b"Plan".to_vec(),
                message: b"Apply?".to_vec(),
            }]
        );
        assert!(output.is_empty());
    }

    #[test]
    fn program_status_leaves_unsent_values_empty() {
        // The example from the upstream docs: no progress means the work is
        // indeterminate, and no id means the root record.
        let (seen, _) = program_status(
            b"\x1b]7501;state=blocked:kind=permission:app=terraform:msg=QXBwbHk/\x1b\\",
        );
        assert_eq!(
            seen,
            [SeenStatus {
                kind: Some(ProgramStatusKind::Permission),
                app: b"terraform".to_vec(),
                message: b"Apply?".to_vec(),
                ..SeenStatus::bare(ProgramStatusState::Blocked)
            }]
        );

        let (seen, _) = program_status(b"\x1b]7501;state=done\x1b\\");
        assert_eq!(seen, [SeenStatus::bare(ProgramStatusState::Done)]);
    }

    #[test]
    fn program_status_progress_and_kind_follow_state() {
        let (seen, _) = program_status(
            &[
                // 0 is a real percentage, not "no progress".
                &b"\x1b]7501;state=working:progress=0\x07"[..],
                b"\x1b]7501;state=working:progress=100\x07",
                // Out of range reads as no percentage.
                b"\x1b]7501;state=working:progress=101\x07",
                // Progress and kind only count for the states that use them.
                b"\x1b]7501;state=idle:progress=50:kind=question\x07",
                b"\x1b]7501;state=blocked:kind=auth\x07",
                // A kind libghostty doesn't know reads as none.
                b"\x1b]7501;state=blocked:kind=telepathy\x07",
            ]
            .concat(),
        );
        assert_eq!(
            seen,
            [
                SeenStatus {
                    progress: Some(0),
                    ..SeenStatus::bare(ProgramStatusState::Working)
                },
                SeenStatus {
                    progress: Some(100),
                    ..SeenStatus::bare(ProgramStatusState::Working)
                },
                SeenStatus::bare(ProgramStatusState::Working),
                SeenStatus::bare(ProgramStatusState::Idle),
                SeenStatus {
                    kind: Some(ProgramStatusKind::Auth),
                    ..SeenStatus::bare(ProgramStatusState::Blocked)
                },
                SeenStatus::bare(ProgramStatusState::Blocked),
            ]
        );
    }

    #[test]
    fn program_status_drops_invalid_reports() {
        // An unknown state and a bad id each discard the whole report.
        let (seen, output) =
            program_status(b"\x1b]7501;state=sleeping\x07\x1b]7501;state=idle:id=a//b\x07");
        assert!(seen.is_empty());
        assert!(output.is_empty());
    }

    #[test]
    fn program_status_clears_every_record_on_full_reset() {
        let (seen, _) = program_status(b"\x1b]7501;state=error:id=x\x07\x1bc");
        assert_eq!(
            seen,
            [
                SeenStatus {
                    id: b"x".to_vec(),
                    ..SeenStatus::bare(ProgramStatusState::Error)
                },
                SeenStatus::bare(ProgramStatusState::Clear),
            ]
        );
    }

    #[test]
    fn program_status_is_not_cleared_by_reset() {
        // Upstream's `ghostty_terminal_reset` only resets the terminal; the
        // clear report comes from the stream handling `ESC c`.
        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_program_status(|_, report| {
                seen.borrow_mut()
                    .push((report.state().ok(), report.id().to_vec()));
            })
            .unwrap();

        terminal.vt_write(b"\x1b]7501;state=error:id=x\x07");
        assert_eq!(
            seen.take(),
            [(Some(ProgramStatusState::Error), b"x".to_vec())]
        );

        terminal.reset();
        assert!(seen.borrow().is_empty());

        terminal.vt_write(b"\x1bc");
        assert_eq!(seen.take(), [(Some(ProgramStatusState::Clear), Vec::new())]);
    }

    #[test]
    fn program_status_query_is_answered_only_with_a_callback() {
        const REPLY: &[u8] =
            b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth";

        // The reply uses the same terminator as the query, and the query
        // itself is not a report.
        let (seen, output) = program_status(b"\x1b]7501;?\x1b\\\x1b]7501;?\x07");
        assert!(seen.is_empty());
        assert_eq!(output, [REPLY, b"\x1b\\", REPLY, b"\x07"].concat());

        // Without the callback the query goes unanswered, so the program
        // sees the protocol as unsupported.
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_pty_write(|_, bytes| output.borrow_mut().extend_from_slice(bytes))
            .unwrap();
        terminal.vt_write(b"\x1b]7501;?\x1b\\");
        drop(terminal);
        assert!(output.into_inner().is_empty());
    }

    /// What a semantic prompt callback observed about one event, copied out
    /// since the event is only valid during the callback.
    ///
    /// `kind` and `prompt_kind` are `None` when the raw value didn't
    /// convert, so that shows up as a failed comparison outside the
    /// callback.
    #[derive(Debug, PartialEq, Eq)]
    struct SeenPrompt {
        kind: Option<SemanticPromptKind>,
        prompt_kind: Option<PromptKind>,
        exit_code: Option<i32>,
        command: Vec<u8>,
        error: Vec<u8>,
    }

    impl SeenPrompt {
        /// An event with only `kind` set, as upstream leaves every other
        /// field zero or empty (and `prompt_kind` primary) unless the kind
        /// carries it.
        fn bare(kind: SemanticPromptKind) -> Self {
            Self {
                kind: Some(kind),
                prompt_kind: Some(PromptKind::Primary),
                exit_code: None,
                command: Vec::new(),
                error: Vec::new(),
            }
        }
    }

    /// Feed `input` to a terminal with a semantic prompt callback, returning
    /// the events it saw.
    ///
    /// Assertions happen outside the callback: a panic inside it would
    /// abort the whole test binary instead of failing the test.
    fn semantic_prompt(input: &[u8]) -> Vec<SeenPrompt> {
        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_semantic_prompt(|_, event| {
                seen.borrow_mut().push(SeenPrompt {
                    kind: event.kind().ok(),
                    prompt_kind: event.prompt_kind().ok(),
                    exit_code: event.exit_code(),
                    command: event.command().to_vec(),
                    error: event.error().to_vec(),
                });
            })
            .unwrap();
        terminal.vt_write(input);
        drop(terminal);
        seen.into_inner()
    }

    #[test]
    fn semantic_prompt_reports_each_command_step() {
        // Mirrors upstream's "set semantic_prompt callback" test in
        // src/terminal/c/terminal.zig, plus a few cases from the stream
        // handler test: OSC 133 A/P/N start a prompt (P's `k=` picks right,
        // continuation or secondary, and an unknown kind falls back to
        // primary), B/I start input, C starts output with a percent-encoded
        // `cmdline_url`, and D ends the command with an optional exit code
        // and `err=`.
        let seen = semantic_prompt(
            b"\x1b]133;A\x07\
              \x1b]133;P;k=r\x07\
              \x1b]133;P;k=c\x07\
              \x1b]133;P;k=s\x07\
              \x1b]133;P;k=x\x07\
              \x1b]133;N\x07\
              \x1b]133;B\x07\
              \x1b]133;I\x07\
              \x1b]133;C;cmdline_url=ls%20-la\x07\
              \x1b]133;D;0\x07\
              \x1b]133;D;-1;err=boom\x07\
              \x1b]133;D\x07",
        );
        assert_eq!(
            seen,
            [
                SeenPrompt::bare(SemanticPromptKind::PromptStart),
                SeenPrompt {
                    prompt_kind: Some(PromptKind::Right),
                    ..SeenPrompt::bare(SemanticPromptKind::PromptStart)
                },
                SeenPrompt {
                    prompt_kind: Some(PromptKind::Continuation),
                    ..SeenPrompt::bare(SemanticPromptKind::PromptStart)
                },
                SeenPrompt {
                    prompt_kind: Some(PromptKind::Secondary),
                    ..SeenPrompt::bare(SemanticPromptKind::PromptStart)
                },
                SeenPrompt::bare(SemanticPromptKind::PromptStart),
                SeenPrompt::bare(SemanticPromptKind::PromptStart),
                SeenPrompt::bare(SemanticPromptKind::InputStart),
                SeenPrompt::bare(SemanticPromptKind::InputStart),
                SeenPrompt {
                    command: b"ls -la".to_vec(),
                    ..SeenPrompt::bare(SemanticPromptKind::OutputStart)
                },
                // 0 is a real exit code, distinct from "not reported".
                SeenPrompt {
                    exit_code: Some(0),
                    ..SeenPrompt::bare(SemanticPromptKind::CommandEnd)
                },
                SeenPrompt {
                    exit_code: Some(-1),
                    error: b"boom".to_vec(),
                    ..SeenPrompt::bare(SemanticPromptKind::CommandEnd)
                },
                SeenPrompt::bare(SemanticPromptKind::CommandEnd),
            ]
        );
    }

    #[test]
    fn semantic_prompt_command_is_raw_bytes() {
        // Percent-decoding can produce any bytes, and upstream makes no
        // UTF-8 promise, so non-UTF-8 command lines come through untouched.
        // An undecodable command line and a fresh line alone (OSC 133 L,
        // layout rather than a command step) are covered too.
        let seen = semantic_prompt(
            b"\x1b]133;C;cmdline_url=%ff%fe\x07\
              \x1b]133;C;cmdline='bad\x07\
              \x1b]133;L\x07",
        );
        assert_eq!(
            seen,
            [
                SeenPrompt {
                    command: b"\xff\xfe".to_vec(),
                    ..SeenPrompt::bare(SemanticPromptKind::OutputStart)
                },
                SeenPrompt::bare(SemanticPromptKind::OutputStart),
            ]
        );
    }

    /// Feed `input` to an 80x24 terminal with a resize window callback,
    /// returning the requests it saw and the terminal's size afterwards as
    /// `(cols, rows)`.
    ///
    /// Assertions happen outside the callback: a panic inside it would
    /// abort the whole test binary instead of failing the test.
    fn resize_window(input: &[u8]) -> (Vec<ResizeWindow>, (u16, u16)) {
        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).unwrap();
        terminal
            .on_resize_window(|_, request| seen.borrow_mut().push(request))
            .unwrap();
        terminal.vt_write(input);
        let size = (terminal.cols().unwrap(), terminal.rows().unwrap());
        drop(terminal);
        (seen.into_inner(), size)
    }

    #[test]
    fn resize_window_delivers_rows_then_columns() {
        let (seen, size) = resize_window(b"\x1b[8;30;100t");
        // Asymmetric values catch a swapped rows/columns mapping.
        assert_eq!(
            seen,
            [ResizeWindow::Grid {
                rows: NonZeroU16::new(30),
                columns: NonZeroU16::new(100),
            }]
        );
        // The request alone doesn't resize the terminal.
        assert_eq!(size, (80, 24));
    }

    #[test]
    fn resize_window_reports_omitted_and_zero_params_as_none() {
        // Upstream can't tell an explicit 0 from an omitted parameter, and
        // both mean "keep the current size" for that dimension.
        let (seen, size) = resize_window(b"\x1b[8;;120t\x1b[8;30t\x1b[8t\x1b[8;0;0t");
        assert_eq!(
            seen,
            [
                ResizeWindow::Grid {
                    rows: None,
                    columns: NonZeroU16::new(120),
                },
                ResizeWindow::Grid {
                    rows: NonZeroU16::new(30),
                    columns: None,
                },
                ResizeWindow::Grid {
                    rows: None,
                    columns: None,
                },
                ResizeWindow::Grid {
                    rows: None,
                    columns: None,
                },
            ]
        );
        assert_eq!(size, (80, 24));
    }

    #[test]
    fn resize_window_from_raw_skips_unknown_tags() {
        let mut raw = ffi::TerminalResizeWindow {
            tag: ffi::TerminalResizeWindowTag::GRID,
            ..Default::default()
        };
        raw.value.grid = ffi::TerminalResizeWindowGrid {
            rows: 30,
            columns: 100,
        };
        assert_eq!(
            // SAFETY: `raw` is a live local with the tag and `grid` written.
            unsafe { ResizeWindow::from_raw(&raw const raw) },
            Some(ResizeWindow::Grid {
                rows: NonZeroU16::new(30),
                columns: NonZeroU16::new(100),
            })
        );

        // A kind added upstream later has no Rust value. That the trampoline
        // then skips the closure is checked by the miri mock in
        // `miri_soundness::resize_window_skips_unknown_tags`.
        raw.tag = ffi::TerminalResizeWindowTag::GRID + 1;
        // SAFETY: Ditto; an unknown tag makes `from_raw` read nothing else.
        assert_eq!(unsafe { ResizeWindow::from_raw(&raw const raw) }, None);
    }

    #[test]
    fn unknown_sequence_from_raw_skips_unknown_tags_and_terminators() {
        let apc_bytes = *b"Gf=100";
        let osc_bytes = *b"7400;status=busy";
        let mut raw = ffi::TerminalUnknownSequence {
            tag: ffi::TerminalUnknownSequenceTag::APC,
            ..Default::default()
        };
        raw.value.apc = ffi::TerminalUnknownStringSequence {
            truncated: true,
            content: ffi::String {
                ptr: apc_bytes.as_ptr(),
                len: apc_bytes.len(),
            },
        };
        assert_eq!(
            // SAFETY: `raw` is a live local with the tag and `apc` written,
            // and its content points into `apc_bytes`, which outlives it.
            unsafe { UnknownSequence::from_raw(&raw const raw) },
            Some(UnknownSequence::Apc {
                content: b"Gf=100",
                truncated: true,
            })
        );

        raw.tag = ffi::TerminalUnknownSequenceTag::OSC;
        raw.value.osc = ffi::TerminalUnknownOscSequence {
            truncated: false,
            content: ffi::String {
                ptr: osc_bytes.as_ptr(),
                len: osc_bytes.len(),
            },
            terminator: ffi::OscTerminator::ST,
        };
        assert_eq!(
            // SAFETY: Ditto, with `osc` written and pointing into `osc_bytes`.
            unsafe { UnknownSequence::from_raw(&raw const raw) },
            Some(UnknownSequence::Osc {
                content: b"7400;status=busy",
                truncated: false,
                terminator: osc::Terminator::St,
            })
        );

        // A terminator added upstream later has no Rust value, so the
        // sequence is skipped rather than given an invented one.
        raw.value.osc.terminator = ffi::OscTerminator::BEL + 1;
        // SAFETY: Ditto
        assert_eq!(unsafe { UnknownSequence::from_raw(&raw const raw) }, None);

        // A kind added upstream later has no Rust value either. That the
        // trampoline then skips the closure is checked by the miri mock in
        // `miri_soundness::unknown_sequence_skips_unknown_tags_and_terminators`.
        raw.tag = ffi::TerminalUnknownSequenceTag::OSC + 1;
        // SAFETY: Ditto; an unknown tag makes `from_raw` read nothing else.
        assert_eq!(unsafe { UnknownSequence::from_raw(&raw const raw) }, None);
    }

    fn tiny_terminal() -> Terminal<'static, 'static> {
        Terminal::new(8, 3).expect("terminal should initialize")
    }

    fn codepoint_at_tracked_ref(terminal: &Terminal<'_, '_>, tracked: &TrackedGridRef) -> u32 {
        let snapshot = tracked
            .snapshot(terminal)
            .expect("tracked snapshot should not fail")
            .expect("tracked ref should have a value");
        snapshot
            .cell()
            .expect("tracked snapshot should resolve to a cell")
            .codepoint()
            .expect("tracked snapshot cell should expose a codepoint")
    }

    #[test]
    fn tracked_grid_ref_follows_scroll() {
        let mut terminal = tiny_terminal();
        terminal.vt_write(b"alpha\r\nbravo\r\ncharlie");

        let tracked = terminal
            .track_grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
            .expect("tracked grid ref should initialize");

        terminal.vt_write(b"\r\ndelta");

        assert!(tracked.has_value());
        assert_eq!(
            codepoint_at_tracked_ref(&terminal, &tracked),
            u32::from('a')
        );
        assert_eq!(
            tracked
                .point(PointSpace::Screen)
                .expect("tracked point should resolve")
                .expect("tracked point should have a value")
                .x,
            0
        );
    }

    #[test]
    fn tracked_grid_ref_reports_loss_and_can_set_point() {
        let mut terminal = tiny_terminal();
        terminal.vt_write(b"alpha\r\nbravo\r\ncharlie");

        let mut tracked = terminal
            .track_grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
            .expect("tracked grid ref should initialize");

        terminal.reset();

        assert!(!tracked.has_value());
        assert!(
            tracked
                .snapshot(&terminal)
                .expect("missing tracked snapshot should not fail")
                .is_none()
        );
        assert!(
            tracked
                .point(PointSpace::Screen)
                .expect("missing tracked point should not fail")
                .is_none()
        );

        terminal.vt_write(b"echo");
        tracked
            .set(&mut terminal, Point::Active(PointCoordinate { x: 0, y: 0 }))
            .expect("tracked grid ref should set to a new point");

        assert!(tracked.has_value());
        assert_eq!(
            codepoint_at_tracked_ref(&terminal, &tracked),
            u32::from('e')
        );
    }

    #[test]
    fn tracked_grid_ref_survives_terminal_drop() {
        let tracked = {
            let mut terminal = tiny_terminal();
            terminal.vt_write(b"alpha");
            terminal
                .track_grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
                .expect("tracked grid ref should initialize")
        };

        assert!(!tracked.has_value());
        assert!(
            tracked
                .point(PointSpace::Screen)
                .expect("detached tracked point should not fail")
                .is_none()
        );
    }

    #[test]
    fn tracked_grid_ref_rejects_different_terminal() {
        let mut first = tiny_terminal();
        first.vt_write(b"alpha");
        let mut second = tiny_terminal();
        second.vt_write(b"bravo");

        let mut tracked = first
            .track_grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
            .expect("tracked grid ref should initialize");

        assert!(matches!(
            tracked.snapshot(&second),
            Err(Error::InvalidValue)
        ));
        assert!(matches!(
            tracked.set(&mut second, Point::Active(PointCoordinate { x: 0, y: 0 })),
            Err(Error::InvalidValue)
        ));
    }

    #[test]
    fn grid_ref_converts_back_to_point() {
        let mut terminal = tiny_terminal();
        terminal.vt_write(b"alpha");

        let original = PointCoordinate { x: 1, y: 0 };
        let grid_ref = terminal
            .grid_ref(Point::Active(original))
            .expect("grid ref should resolve");

        assert_eq!(
            terminal
                .point_from_grid_ref(&grid_ref, PointSpace::Active)
                .expect("grid ref point conversion should not fail")
                .expect("grid ref should be representable in active space"),
            original
        );
    }

    /// The C API only allows reading the fields within a request's `size`.
    /// For a request that ends before `name`, the fields past it must not be
    /// read, and `reply` must not call whatever lies where the reply function
    /// would be.
    #[test]
    fn clipboard_write_reads_only_fields_within_its_size() {
        unsafe extern "C" fn reply(
            _: *const ffi::ClipboardWrite,
            _: *const ffi::ClipboardWriteReply,
        ) {
            panic!("a reply function past the request's size was called");
        }
        let data = b"hi";
        let content = ffi::ClipboardContent {
            mime: ffi::String::from("text/plain"),
            data: ffi::String {
                ptr: data.as_ptr(),
                len: data.len(),
            },
        };
        let raw = ffi::ClipboardWrite {
            size: std::mem::offset_of!(ffi::ClipboardWrite, name),
            location: ffi::ClipboardLocation::PRIMARY,
            contents: &raw const content,
            contents_len: 1,
            name: ffi::String::from("program"),
            granted: true,
            can_remember: true,
            reply: Some(reply),
            ..ffi::sized!(ffi::ClipboardWrite)
        };
        // SAFETY: `raw` outlives the borrow, matching the callback contract.
        let write = unsafe { ClipboardWrite::from_raw(&raw const raw) };
        assert_eq!(write.location(), ClipboardLocation::Primary);
        assert_eq!(write.contents().next().unwrap().data, b"hi");
        assert!(write.name().is_empty());
        assert!(!write.granted());
        assert!(!write.can_remember());
        write.reply(Ok(()), false);
    }

    /// Same as above for a read request that ends before `name`.
    #[test]
    fn clipboard_read_reads_only_fields_within_its_size() {
        unsafe extern "C" fn reply(
            _: *const ffi::ClipboardRead,
            _: *const ffi::ClipboardReadReply,
        ) {
            panic!("a reply function past the request's size was called");
        }
        let mime = ffi::String::from("text/plain");
        let raw = ffi::ClipboardRead {
            size: std::mem::offset_of!(ffi::ClipboardRead, name),
            location: ffi::ClipboardLocation::SELECTION,
            mimes: &raw const mime,
            mimes_len: 1,
            list: true,
            name: ffi::String::from("program"),
            granted: true,
            can_remember: true,
            reply: Some(reply),
            ..ffi::sized!(ffi::ClipboardRead)
        };
        // SAFETY: `raw` outlives the borrow, matching the callback contract.
        let read = unsafe { ClipboardRead::from_raw(&raw const raw) };
        assert_eq!(read.location(), ClipboardLocation::Selection);
        assert_eq!(read.mimes().collect::<Vec<_>>(), [b"text/plain"]);
        assert!(read.name().is_empty());
        assert!(!read.granted());
        assert!(!read.can_remember());
        read.reply(Ok(&[]), &[], false);
    }

    /// Bold `A`, then `B`, a space and `C` on the first row.
    ///
    /// Expected checksums, worked out from upstream's `xt_checksum.compute`:
    /// bold `A` is 0x41 + 0x80 (bold attribute) = 0xC1, `B` is 0x42 and `C`
    /// is 0x43. The DEC calculation omits the space because it isn't the
    /// first cell, so the sum is 0x146, negated to 0xFEBA.
    const CHECKSUM_SCREEN: &[u8] = b"\x1b[1mA\x1b[0mB C";

    /// DECRQCRA with id 7 over row 1, columns 1 through 4.
    const DECRQCRA: &[u8] = b"\x1b[7;1;1;1;1;4*y";

    /// Write a DECRQCRA and return whatever the terminal replied.
    fn request_checksum(terminal: &mut Terminal<'_, '_>, output: &RefCell<Vec<u8>>) -> Vec<u8> {
        output.borrow_mut().clear();
        terminal.vt_write(DECRQCRA);
        output.borrow().clone()
    }

    #[test]
    fn xt_checksum_report_requires_opt_in() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal.vt_write(CHECKSUM_SCREEN);

        // Disabled by default.
        assert_eq!(request_checksum(&mut terminal, &output), b"");

        terminal
            .set_xt_checksum_report_enabled(true)
            .expect("enabling checksum reports should succeed");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FEBA\x1b\\"
        );

        terminal
            .set_xt_checksum_report_enabled(false)
            .expect("disabling checksum reports should succeed");
        assert_eq!(request_checksum(&mut terminal, &output), b"");
    }

    #[test]
    fn xt_checksum_xtchecksum_ignored_while_disabled() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal.vt_write(CHECKSUM_SCREEN);

        // XTCHECKSUM 1 (don't negate) while reporting is off is dropped, so
        // the reply after enabling is still the negated DEC checksum.
        terminal.vt_write(b"\x1b[1#y");
        terminal
            .set_xt_checksum_report_enabled(true)
            .expect("enabling checksum reports should succeed");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FEBA\x1b\\"
        );

        // Once enabled, the same XTCHECKSUM takes effect: 0x146 unnegated.
        terminal.vt_write(b"\x1b[1#y");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~0146\x1b\\"
        );
    }

    #[test]
    fn xt_checksum_extension_changes_calculation() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal.vt_write(CHECKSUM_SCREEN);
        terminal
            .set_xt_checksum_report_enabled(true)
            .expect("enabling checksum reports should succeed");

        // Without attributes: 0x41 + 0x42 + 0x43 = 0xC6, negated to 0xFF3A.
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::NO_ATTRIBUTES)
            .expect("valid extension should be accepted");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FF3A\x1b\\"
        );

        // Keeping blanks adds the space: 0x146 + 0x20 = 0x166, negated to
        // 0xFE9A.
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::NO_OMIT_BLANKS)
            .expect("valid extension should be accepted");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FE9A\x1b\\"
        );

        // The extension also applies after a full reset (RIS), which clears
        // the screen, so rewrite it.
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::NO_NEGATE)
            .expect("valid extension should be accepted");
        terminal.vt_write(b"\x1bc");
        terminal.vt_write(CHECKSUM_SCREEN);
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~0146\x1b\\"
        );

        // The empty set restores the DEC calculation.
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::empty())
            .expect("empty extension should be accepted");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FEBA\x1b\\"
        );

        // Bits above 31 are rejected and leave the calculation unchanged.
        let result = terminal.set_xt_checksum_extension(XtChecksumExtension::from_bits_retain(32));
        assert!(matches!(result, Err(Error::InvalidValue)));
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FEBA\x1b\\"
        );
    }

    #[test]
    fn xt_checksum_extension_unwritten_and_full_codepoints() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal
            .set_xt_checksum_report_enabled(true)
            .expect("enabling checksum reports should succeed");

        // Row 2: column 1 is U+1D400 MATHEMATICAL BOLD CAPITAL A, written
        // without attributes.
        // Row 1: column 1 is never written, column 2 is a bold `X`. Bold is
        // left on so we'd notice it leaking into the unwritten cell.
        terminal.vt_write("\x1b[2;1H\u{1D400}\x1b[1;2H\x1b[1mX".as_bytes());
        let request = |terminal: &mut Terminal<'_, '_>, req: &[u8]| {
            output.borrow_mut().clear();
            terminal.vt_write(req);
            output.borrow().clone()
        };

        // Row 1, columns 1 through 2. Bold `X` is 0x58 + 0x80 = 0xD8.
        // DEC: the unwritten cell is skipped entirely, so `X` is the only
        // cell: 0xD8, negated to 0xFF28.
        let row1 = b"\x1b[7;1;1;1;1;2*y";
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::empty())
            .expect("valid extension should be accepted");
        assert_eq!(request(&mut terminal, row1), b"\x1bP7!~FF28\x1b\\");
        // Unwritten as spaces: the unwritten cell is a space with no style
        // (bold only applies to cells that get printed) and, being the first
        // cell, isn't trimmed: 0x20 + 0xD8 = 0xF8, negated to 0xFF08.
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::UNWRITTEN_AS_SPACES)
            .expect("valid extension should be accepted");
        assert_eq!(request(&mut terminal, row1), b"\x1bP7!~FF08\x1b\\");

        // Row 2, column 1.
        // DEC: anything outside the 8-bit ranges counts as ESC: 0x1B, negated
        // to 0xFFE5.
        let row2 = b"\x1b[7;1;2;1;2;1*y";
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::empty())
            .expect("valid extension should be accepted");
        assert_eq!(request(&mut terminal, row2), b"\x1bP7!~FFE5\x1b\\");
        // Full codepoints: 0x1D400 truncated to 16 bits is 0xD400, negated to
        // 0x2C00.
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::FULL_CODEPOINTS)
            .expect("valid extension should be accepted");
        assert_eq!(request(&mut terminal, row2), b"\x1bP7!~2C00\x1b\\");
    }
}

/// Soundness regression tests for
/// <https://github.com/Uzaaft/libghostty-rs/issues/74>.
///
/// These tests are gated on `cfg(miri)` because they construct the exact
/// shapes the C API produces and feed them into the safe wrappers, which was
/// UB before the wrappers stopped building slices and `&str` from unvalidated
/// FFI input. The module also holds tests that need the mock C API to produce
/// shapes real libghostty can't yet (e.g. unknown upstream tags). Run with:
///
/// ```sh
/// cargo +nightly miri test -p libghostty-vt miri_soundness
/// ```
#[cfg(all(test, miri))]
mod miri_soundness {
    use super::*;

    // Miri cannot run Zig. These shims implement only native handle ownership
    // and synchronous callback dispatch; the constructor, registration,
    // trampolines and destructor under test are the actual Rust wrapper.
    struct MockTerminal {
        userdata: *mut std::ffi::c_void,
        write: ffi::TerminalWritePtyFn,
        read: ffi::TerminalClipboardReadFn,
        resize: ffi::TerminalResizeWindowFn,
        unknown: ffi::TerminalUnknownSequenceFn,
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn ghostty_terminal_new(
        _: *const ffi::Allocator,
        out: *mut ffi::Terminal,
        _: u16,
        _: u16,
    ) -> ffi::Result::Type {
        let terminal = Box::new(MockTerminal {
            userdata: std::ptr::null_mut(),
            write: None,
            read: None,
            resize: None,
            unknown: None,
        });
        // SAFETY: The constructor supplies a writable out parameter.
        unsafe { *out = Box::into_raw(terminal).cast() };
        ffi::Result::SUCCESS
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn ghostty_terminal_free(terminal: ffi::Terminal) {
        // SAFETY: The wrapper frees the handle from new exactly once.
        unsafe { drop(Box::from_raw(terminal.cast::<MockTerminal>())) };
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn ghostty_terminal_set(
        terminal: ffi::Terminal,
        option: ffi::TerminalOption::Type,
        value: *const std::ffi::c_void,
    ) -> ffi::Result::Type {
        // SAFETY: Registration supplies a live handle with exclusive access.
        let terminal = unsafe { &mut *terminal.cast::<MockTerminal>() };
        match option {
            ffi::TerminalOption::USERDATA => terminal.userdata = value.cast_mut(),
            ffi::TerminalOption::WRITE_PTY => {
                // SAFETY: The wrapper supplies the matching callback ABI.
                terminal.write = Some(unsafe { std::mem::transmute(value) });
            }
            ffi::TerminalOption::CLIPBOARD_READ => {
                // SAFETY: The wrapper supplies the matching callback ABI.
                terminal.read = Some(unsafe { std::mem::transmute(value) });
            }
            ffi::TerminalOption::RESIZE_WINDOW => {
                // SAFETY: The wrapper supplies the matching callback ABI.
                terminal.resize = Some(unsafe { std::mem::transmute(value) });
            }
            ffi::TerminalOption::UNKNOWN_SEQUENCE => {
                // SAFETY: The wrapper supplies the matching callback ABI.
                terminal.unknown = Some(unsafe { std::mem::transmute(value) });
            }
            _ => return ffi::Result::INVALID_VALUE,
        }
        ffi::Result::SUCCESS
    }

    unsafe extern "C" fn reply_to_read(
        read: *const ffi::ClipboardRead,
        _: *const ffi::ClipboardReadReply,
    ) {
        // SAFETY: The synchronous request stores the live mock handle in ctx.
        let terminal = unsafe { (*read).ctx.cast_mut().cast::<ffi::TerminalImpl>() };
        // SAFETY: This models a clipboard reply's nested pty write dispatch.
        unsafe { ghostty_terminal_vt_write(terminal, b"reply".as_ptr(), 5) };
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn ghostty_terminal_vt_write(
        terminal: ffi::Terminal,
        data: *const u8,
        len: usize,
    ) {
        let (userdata, write, read, resize, unknown) = {
            // SAFETY: The wrapper supplies a live handle. End this borrow
            // before dispatch, since a clipboard reply can dispatch again.
            let mock = unsafe { &*terminal.cast::<MockTerminal>() };
            (
                mock.userdata,
                mock.write,
                mock.read,
                mock.resize,
                mock.unknown,
            )
        };
        // SAFETY: vt_write supplies initialized bytes for this call.
        let bytes = unsafe { std::slice::from_raw_parts(data, len) };
        if let Some(kind) = bytes.strip_prefix(b"resize:") {
            // The libghostty these bindings build against only sends tags
            // they know, so a kind added upstream later can only be faked
            // here. The input picks the tag.
            let known = kind == b"grid";
            // Write only the tag and the active field, leaving the rest of the
            // union (and struct padding) uninitialized, so miri flags any
            // read of bytes upstream never wrote. An unknown kind has no
            // field we know, so its whole union stays uninitialized and miri
            // also flags a grid read before the tag is checked.
            let mut request = MaybeUninit::<ffi::TerminalResizeWindow>::uninit();
            let p = request.as_mut_ptr();
            // SAFETY: `p` points to writable, aligned storage for the struct,
            // and `&raw mut` makes no reference to the uninitialized memory.
            // This matches `lib.TaggedUnion.cval`, whose `@unionInit` leaves
            // the rest of the union undefined.
            unsafe {
                if known {
                    (&raw mut (*p).tag).write(ffi::TerminalResizeWindowTag::GRID);
                    (&raw mut (*p).value.grid).write(ffi::TerminalResizeWindowGrid {
                        rows: 30,
                        columns: 100,
                    });
                } else {
                    (&raw mut (*p).tag).write(ffi::TerminalResizeWindowTag::GRID + 1);
                }
            }
            // SAFETY: Registration provided this callback and its userdata;
            // the request remains live throughout synchronous dispatch.
            unsafe { resize.unwrap()(terminal, userdata, p.cast_const()) };
        } else if let Some(kind) = bytes.strip_prefix(b"unknown:") {
            // Like `resize:` above, the input picks the shape and only the tag
            // and the active field are written. `future` is a kind added
            // upstream later, so its whole union stays uninitialized;
            // `osc-badterm` is an OSC with a terminator added upstream later.
            let content = b"payload";
            let content = ffi::String {
                ptr: content.as_ptr(),
                len: content.len(),
            };
            let mut sequence = MaybeUninit::<ffi::TerminalUnknownSequence>::uninit();
            let p = sequence.as_mut_ptr();
            // SAFETY: As for `resize:` above.
            unsafe {
                match kind {
                    b"apc" => {
                        (&raw mut (*p).tag).write(ffi::TerminalUnknownSequenceTag::APC);
                        (&raw mut (*p).value.apc).write(ffi::TerminalUnknownStringSequence {
                            truncated: true,
                            content,
                        });
                    }
                    b"osc" | b"osc-badterm" => {
                        let terminator = if kind == b"osc" {
                            ffi::OscTerminator::BEL
                        } else {
                            ffi::OscTerminator::BEL + 1
                        };
                        (&raw mut (*p).tag).write(ffi::TerminalUnknownSequenceTag::OSC);
                        (&raw mut (*p).value.osc).write(ffi::TerminalUnknownOscSequence {
                            truncated: false,
                            content,
                            terminator,
                        });
                    }
                    _ => (&raw mut (*p).tag).write(ffi::TerminalUnknownSequenceTag::OSC + 1),
                }
            }
            // SAFETY: Registration provided this callback and its userdata;
            // the sequence and its static content remain live throughout
            // synchronous dispatch.
            unsafe { unknown.unwrap()(terminal, userdata, p.cast_const()) };
        } else if bytes == b"clipboard" {
            let request = ffi::ClipboardRead {
                ctx: terminal.cast(),
                reply: Some(reply_to_read),
                ..ffi::sized!(ffi::ClipboardRead)
            };
            // SAFETY: Registration provided this callback and its userdata;
            // the sized request remains live throughout synchronous dispatch.
            unsafe { read.unwrap()(terminal, userdata, &raw const request) };
        } else {
            // SAFETY: Registration provided this callback and its userdata.
            unsafe { write.unwrap()(terminal, userdata, data, len) };
        }
    }

    #[test]
    fn moved_terminal_callbacks_and_destruction() {
        use std::cell::Cell;
        struct CountDrop<'a>(&'a Cell<usize>);
        impl Drop for CountDrop<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        fn dispatch<'alloc: 'cb, 'cb>(
            mut terminal: Terminal<'alloc, 'cb>,
        ) -> Terminal<'alloc, 'cb> {
            terminal.vt_write(b"reply");
            terminal
        }

        let drops = Cell::new(0);
        let calls = Cell::new(0);
        let mut terminal = Terminal::new(8, 2).unwrap();
        let first = CountDrop(&drops);
        let observed = &calls;
        terminal
            .on_pty_write(move |_, bytes| {
                let _guard = &first;
                assert_eq!(bytes, b"reply");
                observed.set(observed.get() + 1);
            })
            .unwrap();
        let mut terminal = dispatch(terminal);
        assert_eq!(calls.get(), 1);
        assert_eq!(drops.get(), 0);

        let second = CountDrop(&drops);
        terminal
            .on_pty_write(move |_, bytes| {
                let _guard = &second;
                assert_eq!(bytes, b"reply");
                observed.set(observed.get() + 10);
            })
            .unwrap();
        assert_eq!(drops.get(), 1);
        drop(dispatch(terminal));
        assert_eq!(calls.get(), 11);
        assert_eq!(drops.get(), 2);
        drop(Terminal::new(8, 2).unwrap());
    }

    #[test]
    fn moved_terminal_nested_clipboard_reply() {
        let order = std::cell::RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal
            .on_pty_write(|_, bytes| {
                assert_eq!(bytes, b"reply");
                order.borrow_mut().push(2);
            })
            .unwrap();
        terminal
            .on_clipboard_read(|_, request| {
                order.borrow_mut().push(1);
                request.reply(Ok(&[]), &[], false);
                order.borrow_mut().push(3);
            })
            .unwrap();
        fn dispatch(mut terminal: Terminal<'_, '_>) {
            terminal.vt_write(b"clipboard");
        }
        dispatch(terminal);
        assert_eq!(*order.borrow(), [1, 2, 3]);
    }

    /// Upstream may add resize request kinds without an ABI break, and asks
    /// callbacks to ignore tags they don't handle. The trampoline must skip
    /// the closure for such a tag rather than abort (e.g. by `expect`ing the
    /// conversion inside `extern "C"`), while still delivering known tags.
    #[test]
    fn resize_window_skips_unknown_tags() {
        let seen = std::cell::RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal
            .on_resize_window(|_, request| seen.borrow_mut().push(request))
            .unwrap();
        // `GRID + 1`, then `GRID` as the positive control.
        terminal.vt_write(b"resize:unknown");
        terminal.vt_write(b"resize:grid");
        drop(terminal);
        assert_eq!(
            seen.into_inner(),
            [ResizeWindow::Grid {
                rows: NonZeroU16::new(30),
                columns: NonZeroU16::new(100),
            }]
        );
    }

    /// Upstream may add unknown-sequence kinds and OSC terminators without an
    /// ABI break, and fills the union with `lib.TaggedUnion.cval`, which
    /// leaves bytes outside the active field (the whole union, for a kind
    /// without a payload) undefined. The trampoline must read only the tag
    /// and then the field it names, and skip the closure for a kind or
    /// terminator it doesn't know, while still delivering known ones.
    #[test]
    fn unknown_sequence_skips_unknown_tags_and_terminators() {
        let seen = std::cell::RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal
            .on_unknown_sequence(|_, sequence| {
                // Copy the content out; it is only borrowed for the callback.
                seen.borrow_mut().push(match sequence {
                    UnknownSequence::Apc { content, truncated } => {
                        (None, content.to_vec(), truncated)
                    }
                    UnknownSequence::Osc {
                        content,
                        truncated,
                        terminator,
                    } => (Some(terminator), content.to_vec(), truncated),
                });
            })
            .unwrap();
        // `OSC + 1` and an OSC ending in `BEL + 1`, then APC and OSC as the
        // positive controls.
        terminal.vt_write(b"unknown:future");
        terminal.vt_write(b"unknown:osc-badterm");
        terminal.vt_write(b"unknown:apc");
        terminal.vt_write(b"unknown:osc");
        drop(terminal);
        assert_eq!(
            seen.into_inner(),
            [
                (None, b"payload".to_vec(), true),
                (Some(osc::Terminator::Bel), b"payload".to_vec(), false),
            ]
        );
    }

    /// The C trampoline declares `contents: ?[*]const ClipboardContent` and
    /// sends `contents = NULL, contents_len = 0` for a write carrying no
    /// representations (e.g. OSC 52 with an empty payload, the documented
    /// "clear the clipboard" shape). `slice::from_raw_parts` requires a
    /// non-null pointer even at length zero, so `contents()` used to be UB
    /// here; it must yield an empty iterator so hosts can observe "clear".
    #[test]
    fn clipboard_write_with_no_representations() {
        let raw = ffi::ClipboardWrite {
            location: ffi::ClipboardLocation::STANDARD,
            contents: std::ptr::null(),
            contents_len: 0,
            ..ffi::sized!(ffi::ClipboardWrite)
        };
        // SAFETY: `raw` outlives the borrow, matching the callback contract.
        let write = unsafe { ClipboardWrite::from_raw(&raw) };
        assert_eq!(write.contents().count(), 0);
    }

    /// OSC 52 payloads are base64-decoded arbitrary bytes ("binary-safe" per
    /// the C header), but `ClipboardContent` used to expose them as `&str`
    /// built with `str::from_utf8_unchecked` in the sys crate, so decoding
    /// the invalid `&str` entered unreachable code in std's UTF-8 decoder.
    /// The data is exposed as `&[u8]` now; check it round-trips verbatim.
    #[test]
    fn clipboard_content_with_non_utf8_data() {
        // OSC 52 payload "//4=" base64-decodes to FF FE, which is not UTF-8.
        let data = [0xFF_u8, 0xFE];
        let raw = ffi::ClipboardContent {
            mime: ffi::String::from("text/plain"),
            data: ffi::String {
                ptr: data.as_ptr(),
                len: data.len(),
            },
        };
        // SAFETY: `data` outlives the borrow, matching the callback contract.
        let content = unsafe { ClipboardContent::from_raw(&raw) };
        assert_eq!(content.mime, b"text/plain");
        assert_eq!(content.data, &data);
    }

    /// The C API doesn't promise UTF-8 for `mime` either. It used to be
    /// validated into a `&str`, with non-UTF-8 values replaced by
    /// `application/octet-stream`; it is raw bytes now, so check a non-UTF-8
    /// mime string comes back verbatim instead of being substituted.
    #[test]
    fn clipboard_content_with_non_utf8_mime() {
        let mime = [0xFF_u8, 0xFE];
        let data = *b"hello";
        let raw = ffi::ClipboardContent {
            mime: ffi::String {
                ptr: mime.as_ptr(),
                len: mime.len(),
            },
            data: ffi::String {
                ptr: data.as_ptr(),
                len: data.len(),
            },
        };
        // SAFETY: `mime` and `data` outlive the borrow, matching the
        // callback contract.
        let content = unsafe { ClipboardContent::from_raw(&raw) };
        assert_eq!(content.mime, &mime);
        assert_eq!(content.data, b"hello");
    }
}
