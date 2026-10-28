//! Managing [render states](RenderState) of the terminal.

use std::{convert::Into, marker::PhantomData, mem::MaybeUninit};

use crate::{
    alloc::{Allocator, Object},
    error::{Error, Result, from_optional_result, from_optional_result_uninit, from_result},
    ffi,
    screen::{Cell, Row},
    style::{RgbColor, Style},
    terminal::Terminal,
};

pub use ffi::RenderStateRowSelection as RowSelection;

/// A number of rows above and below the viewport.
///
/// This is used both to [request](RenderState::set_overscan) overscan and to
/// [report](Snapshot::overscan) how many rows an update captured. See
/// [Overscan](RenderState#overscan) for how the extra rows are used.
pub use ffi::RenderStateOverscan as Overscan;

/// Represents the state required to render a visible screen (a viewport) of
/// a terminal instance.
///
/// This is stateful and optimized for repeated updates from a single terminal
/// instance and only updating dirty regions of the screen.
///
/// The key design principle of this API is that it only needs read/write
/// access to the terminal instance during the update call. This allows the
/// render state to minimally impact terminal IO performance and also allows
/// the renderer to be safely multi-threaded (as long as a lock is held
/// during the update call to ensure exclusive access to the terminal instance).
///
/// The basic usage of this API is:
///
///  1. Create an empty render state
///  2. Update it from a terminal instance whenever you need.
///  3. Read from the render state to get the data needed to draw your frame.
///
/// # Dirty Tracking
///
/// Dirty tracking is a key feature of the render state that allows renderers
/// to efficiently determine what parts of the screen have changed and only
/// redraw changed regions.
///
/// The render state API keeps track of dirty state at two independent layers:
/// a global dirty state that indicates whether the entire frame is clean,
/// partially dirty, or fully dirty, and a per-row dirty state that allows
/// tracking which rows in a partially dirty frame have changed.
///
/// The user of the render state API is expected to unset both of these.
/// The update call does not unset dirty state, it only updates it. After
/// successfully rendering a complete frame, use [`Snapshot::clean`] to unset
/// both layers in one call. The granular setters remain available for
/// callers that only consume part of a frame.
///
/// An extremely important detail: **setting one dirty state doesn't unset
/// the other.** For example, setting the global dirty state to false does
/// not reset the row-level dirty flags. So, the caller of the render state
/// API must be careful to manage both layers of dirty state correctly.
///
/// # Overscan
///
/// By default, the render state captures exactly the rows visible in the
/// viewport. That is all a renderer needs when it draws whole rows.
///
/// Some renderers draw the grid shifted by a fraction of a row, most commonly
/// to scroll smoothly. While the grid is shifted, part of a row just outside
/// the viewport becomes visible at one edge, and the renderer needs that
/// row's content to draw it. Overscan asks the render state to capture extra
/// rows above and below the viewport for this purpose.
///
/// Request overscan with [`RenderState::set_overscan`]. The request applies to
/// every update after it is set. [Row iterations](RowIteration) then visit
/// the extra rows along with the viewport, from top to bottom: the rows above
/// the viewport, the viewport rows, and then the rows below it.
/// [`RowIteration::viewport_y`] tells you where each row belongs. Rows above
/// the viewport have negative values, viewport rows are 0 through
/// [`rows`](Snapshot::rows) - 1, and rows below the viewport start at
/// [`rows`](Snapshot::rows).
///
/// Extra rows are only captured when they exist. There is nothing above the
/// first line of scrollback, and there is nothing below the viewport while it
/// is scrolled to the bottom, which is the usual case. After an update,
/// [`Snapshot::overscan`] reports how many rows were actually captured on
/// each side. Don't shift the grid toward a side where nothing was captured.
///
/// Extra rows carry the same data as viewport rows, including cells, styles,
/// dirty flags, and selection. The cursor is only reported when it is inside
/// the viewport.
///
/// The render state doesn't store the scroll position. If you need it, read
/// [`Terminal::scrollbar`](crate::Terminal::scrollbar) or
/// [`Terminal::viewport_active`](crate::Terminal::viewport_active) at the same
/// time as you call [`RenderState::begin_update`], while you have exclusive
/// access to the terminal. The values then describe the same moment as the
/// render state.
///
/// # Row identity
///
/// Every row has an [id](RowIteration::id) that stays with the row as it
/// moves. When the viewport scrolls by one row, each row shows up one
/// position higher or lower in the next update but keeps its id. Ids work
/// with or without overscan.
///
/// Ids let a renderer keep expensive per-row work, such as shaped text or a
/// prepared texture, in its own cache keyed by id. A cached entry can be
/// reused when both of these are true:
///
///  1. A row with the same id is present in the new update.
///  2. That row's [dirty flag](RowIteration::dirty) is not set.
///
/// The dirty flag is conservative. A row may be marked dirty even though its
/// content didn't change. For example, every row is currently marked dirty
/// after the viewport scrolls. Rebuilding a dirty row is always correct.
///
/// An id disappears when its row is no longer captured, is removed from
/// scrollback, or is changed in place by the terminal (for example, when a
/// program scrolls only part of the screen). Ids are never reused, so an old
/// id can never match a different row. Cache entries for ids that no longer
/// appear can be discarded.
///
/// # Images
///
/// Programs can show images in a terminal through image protocols, such as
/// the Kitty graphics protocol. The render state gives you the images to draw
/// as a list of image placements, which you read with an [`ImageIterator`].
/// Each placement is one rectangle of one image:
///
///  - **What** to draw: the image's [protocol](ImageIteration::protocol) and
///    [ID](ImageIteration::image_id), plus a source rectangle that selects
///    part of the image in image pixels.
///  - **Where** to draw it: the top-left cell, relative to the viewport, plus
///    a pixel offset within that cell.
///  - **How big** to draw it: a destination size in pixels. The image is
///    already scaled to fit its cells.
///  - **In what order** to draw it: a [layer](ImageLayer) (below cell
///    backgrounds, below text, or above text) and a z-index within the layer.
///
/// These values mean the same thing for every protocol, so one drawing
/// routine handles all of them. The only part that depends on the protocol is
/// getting the image's pixels. Use the placement's protocol to pick the right
/// storage, and its image ID to look the image up. For
/// [`ImageProtocol::Kitty`], that is
// Only link to the `kitty` wrappers when the `kitty-graphics` feature
// provides them, as for the `PlacementIterator` link below.
#[cfg_attr(
    feature = "kitty-graphics",
    doc = "[`Graphics::image`](crate::kitty::graphics::Graphics::image)."
)]
#[cfg_attr(not(feature = "kitty-graphics"), doc = "`Graphics::image`.")]
///
/// ## Which images are included
///
/// The list holds every Kitty graphics image on screen:
///
///  - Images shown with placeholder cells. See
///    [Kitty graphics placeholders](#kitty-graphics-placeholders) below.
///  - Images placed at a cell on the screen. These scroll with the text
///    around them, like a line of output. See
///    [Kitty graphics fixed placements](#kitty-graphics-fixed-placements)
///    below.
///  - Images placed relative to another image. See
///    [Kitty graphics relative placements](#kitty-graphics-relative-placements)
///    below.
///
/// Earlier versions left some of these images out, so renderers had to draw
/// them from the Kitty graphics placement iterator,
// The `kitty` wrappers only exist with the `kitty-graphics` feature, so only
// link to them when they do. Otherwise the link would be broken.
#[cfg_attr(
    feature = "kitty-graphics",
    doc = "[`kitty::graphics::PlacementIterator`](crate::kitty::graphics::PlacementIterator)."
)]
#[cfg_attr(
    not(feature = "kitty-graphics"),
    doc = "`kitty::graphics::PlacementIterator`."
)]
/// That is no longer needed. If your renderer still draws images from that
/// iterator, stop, or those images are drawn twice.
///
/// ## Images partly outside the captured rows
///
/// An image is in the list when any part of it is in the captured rows (the
/// viewport plus any [overscan](#overscan) rows). Images shown with
/// placeholder cells are split into one placement per row of cells, so each
/// of those placements is always inside the captured rows.
///
/// Every other placement is reported at its full size, even when only part of
/// it can be seen. Its [column](ImageIteration::viewport_col) can be
/// negative, and its [row](ImageIteration::viewport_row) can be above the
/// first overscan row. It can also extend past the right or bottom edge. Clip
/// it to the area you draw. Most graphics APIs can do this for you with a
/// scissor or clip rectangle.
///
/// For example, with no overscan, an image 4 rows tall whose top row scrolled
/// 2 rows above the viewport has a row of -2 and covers 4 rows. Draw it at its
/// full size starting 2 rows above the viewport, clipped to the viewport. Its
/// bottom 2 rows show at the top of the viewport.
///
/// ## Kitty graphics fixed placements
///
/// A program can place a Kitty image at a cell on the screen. The image is
/// attached to that cell, so it moves when the text moves: when new output
/// scrolls the screen, when a program scrolls part of the screen, and when
/// the viewport scrolls through the scrollback.
///
/// The render state finds the position of these images during each update,
/// so you don't need to track the cells yourself. This costs very little on a
/// typical frame, including frames where the viewport scrolls.
///
/// Later versions may add more protocols and more kinds of placements. Skip
/// any placement whose protocol you don't support, so that your renderer
/// keeps working when that happens.
///
/// ## Kitty graphics placeholders
///
/// The Kitty graphics protocol lets a program place an image *with text*. The
/// program creates a virtual placement, which has no position of its own,
/// and then prints placeholder characters (U+10EEEE) wherever the image
/// should appear. Each placeholder cell says which image it shows and which
/// piece of that image belongs in the cell. Programs such as tmux use this so
/// that images scroll, move and disappear along with the text around them.
///
/// Since the cells decide where these images go, the Kitty graphics storage
/// alone can't tell you where to draw them. The render state works this out
/// for you during each update. It finds the placeholder cells, groups
/// neighboring cells that show the same image into runs, and calculates which
/// part of the image each run shows. It only looks at rows that changed, so
/// this costs very little on a typical frame.
///
/// A virtual placement usually turns into several placements in the list,
/// one for each row of placeholder cells. For example, an image shown in a
/// block of 10 columns by 4 rows of placeholders gives 4 placements, each 10
/// cells wide and 1 cell tall, and each drawing a quarter of the image. Drawn
/// together, they form the whole image.
///
/// ## Kitty graphics relative placements
///
/// A program can also place a Kitty image relative to another placement, its
/// parent. The image is drawn a number of cells to the right of and below its
/// parent. Negative numbers move it left or up. The parent can be relative to
/// another placement too, so placements can form a chain.
///
/// The render state follows the chain to the placement at its start and
/// positions the image from there:
///
///  - If the chain starts at an image placed at a cell, the image is drawn
///    from that cell, and moves whenever that cell moves.
///  - If the chain starts at a virtual placement, the image is drawn from the
///    top-left corner of that placement's placeholder cells. If none of those
///    cells are in the captured rows, the image is not in the list.
///
/// For example, an image placed 3 columns right of and 2 rows below a parent
/// at column 2, row 1 is drawn at column 5, row 3. Like other placements, it
/// is in the list when any part of it is in the captured rows, even if its
/// parent isn't.
///
/// ## Knowing when images changed
///
/// Most frames don't change any image. A program prints text or moves the
/// cursor, and every image stays where it was. You don't need to read the
/// iterator again on those frames. After each update, read
/// [`Snapshot::image_placements_generation`] and compare it with the value
/// from the last time you read the list. If the two match, the list is the
/// same as before, and you can keep drawing what you built last time:
///
/// ```rust
/// use libghostty_vt::{RenderState, Terminal, render::ImageIterator};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let terminal = Terminal::new(80, 24)?;
/// # let mut render_state = RenderState::new()?;
/// # let mut images = ImageIterator::new()?;
/// // Kept by your renderer between frames. Starting at `None` makes sure the
/// // first frame always reads the list: a new render state reports 0, so
/// // starting at `Some(0)` would skip it.
/// let mut last_generation: Option<u64> = None;
/// # let mut rebuilds = 0;
/// # for _frame in 0..2 {
///
/// // Each frame, after the update:
/// let snapshot = render_state.update(&terminal)?;
/// let generation = snapshot.image_placements_generation()?;
/// if last_generation != Some(generation) {
///     last_generation = Some(generation);
///
///     // Your renderer: read the iterator as in the `ImageIterator` example
///     // and build a new queue of images to draw.
///     let mut placements = images.update(&snapshot)?;
///     while let Some(placement) = placements.next() {
///         // queue_image(placement, ...);
///     }
/// #   rebuilds += 1;
/// }
/// // draw_image_queue();
/// # }
/// # // The first frame read the list even though it reported 0, and the
/// # // second frame, with nothing changed, skipped it.
/// # assert_eq!(last_generation, Some(0));
/// # assert_eq!(rebuilds, 1);
/// # Ok(())}
/// ```
///
/// The value changes when an update adds or removes an image, moves one,
/// changes which part of one is shown, or changes the pixels of one. It stays
/// the same when only text changes, as long as the changed rows had no Kitty
/// placeholder cells before or after the change. (See
/// [Kitty graphics placeholders](#kitty-graphics-placeholders) above for what
/// those are.)
///
/// A new value means "read the list again". It doesn't promise that anything
/// on screen moved. The value can change even when every image ends up
/// exactly where it was, for example:
///
///  - When a program rewrites a row that has placeholder cells.
///  - When a program changes any image or placement while images shown with
///    placeholder cells are on screen.
///
/// Only compare the value for equality. It wraps around, so a larger value
/// doesn't mean a newer one. It is always 0 when libghostty is built without
/// Kitty graphics support. (This crate's `kitty-graphics` feature only gates
/// the `kitty` module's wrappers, so turning it off doesn't make the value
/// 0.)
///
/// Don't confuse this value with [`ImageIteration::image_generation`]. That
/// one belongs to a single image and tells you when to upload its pixels
/// again. This one belongs to the whole list and tells you when to read the
/// list again.
///
/// ## Things to keep in mind
///
///  - **Image pixels are not copied.** The render state only knows where to
///    draw. Read pixels from the terminal while you still have access to it,
///    for example right after the update while you hold your terminal lock.
///    The image generation tells you when you need to read them again, so
///    most frames don't touch the terminal at all.
///  - **Some images are only partly visible.** Clip what you draw to the
///    viewport, or to the captured rows if you draw overscan rows. See
///    [Images partly outside the captured rows](#images-partly-outside-the-captured-rows)
///    above.
///  - **The terminal needs a cell size.** Image sizes depend on the size of a
///    cell in pixels, which you set with [`Terminal::resize`]. Without it
///    there is nothing to draw and the iterator is empty.
///  - **Results last until the next update.** Like rows, they belong to the
///    render state and can be read without access to the terminal. After each
///    update, fill the iterator again with [`ImageIterator::update`] before
///    you read it.
///
/// # Examples
///
/// ## Creating and updating render state
///
/// ```rust
/// // Create a terminal and render state, then update the render state
/// // from the terminal. The render state captures a snapshot of everything
/// // needed to draw a frame.
/// use libghostty_vt::{Terminal, RenderState};
///
/// let mut terminal = Terminal::new(40, 5).unwrap();
/// let mut render_state = RenderState::new().unwrap();
///
/// // Feed some styled content into the terminal.
/// terminal.vt_write(b"Hello, \x1b[1;32mworld\x1b[0m!\r\n");
/// terminal.vt_write(b"\x1b[4munderlined\x1b[0m text\r\n");
/// terminal.vt_write(b"\x1b[38;2;255;128;0morange\x1b[0m\r\n");
///
/// assert!(render_state.update(&terminal).is_ok());
/// ```
///
/// ## Splitting an update
///
/// ```rust
/// use libghostty_vt::{RenderState, Terminal};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let terminal = Terminal::new(80, 25)?;
/// let mut render_state = RenderState::new()?;
///
/// // Use `update` unless you need to minimize how long terminal access is
/// // held. `begin_update` copies the terminal-dependent state into an update
/// // token, then `end` finishes the deferred render-state work.
/// let update = render_state.begin_update(&terminal)?;
///
/// // Terminal access is no longer needed here.
/// let snapshot = update.end()?;
///
/// // Read from the snapshot to draw the frame.
/// let _dirty = snapshot.dirty()?;
/// # Ok(())}
/// ```
///
/// ## Checking dirty state
///
/// ```rust
/// // Check the global dirty state to decide how much work the renderer
/// // needs to do. After rendering, reset it to false.
/// # use libghostty_vt::{Terminal, RenderState, render::Dirty};
/// # let terminal = Terminal::new(80, 25).unwrap();
/// # let mut render_state = RenderState::new().unwrap();
/// let snapshot = render_state.update(&terminal).unwrap();
///
/// match snapshot.dirty().unwrap() {
///     Dirty::Clean => println!("Frame is clean, nothing to draw."),
///     Dirty::Partial => println!("Partial redraw needed."),
///     Dirty::Full => println!("Full redraw needed."),
/// }
/// ```
///
/// ## Reading colors
///
/// ```rust
/// // Retrieve colors (background, foreground, palette) from the render
/// // state. These are needed to resolve palette-indexed cell colors.
/// # use libghostty_vt::{Terminal, RenderState};
/// # let terminal = Terminal::new(80, 25).unwrap();
/// # let mut render_state = RenderState::new().unwrap();
/// let snapshot = render_state.update(&terminal).unwrap();
/// let colors = snapshot.colors().unwrap();
///
/// println!(
///     "Background: {:02x}{:02x}{:02x}",
///     colors.background.r, colors.background.g, colors.background.b
/// );
/// println!(
///     "Foreground: {:02x}{:02x}{:02x}",
///     colors.background.r, colors.background.g, colors.background.b
/// );
/// ```
///
/// ## Reading cursor state
///
/// ```rust
/// // Read cursor position and visual style from the render state.
/// use libghostty_vt::render::CursorViewport;
/// # use libghostty_vt::{Terminal, RenderState};
/// # let terminal = Terminal::new(80, 25).unwrap();
/// # let mut render_state = RenderState::new().unwrap();
/// let snapshot = render_state.update(&terminal).unwrap();
///
/// if snapshot.cursor_visible().unwrap() {
///     if let Some(CursorViewport { x, y, .. }) = snapshot.cursor_viewport().unwrap() {
///         let style = snapshot.cursor_visual_style().unwrap();
///         println!("Cursor at ({x}, {y}), style {style:?}");
///     }
/// }
/// ```
///
/// ## Scrolling smoothly with overscan
///
/// ```rust
/// // Draw one frame of a smooth scroll. `offset_px` comes from the renderer's
/// // own scroll animation: how far the grid is shifted up, from zero up to
/// // but not including one row height.
/// use libghostty_vt::{RenderState, Terminal, render::{Overscan, RowIterator}};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let terminal = Terminal::new(80, 25)?;
/// # let cell_height = 16;
/// # let mut offset_px = 4;
/// # let mut draw_row = |_: &_, _: i32| {};
/// let mut render_state = RenderState::new()?;
/// let mut rows = RowIterator::new()?;
///
/// // Once, when setting up the render state. One row below the viewport is
/// // enough to draw the partially visible row at the bottom edge.
/// render_state.set_overscan(Overscan { above: 0, below: 1 })?;
///
/// // Each frame.
/// let snapshot = render_state.update(&terminal)?;
///
/// // With no row below the viewport, there is nothing to scroll into.
/// if snapshot.overscan()?.below == 0 {
///     offset_px = 0;
/// }
///
/// let mut row_iter = rows.update(&snapshot)?;
/// while let Some(row) = row_iter.next() {
///     draw_row(row, row.viewport_y()? * cell_height - offset_px);
/// }
/// # Ok(())}
/// ```
///
/// ## Caching per-row work by id
///
/// ```rust
/// use std::collections::HashMap;
/// use libghostty_vt::{RenderState, Terminal, render::{RowId, RowIterator}};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let terminal = Terminal::new(80, 25)?;
/// # let mut render_state = RenderState::new()?;
/// # let mut rows = RowIterator::new()?;
/// # let prepare = |_: &_| ();
/// // The renderer's own map from row id to prepared row.
/// let mut cache: HashMap<RowId, ()> = HashMap::new();
///
/// let snapshot = render_state.update(&terminal)?;
/// let mut row_iter = rows.update(&snapshot)?;
/// while let Some(row) = row_iter.next() {
///     let id = row.id()?;
///     if row.dirty()? || !cache.contains_key(&id) {
///         cache.insert(id, prepare(row));
///     }
/// }
/// # Ok(())}
/// ```
///
/// ## Iterating rows and cells
///
/// ```rust
/// // Iterate rows via the row iterator. For each dirty row, iterate its
/// // cells, read codepoints/graphemes and styles, and emit ANSI-colored
/// // output as a simple "renderer".
/// use libghostty_vt::{Terminal, RenderState};
/// use libghostty_vt::style::Underline;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let terminal = Terminal::new(80, 25).unwrap();
/// # let mut render_state = RenderState::new()?;
/// use libghostty_vt::render::{RowIterator, CellIterator};
///
/// // During setup:
/// let mut rows = RowIterator::new()?;
/// let mut cells = CellIterator::new()?;
///
/// // On each frame:
/// let snapshot = render_state.update(&terminal)?;
/// let colors = snapshot.colors()?;
///
/// let mut row_iter = rows.update(&snapshot)?;
/// let mut row_index = 0;
///
/// while let Some(row) = row_iter.next() {
///     // Check per-row dirty state; a real renderer would skip clean rows.
///     print!(
///         "Row {row_index} [{}]",
///         if row.dirty()? { "dirty" } else { "clean" }
///     );
///
///     // Get cells for this row (reuses the same cells handle).
///     let mut cell_iter = cells.update(&row)?;
///     while let Some(cell) = cell_iter.next() {
///         let graphemes = cell.graphemes()?;
///
///         if graphemes.is_empty() {
///             print!(" ");
///             continue;
///         }
///
///         // Resolve foreground color for this cell.
///         let fg = cell.fg_color()?.unwrap_or(colors.foreground);
///         // Emit ANSI true-color escape for the foreground.
///         print!("\x1b[38;2;{};{};{}m", fg.r, fg.g, fg.b);
///
///         // Read the style for this cell. Returns the default style for
///         // cells that have no explicit styling.
///         let style = cell.style()?;
///         if style.bold {
///             print!("\x1b[1m");
///         }
///         if style.underline != Underline::None {
///             print!("\x1b[4m");
///         }
///
///         for grapheme in graphemes {
///             print!("{}", grapheme.escape_default());
///         }
///         print!("\x1b[0m"); // Reset style after each cell.
///     }
///     println!();
///
///     // Clear per-row dirty flag after "rendering" it.
///     row.set_dirty(false);
///
///     row_index += 1;
/// }
/// # Ok(())}
/// ```
#[derive(Debug)]
pub struct RenderState<'alloc>(Object<'alloc, ffi::RenderStateImpl>);

/// A snapshot of the render state after an update.
///
/// This struct exists to guard data accessed from the render state from
/// being accidentally modified after an update. If you find yourself unable
/// to update the render state due to borrow checker errors, make sure to
/// drop the active snapshot (and data that depends on it) before updating.
#[derive(Debug)]
pub struct Snapshot<'alloc, 's>(&'s mut RenderState<'alloc>);

/// An in-progress render state update.
///
/// This token is returned by [`RenderState::begin_update`] and keeps the render
/// state borrowed until [`Self::end`] completes the deferred update work. This
/// makes it impossible to read from the render state while it is incomplete.
#[derive(Debug)]
pub struct Update<'alloc, 's> {
    state: Option<&'s mut RenderState<'alloc>>,
}

/// Opaque handle to a render-state row iterator.
///
/// The row iterator must be [updated](RowIterator::update) from a snapshot of
/// the render state in order to function, as most data is only accessible
/// per [iteration](RowIteration).
///
/// The iteration visits every row the last update captured, from top to
/// bottom. This is exactly the viewport unless
/// [overscan](RenderState#overscan) was requested.
#[derive(Debug)]
pub struct RowIterator<'alloc>(Object<'alloc, ffi::RenderStateRowIteratorImpl>);

/// An active iteration over the rows in the render state.
///
/// Row iterations are created by [updating](RowIterator::update) row iterators
/// with a snapshot of the render state. The borrow checker statically
/// guarantees that all accesses of the data do not outlive the given snapshot,
/// at the cost of added lifetime annotations.
#[derive(Debug)]
pub struct RowIteration<'alloc, 's> {
    iter: &'s mut RowIterator<'alloc>,
    // NOTE: While in theory the snapshot borrow should have its own
    // lifetime 'ss where 'rs: 'ss, but it gets very unwieldy and honestly
    // one wouldn't run into too many situations where this simpler constraint
    // isn't enough.
    _phan: PhantomData<&'s Snapshot<'alloc, 's>>,
}

/// Opaque handle to a render state cell iterator.
///
/// The cell iterator must be [updated](CellIterator::update) from a
/// [row](RowIteration) in order to function, as most data is only
/// accessible per [iteration](CellIteration).
#[derive(Debug)]
pub struct CellIterator<'alloc>(Object<'alloc, ffi::RenderStateRowCellsImpl>);

/// An active iteration over the cells on a given row
/// within the render state.
///
/// Cell iterations are created by [updating](CellIterator::update) row iterators
/// at a given [row](RowIteration). The borrow checker statically
/// guarantees that all accesses of the data do not outlive the given snapshot,
/// at the cost of added lifetime annotations.
#[derive(Debug)]
pub struct CellIteration<'alloc, 's> {
    iter: &'s mut CellIterator<'alloc>,
    _phan: PhantomData<&'s RowIteration<'alloc, 's>>,
}

/// Opaque handle to an iterator over the image placements of a render
/// state.
///
/// The iterator starts out empty. Fill it from a snapshot of the render
/// state with [`ImageIterator::update`], which returns an
/// [iteration](ImageIteration) to read the placements from. You can keep one
/// iterator for the lifetime of your renderer and fill it again after every
/// update. See [Images](RenderState#images) for how placements work.
///
/// # Example
///
/// ```rust
/// use libghostty_vt::{
///     RenderState, Terminal,
///     render::{ImageIterator, ImageProtocol},
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut terminal = Terminal::new(80, 24)?;
/// // Image sizes depend on the cell size, so give the terminal one. Without
/// // it there is nothing to draw and the iterator is empty.
/// terminal.resize(80, 24, 10, 20)?;
///
/// // Transmit and display a 2x2 RGB image (f=24) at the cursor. The payload
/// // is 12 bytes of 0xFF in base64, so a white square.
/// terminal.vt_write(b"\x1b_Ga=T,f=24,s=2,v=2,i=1,q=2;////////////////\x1b\\");
///
/// // Create the iterator once and reuse it for every frame.
/// let mut images = ImageIterator::new()?;
/// let mut render_state = RenderState::new()?;
///
/// // Each frame, after the update. To skip this on frames where no image
/// // changed, see `Snapshot::image_placements_generation`.
/// let snapshot = render_state.update(&terminal)?;
/// let mut placements = images.update(&snapshot)?;
///
/// # let mut count = 0;
/// while let Some(placement) = placements.next() {
///     // Skip any protocol this renderer doesn't support. That includes
///     // protocols added upstream that this crate doesn't know yet, which
///     // give `Err(Error::InvalidValue)` rather than a variant.
///     let Ok(ImageProtocol::Kitty) = placement.protocol() else {
///         continue;
///     };
/// #   count += 1;
///
///     // Your renderer: find the texture for this image, keyed on protocol,
///     // `placement.image_id()?` and `placement.image_generation()?`. Only
///     // read the pixels (with `Graphics::image`) when you don't have one.
///
///     // The destination rectangle in pixels, relative to the top-left
///     // corner of the viewport. It can start above or left of the
///     // viewport, so clip it when drawing.
///     let (cell_width, cell_height) = (10, 20);
///     let x = placement.viewport_col()? * cell_width
///         + i32::try_from(placement.cell_offset_x()?)?;
///     let y = placement.viewport_row()? * cell_height
///         + i32::try_from(placement.cell_offset_y()?)?;
///
///     // Queue the image. Sort the queue by layer and then z before
///     // drawing, since iteration order is not drawing order.
///     // queue_image(texture, placement.layer()?, placement.z()?, x, y,
///     //             placement.pixel_width()?, placement.pixel_height()?, ...);
/// #   assert_eq!(placement.image_id()?, 1);
/// #   // z defaults to 0, which is above the text.
/// #   assert_eq!(placement.layer()?, libghostty_vt::render::ImageLayer::AboveText);
/// #   assert_eq!(placement.z()?, 0);
/// #   // Displayed at the cursor in the top-left cell, with no offset.
/// #   assert_eq!((x, y), (0, 0));
/// #   // No columns or rows were given, so it is drawn at its native 2x2 size.
/// #   assert_eq!((placement.pixel_width()?, placement.pixel_height()?), (2, 2));
/// }
/// # // The `kitty-graphics` feature only gates the `kitty` module's wrappers;
/// # // libghostty stores and places the image either way.
/// # assert_eq!(count, 1);
/// # Ok(())}
/// ```
#[derive(Debug)]
pub struct ImageIterator<'alloc>(Object<'alloc, ffi::RenderStateImageIteratorImpl>);

/// An active iteration over the image placements of a render state.
///
/// Image iterations are created by [updating](ImageIterator::update) image
/// iterators with a snapshot of the render state. The iteration reads the
/// render state's memory, and the borrow checker statically guarantees that
/// it does not outlive the given snapshot (and therefore the next update).
///
/// The getters read the current placement. They return
/// [`Error::InvalidValue`] if the iteration is not on a placement: before the
/// first call to [`ImageIteration::next`], or after it returned `None`. All
/// pixel values use the cell size set with [`Terminal::resize`].
#[derive(Debug)]
pub struct ImageIteration<'alloc, 's> {
    iter: &'s mut ImageIterator<'alloc>,
    // Same simplification as `RowIteration`: one lifetime for both the
    // iterator borrow and the snapshot borrow.
    _phan: PhantomData<&'s Snapshot<'alloc, 's>>,
}

//--------------------------
// Impl blocks
//--------------------------

impl<'alloc> RenderState<'alloc> {
    /// Create a new render state instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new render state instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::RenderState = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_render_state_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    /// Update a render state instance from a terminal,
    /// returning a new [snapshot](Snapshot).
    ///
    /// This consumes terminal/screen dirty state in the same way as the
    /// internal render state update path.
    ///
    /// # Errors
    ///
    /// Returns `Err(Error::OutOfMemory)` if updating the state requires
    /// allocation and that allocation fails.
    pub fn update<'cb>(
        &mut self,
        terminal: &Terminal<'alloc, 'cb>,
    ) -> Result<Snapshot<'alloc, '_>> {
        let result =
            unsafe { ffi::ghostty_render_state_update(self.0.as_raw(), terminal.inner.as_raw()) };
        from_result(result)?;
        Ok(Snapshot(self))
    }

    /// Begin an update of a render state instance from a terminal.
    ///
    /// Every begin must be completed with [`Update::end`] before the render
    /// state is read.
    ///
    /// This two-phase structure exists for callers that synchronize access to
    /// the terminal state: only this function requires terminal access, so a
    /// caller can hold its lock for this call only and then call [`Update::end`]
    /// after releasing it. The end phase exclusively reads
    /// and writes memory owned by the render state, so it is safe to call while
    /// the terminal is being modified.
    ///
    /// Work that doesn't require terminal access may be deferred to the end
    /// phase to keep this call, and therefore lock hold time, as short as
    /// possible. Callers must treat the render state as incomplete until
    /// [`Update::end`] is called.
    ///
    /// This consumes terminal and screen dirty state in the same way as the
    /// internal render state update path.
    pub fn begin_update<'cb>(
        &mut self,
        terminal: &Terminal<'alloc, 'cb>,
    ) -> Result<Update<'alloc, '_>> {
        let result = unsafe {
            ffi::ghostty_render_state_begin_update(self.0.as_raw(), terminal.inner.as_raw())
        };
        from_result(result)?;
        Ok(Update { state: Some(self) })
    }

    fn get<T>(&self, tag: ffi::RenderStateData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_get(self.0.as_raw(), tag, value.as_mut_ptr().cast())
        };
        // Since we manually model every possible query, this should never fail.
        from_result(result)?;
        // SAFETY: Value should be initialized after successful call.
        Ok(unsafe { value.assume_init() })
    }

    fn set<T>(&self, tag: ffi::RenderStateOption::Type, value: &T) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_render_state_set(self.0.as_raw(), tag, std::ptr::from_ref(value).cast())
        };
        // Since we manually model every possible query, this should never fail.
        from_result(result)
    }

    /// Request [overscan](Self#overscan) rows above and below the viewport.
    ///
    /// The request takes effect on the next update and stays in effect until
    /// it is changed. Both sides are zero by default, which captures only the
    /// viewport. Expect a full redraw on the update after a change.
    pub fn set_overscan(&mut self, request: Overscan) -> Result<&mut Self> {
        self.set(ffi::RenderStateOption::OVERSCAN, &request)?;
        Ok(self)
    }

    /// The overscan request most recently set with [`Self::set_overscan`].
    ///
    /// The next update uses this request. Both sides are zero if it was never
    /// set.
    pub fn overscan_request(&self) -> Result<Overscan> {
        self.get(ffi::RenderStateData::OVERSCAN_REQUEST)
    }
}

impl Drop for RenderState<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_render_state_free(self.0.as_raw()) }
    }
}

impl<'alloc, 's> Update<'alloc, 's> {
    /// Complete a prior [`RenderState::begin_update`] call by performing any deferred work.
    ///
    /// This only reads and writes memory owned by the render state, so it is
    /// safe to call while the terminal is being modified. Consumes the update
    /// token and returns a snapshot that can be read to draw the frame.
    pub fn end(mut self) -> Result<Snapshot<'alloc, 's>> {
        let Some(state) = self.state.take() else {
            return Err(Error::InvalidValue);
        };
        let result = unsafe { ffi::ghostty_render_state_end_update(state.0.as_raw()) };
        from_result(result)?;
        Ok(Snapshot(state))
    }
}

impl Drop for Update<'_, '_> {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            let _ = unsafe { ffi::ghostty_render_state_end_update(state.0.as_raw()) };
        }
    }
}

impl Snapshot<'_, '_> {
    fn get<T>(&self, tag: ffi::RenderStateData::Type) -> Result<T> {
        self.0.get(tag)
    }

    fn set<T>(&self, tag: ffi::RenderStateOption::Type, value: &T) -> Result<()> {
        self.0.set(tag, value)
    }

    /// Get the current dirty state.
    pub fn dirty(&self) -> Result<Dirty> {
        self.get::<ffi::RenderStateDirty::Type>(ffi::RenderStateData::DIRTY)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    /// Get the viewport width.
    pub fn cols(&self) -> Result<u16> {
        self.get(ffi::RenderStateData::COLS)
    }

    /// Get the viewport height.
    ///
    /// This does not include [overscan](RenderState#overscan) rows.
    pub fn rows(&self) -> Result<u16> {
        self.get(ffi::RenderStateData::ROWS)
    }

    /// How many [overscan](RenderState#overscan) rows this update captured on
    /// each side.
    ///
    /// This is never more than the [request](RenderState::overscan_request).
    /// It is less when those rows don't exist: `above` is smaller near the top
    /// of the scrollback, and `below` is zero while the viewport is scrolled
    /// to the bottom.
    pub fn overscan(&self) -> Result<Overscan> {
        self.get(ffi::RenderStateData::OVERSCAN)
    }

    /// Get the cursor color that may have been explicitly set by the terminal state.
    pub fn cursor_color(&self) -> Result<Option<RgbColor>> {
        let has_value = self.get(ffi::RenderStateData::COLOR_CURSOR_HAS_VALUE)?;
        if has_value {
            let color = self.get(ffi::RenderStateData::COLOR_CURSOR)?;
            Ok(Some(color))
        } else {
            Ok(None)
        }
    }

    /// Whether the cursor is currently visible based on terminal modes.
    pub fn cursor_visible(&self) -> Result<bool> {
        self.get(ffi::RenderStateData::CURSOR_VISIBLE)
    }

    /// Whether the cursor is currently blinking based on terminal modes.
    pub fn cursor_blinking(&self) -> Result<bool> {
        self.get(ffi::RenderStateData::CURSOR_BLINKING)
    }

    /// Whether the cursor is at a password input field.
    pub fn cursor_password_input(&self) -> Result<bool> {
        self.get(ffi::RenderStateData::CURSOR_PASSWORD_INPUT)
    }

    /// Get the visual style of the cursor.
    pub fn cursor_visual_style(&self) -> Result<CursorVisualStyle> {
        self.get::<ffi::RenderStateCursorVisualStyle::Type>(
            ffi::RenderStateData::CURSOR_VISUAL_STYLE,
        )
        .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    /// Get the relative position of the cursor and other information
    /// if it is currently visible within the viewport.
    pub fn cursor_viewport(&self) -> Result<Option<CursorViewport>> {
        let has_value = self.get(ffi::RenderStateData::CURSOR_VIEWPORT_HAS_VALUE)?;
        if has_value {
            let x = self.get(ffi::RenderStateData::CURSOR_VIEWPORT_X)?;
            let y = self.get(ffi::RenderStateData::CURSOR_VIEWPORT_Y)?;
            let at_wide_tail = self.get(ffi::RenderStateData::CURSOR_VIEWPORT_WIDE_TAIL)?;
            Ok(Some(CursorViewport { x, y, at_wide_tail }))
        } else {
            Ok(None)
        }
    }

    /// All cursor state in one call.
    ///
    /// This is equivalent to the individual `cursor_*` getters, but needs a
    /// single query instead of one per property.
    pub fn cursor(&self) -> Result<Cursor> {
        let mut raw = ffi::sized!(ffi::RenderStateCursor);
        from_result(unsafe {
            ffi::ghostty_render_state_get(
                self.0.0.as_raw(),
                ffi::RenderStateData::CURSOR,
                (&raw mut raw).cast(),
            )
        })?;
        Ok(Cursor {
            // The viewport fields are only meaningful when
            // `viewport_has_value` is set. `then_some` still reads them, which
            // is fine: `sized!` zero-initializes the struct, and libghostty
            // leaves them alone when the cursor isn't in the viewport.
            viewport: raw.viewport_has_value.then_some(CursorViewport {
                x: raw.viewport_x,
                y: raw.viewport_y,
                at_wide_tail: raw.wide_tail,
            }),
            visible: raw.visible,
            blinking: raw.blinking,
            password_input: raw.password_input,
            visual_style: raw
                .visual_style
                .try_into()
                .map_err(|_| Error::InvalidValue)?,
        })
    }

    /// Mark all dirty render-state data as consumed.
    ///
    /// This sets the global [dirty state](Self::dirty) to [`Dirty::Clean`] and
    /// clears every per-row dirty flag. It is idempotent and does not modify
    /// cell contents or dirty state owned by the terminal. Call this only
    /// after a complete frame has been rendered successfully; partial
    /// consumers should use [`Self::set_dirty`] and [`RowIteration::set_dirty`]
    /// instead.
    pub fn clean(&self) -> Result<()> {
        from_result(unsafe { ffi::ghostty_render_state_clean(self.0.0.as_raw()) })
    }

    /// Get the current color information from a render state.
    pub fn colors(&self) -> Result<Colors> {
        let mut colors = ffi::sized!(ffi::RenderStateColors);
        let result = unsafe {
            ffi::ghostty_render_state_get(
                self.0.0.as_raw(),
                ffi::RenderStateData::COLORS,
                (&raw mut colors).cast(),
            )
        };
        from_result(result)?;

        Ok(Colors {
            background: colors.background.into(),
            foreground: colors.foreground.into(),
            cursor: if colors.cursor_has_value {
                Some(colors.cursor.into())
            } else {
                None
            },
            palette: colors.palette.map(Into::into),
        })
    }

    /// A value that changes whenever an update changes the placements that an
    /// [`ImageIterator`] returns.
    ///
    /// If it matches the value from the last time you read the iterator, the
    /// placements are the same and you don't need to read them again. See
    /// [Knowing when images changed](RenderState#knowing-when-images-changed)
    /// for an example and for exactly when the value changes.
    ///
    /// The value belongs to the whole list. To know when the pixels of a
    /// single image changed, use [`ImageIteration::image_generation`].
    ///
    /// Only compare the value for equality, and only with the last value you
    /// read from the same render state. It is always 0 when libghostty is
    /// built without Kitty graphics support, which this crate's
    /// `kitty-graphics` feature doesn't control.
    pub fn image_placements_generation(&self) -> Result<u64> {
        self.get(ffi::RenderStateData::IMAGE_PLACEMENTS_GENERATION)
    }

    /// Set dirty state.
    pub fn set_dirty(&self, dirty: Dirty) -> Result<()> {
        self.set(
            ffi::RenderStateOption::DIRTY,
            &(dirty as ffi::RenderStateDirty::Type),
        )
    }
}

impl<'alloc> RowIterator<'alloc> {
    /// Create a new row iterator instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new cell iterator instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::RenderStateRowIterator = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_render_state_row_iterator_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    /// Update the row iterator for a snapshot of the render state,
    /// returning a new row iteration.
    ///
    /// The iteration borrows the snapshot, so it cannot outlive it:
    ///
    /// ```compile_fail,E0505
    /// use libghostty_vt::{Terminal, RenderState, render::RowIterator};
    /// let terminal = Terminal::new(8, 2).unwrap();
    /// let mut state = RenderState::new().unwrap();
    /// let snapshot = state.update(&terminal).unwrap();
    /// let mut rows = RowIterator::new().unwrap();
    /// let mut iteration = rows.update(&snapshot).unwrap();
    /// drop(snapshot); // Iteration still borrows its owning snapshot.
    /// iteration.next();
    /// ```
    pub fn update<'s>(
        &'s mut self,
        snapshot: &'s Snapshot<'alloc, '_>,
    ) -> Result<RowIteration<'alloc, 's>> {
        let result = unsafe {
            ffi::ghostty_render_state_get(
                snapshot.0.0.as_raw(),
                ffi::RenderStateData::ROW_ITERATOR,
                std::ptr::from_mut(&mut self.0.ptr).cast(),
            )
        };
        from_result(result)?;

        Ok(RowIteration {
            iter: self,
            _phan: PhantomData,
        })
    }
}

impl Drop for RowIterator<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_render_state_row_iterator_free(self.0.as_raw()) }
    }
}

impl<'s> RowIteration<'_, 's> {
    /// The raw cell values for the current row, one per column.
    ///
    /// This is identical to querying [`CellIteration::raw_cell`] for each
    /// cell, and is the bulk alternative to iterating cells one at a time.
    ///
    /// The values are only valid as long as the underlying render state is
    /// not updated, so they borrow what the row iteration borrows (the
    /// snapshot and the row iterator) rather than this row: the iteration may
    /// keep advancing while they are in use.
    ///
    /// ```compile_fail,E0505
    /// use libghostty_vt::{RenderState, Terminal, render::RowIterator};
    /// let terminal = Terminal::new(8, 2).unwrap();
    /// let mut state = RenderState::new().unwrap();
    /// let snapshot = state.update(&terminal).unwrap();
    /// let mut rows = RowIterator::new().unwrap();
    /// let mut iteration = rows.update(&snapshot).unwrap();
    /// let cells = iteration.next().unwrap().cells_raw().unwrap();
    /// drop(snapshot); // The cells still borrow the snapshot.
    /// cells.count();
    /// ```
    pub fn cells_raw(&self) -> Result<impl ExactSizeIterator<Item = Cell> + use<'s>> {
        let view: ffi::CellsView = self.get(ffi::RenderStateRowData::CELLS_RAW)?;
        // An empty row's view comes from an empty Zig slice, whose pointer
        // need not be dereferenceable or aligned, so never build a slice
        // from it.
        let cells: &'s [ffi::Cell] = if view.len == 0 {
            &[]
        } else {
            // SAFETY: libghostty keeps the view valid until the render state
            // is updated, which the snapshot borrow `'s` rules out. The only
            // writes possible meanwhile are to dirty flags (`set_dirty`,
            // `clean`), which live outside the cells. Each element is the
            // cell's `u64` value (`ffi::Cell`), which `Cell` wraps by value.
            unsafe { std::slice::from_raw_parts(view.ptr, view.len) }
        };
        Ok(cells.iter().copied().map(Cell))
    }
}

impl RowIteration<'_, '_> {
    /// Move a row iteration to the next row requiring a redraw.
    ///
    /// If the global dirty state is [`Dirty::Clean`], this returns `None`. If
    /// it is [`Dirty::Partial`], clean rows are skipped. If it is
    /// [`Dirty::Full`], every remaining row is returned regardless of its
    /// per-row dirty flag. Rows are returned in ascending viewport order,
    /// together with their position in the iteration. This does not clear any
    /// dirty state.
    ///
    /// Without [overscan](RenderState#overscan), the position is the viewport
    /// y. With overscan, it counts from the highest captured row, so use
    /// [`Self::viewport_y`] to place the row.
    pub fn next_dirty(&mut self) -> Option<(u16, &Self)> {
        let mut y = 0;
        // The receiver is evaluated before the arguments, so `y` is read only
        // after libghostty has written it.
        unsafe {
            ffi::ghostty_render_state_row_iterator_next_dirty(self.iter.0.as_raw(), &raw mut y)
        }
        .then_some((y, self))
    }

    /// Move a row iteration to the next row.
    ///
    /// Returns `Some(row)` if the iteration moved successfully and row
    /// data is available to read at the new position using `row`.
    #[expect(
        clippy::should_implement_trait,
        reason = "lending `next` cannot implement trait"
    )]
    pub fn next(&mut self) -> Option<&Self> {
        if unsafe { ffi::ghostty_render_state_row_iterator_next(self.iter.0.as_raw()) } {
            Some(self)
        } else {
            None
        }
    }

    fn get<T>(&self, tag: ffi::RenderStateRowData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_row_get(self.iter.0.as_raw(), tag, value.as_mut_ptr().cast())
        };
        // Since we manually model every possible query, this should never fail.
        from_result(result)?;
        // SAFETY: Value should be initialized after successful call.
        Ok(unsafe { value.assume_init() })
    }

    fn set<T>(&self, tag: ffi::RenderStateRowOption::Type, value: &T) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_render_state_row_set(
                self.iter.0.as_raw(),
                tag,
                std::ptr::from_ref(value).cast(),
            )
        };
        from_result(result)
    }

    /// Whether the current row is dirty.
    pub fn dirty(&self) -> Result<bool> {
        self.get(ffi::RenderStateRowData::DIRTY)
    }

    /// The row's [identity](RenderState#row-identity) across updates.
    ///
    /// This works with or without [overscan](RenderState#overscan).
    pub fn id(&self) -> Result<RowId> {
        self.get::<ffi::RenderStateRowId>(ffi::RenderStateRowData::ID)
            .map(|id| RowId(id.bits))
    }

    /// The row's position relative to the top of the viewport.
    ///
    /// Viewport rows are 0 through [`rows`](Snapshot::rows) - 1.
    /// [Overscan](RenderState#overscan) rows above the viewport are negative,
    /// and overscan rows below it start at [`rows`](Snapshot::rows). Without
    /// overscan, this equals the position reported by [`Self::next_dirty`].
    pub fn viewport_y(&self) -> Result<i32> {
        self.get(ffi::RenderStateRowData::VIEWPORT_Y)
    }

    /// The raw row value.
    pub fn raw_row(&self) -> Result<Row> {
        self.get(ffi::RenderStateRowData::RAW).map(Row)
    }

    /// Set dirty state for the current row.
    pub fn set_dirty(&self, dirty: bool) -> Result<()> {
        self.set(ffi::RenderStateRowOption::DIRTY, &dirty)
    }

    /// Row-local selected cell range.
    pub fn selection(&self) -> Result<Option<RowSelection>> {
        let mut value = ffi::sized!(RowSelection);
        let result = unsafe {
            ffi::ghostty_render_state_row_get(
                self.iter.0.as_raw(),
                ffi::RenderStateRowData::SELECTION,
                std::ptr::from_mut(&mut value).cast(),
            )
        };
        // Since we manually model every possible query, this should never fail.
        // SAFETY: Value should be initialized after successful call.
        from_optional_result(result, value)
    }
}

impl<'alloc> CellIterator<'alloc> {
    /// Create a new cell iterator instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new cell iterator instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::RenderStateRowCells = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_render_state_row_cells_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    /// Update the cell iterator for a new row iteration,
    /// returning a new cell iteration.
    ///
    /// The iteration borrows the row, so it cannot outlive it:
    ///
    /// ```compile_fail,E0505
    /// use libghostty_vt::{
    ///     RenderState, Terminal,
    ///     render::{CellIterator, RowIterator},
    /// };
    /// let terminal = Terminal::new(8, 2).unwrap();
    /// let mut state = RenderState::new().unwrap();
    /// let snapshot = state.update(&terminal).unwrap();
    /// let mut rows = RowIterator::new().unwrap();
    /// let mut row = rows.update(&snapshot).unwrap();
    /// row.next();
    /// let mut cells = CellIterator::new().unwrap();
    /// let mut iteration = cells.update(&row).unwrap();
    /// drop(row); // Iteration still borrows its owning row.
    /// iteration.next();
    /// ```
    pub fn update<'s>(
        &'s mut self,
        row: &'s RowIteration<'alloc, '_>,
    ) -> Result<CellIteration<'alloc, 's>> {
        let result = unsafe {
            ffi::ghostty_render_state_row_get(
                row.iter.0.as_raw(),
                ffi::RenderStateRowData::CELLS,
                std::ptr::from_mut(&mut self.0.ptr).cast(),
            )
        };
        from_result(result)?;

        Ok(CellIteration {
            iter: self,
            _phan: PhantomData,
        })
    }
}

impl Drop for CellIterator<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_render_state_row_cells_free(self.0.as_raw()) }
    }
}

impl CellIteration<'_, '_> {
    /// Move a cell iteration to the next cell.
    ///
    /// Returns `Some(cell)` if the iteration moved successfully and cell
    /// data is available to read at the new position using `cell`.
    #[expect(
        clippy::should_implement_trait,
        reason = "lending `next` cannot implement trait"
    )]
    pub fn next(&mut self) -> Option<&Self> {
        if unsafe { ffi::ghostty_render_state_row_cells_next(self.iter.0.as_raw()) } {
            Some(self)
        } else {
            None
        }
    }

    /// Move a cell iteration to a specific column.
    ///
    /// Positions the iteration at the given x (column) index so that
    /// subsequent reads return data for that cell.
    pub fn select(&mut self, x: u16) -> Result<()> {
        let result = unsafe { ffi::ghostty_render_state_row_cells_select(self.iter.0.as_raw(), x) };
        from_result(result)
    }

    fn get<T>(&self, tag: ffi::RenderStateRowCellsData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_row_cells_get(
                self.iter.0.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        from_result(result)?;
        // SAFETY: Value should be initialized after successful call.
        Ok(unsafe { value.assume_init() })
    }

    /// The raw cell value.
    pub fn raw_cell(&self) -> Result<Cell> {
        self.get(ffi::RenderStateRowCellsData::RAW).map(Cell)
    }

    /// The style for the current cell.
    pub fn style(&self) -> Result<Style> {
        let mut value = ffi::sized!(ffi::Style);
        let result = unsafe {
            ffi::ghostty_render_state_row_cells_get(
                self.iter.0.as_raw(),
                ffi::RenderStateRowCellsData::STYLE,
                std::ptr::from_mut(&mut value).cast(),
            )
        };
        from_result(result)?;
        Style::try_from(value)
    }

    /// The resolved foreground color of the cell.
    ///
    /// Resolves palette indices through the palette. Bold color handling
    /// is not applied; the caller should handle bold styling separately.
    ///
    /// Returns `None` if the cell has no explicit foreground color, in which
    /// case the caller should use whatever default foreground color it want
    /// (e.g. the terminal foreground).
    pub fn fg_color(&self) -> Result<Option<RgbColor>> {
        let res = self.get::<ffi::ColorRgb>(ffi::RenderStateRowCellsData::FG_COLOR);
        match res {
            Ok(o) => Ok(Some(o.into())),
            Err(Error::InvalidValue) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The resolved background color of the cell.
    ///
    /// Flattens the three possible sources: [`Cell::bg_color_rgb`],
    /// [`Cell::bg_color_palette`] (looked up in the palette), or the
    /// style's [`bg_color`][Style::bg_color].
    ///
    /// Returns `None` if the cell has no background color, in which case the
    /// caller should use whatever default background color it wants
    /// (e.g. the terminal background).
    pub fn bg_color(&self) -> Result<Option<RgbColor>> {
        let res = self.get::<ffi::ColorRgb>(ffi::RenderStateRowCellsData::BG_COLOR);
        match res {
            Ok(o) => Ok(Some(o.into())),
            Err(Error::InvalidValue) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get the grapheme codepoints.
    ///
    /// The base codepoint is placed first, followed by any extra codepoints.
    pub fn graphemes(&self) -> Result<Vec<char>> {
        let len = self.graphemes_len()? as usize;
        let mut graphemes = vec!['\0'; len];
        self.graphemes_buf(&mut graphemes)?;
        Ok(graphemes)
    }

    /// The total number of grapheme codepoints including the base codepoint.
    ///
    /// Returns 0 if the cell has no text.
    pub fn graphemes_len(&self) -> Result<u32> {
        self.get(ffi::RenderStateRowCellsData::GRAPHEMES_LEN)
    }

    /// Write grapheme codepoints into a caller-provided buffer.
    ///
    /// The buffer must be at least [`CellIteration::graphemes_len`] elements.
    /// The base codepoint is written first, followed by any extra codepoints.
    ///
    /// If the buffer is too small, the function returns
    /// `Err(Error::OutOfSpace { required })` where `required` is the
    /// required number of codepoints, and `buf` is left unchanged. The
    /// caller can then retry with a sufficiently sized buffer.
    pub fn graphemes_buf(&self, buf: &mut [char]) -> Result<()> {
        // Upstream takes no buffer length and writes unchecked, so the size
        // check has to happen here: without it a short buffer from safe code
        // would be an out-of-bounds write.
        let required = self.graphemes_len()? as usize;
        if buf.len() < required {
            return Err(Error::OutOfSpace { required });
        }
        // SAFETY: GRAPHEMES_BUF writes exactly `graphemes_len` elements for
        // the current cell, and we just checked `buf` holds at least that
        // many, so every write stays in bounds. Nothing between the length
        // query and this call can move the iterator, since we hold `&self`.
        let result = unsafe {
            ffi::ghostty_render_state_row_cells_get(
                self.iter.0.as_raw(),
                ffi::RenderStateRowCellsData::GRAPHEMES_BUF,
                buf.as_mut_ptr().cast(),
            )
        };
        from_result(result)
    }

    /// Replace the contents of `buf` with the current cell's full grapheme
    /// cluster, encoded as UTF-8.
    ///
    /// The base codepoint is encoded first, followed by any extra grapheme
    /// codepoints. A cell without text leaves `buf` empty.
    ///
    /// `buf`'s allocation is reused and only grows if the cluster does not
    /// fit, so one buffer can serve every cell of a frame. On error, `buf` is
    /// left empty.
    pub fn graphemes_utf8(&self, buf: &mut String) -> Result<()> {
        // libghostty writes from the start of the buffer, so start empty.
        buf.clear();
        // SAFETY: The length is only ever set, on success, to what libghostty
        // reports having written, and on success it has encoded every
        // codepoint of the cluster as UTF-8, so the string stays valid UTF-8.
        // A failed call can leave encoded bytes behind (a surrogate is only
        // rejected while encoding), but they stay past the length, which is
        // still zero.
        let bytes = unsafe { buf.as_mut_vec() };
        loop {
            let mut cbuf = ffi::Buffer {
                ptr: bytes.as_mut_ptr(),
                cap: bytes.capacity(),
                len: 0,
            };
            let result = unsafe {
                ffi::ghostty_render_state_row_cells_get(
                    self.iter.0.as_raw(),
                    ffi::RenderStateRowCellsData::GRAPHEMES_UTF8,
                    std::ptr::from_mut(&mut cbuf).cast(),
                )
            };
            match result {
                ffi::Result::SUCCESS => {
                    // SAFETY: libghostty wrote `cbuf.len <= cap` bytes.
                    unsafe { bytes.set_len(cbuf.len) };
                    return Ok(());
                }
                // `cbuf.len` is the size needed, and `bytes` is empty.
                ffi::Result::OUT_OF_SPACE => bytes.reserve(cbuf.len),
                ffi::Result::OUT_OF_MEMORY => return Err(Error::OutOfMemory),
                _ => return Err(Error::InvalidValue),
            }
        }
    }

    /// Whether the cell is contained within the current selection.
    ///
    /// This returns true when the cell's column is within the current row's
    /// row-local selection range, and false otherwise. Rendering policy for
    /// selected cells (colors, inversion, etc.) is left to the caller.
    ///
    /// Renderers that can draw cells in spans may be more efficient calling
    /// [`RowIteration::selection`] once per row and applying that range
    /// directly, avoiding one C API call per cell for selection state.
    pub fn is_selected(&self) -> Result<bool> {
        self.get(ffi::RenderStateRowCellsData::SELECTED)
    }

    /// Whether the cell has any explicit styling.
    ///
    /// This is equivalent to querying the raw cell's [`Cell::has_styling`]
    /// value, but avoids materializing the raw [`Cell`] for renderers that
    /// only need to know whether fetching the full style is necessary.
    pub fn has_styling(&self) -> Result<bool> {
        self.get(ffi::RenderStateRowCellsData::HAS_STYLING)
    }
}

impl<'alloc> ImageIterator<'alloc> {
    /// Create a new image iterator instance.
    ///
    /// The iterator starts out empty. Fill it with [`Self::update`].
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new image iterator instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::RenderStateImageIterator = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_render_state_image_iterator_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    /// Populate the image iterator with the image placements found by the
    /// last update, returning a new image iteration.
    ///
    /// See [Images](RenderState#images) for which placements are included.
    ///
    /// The iteration reads from the render state that filled it, so it
    /// borrows the snapshot and cannot outlive it. After the next update,
    /// fill the iterator again before using it:
    ///
    /// ```compile_fail,E0505
    /// use libghostty_vt::{RenderState, Terminal, render::ImageIterator};
    /// let terminal = Terminal::new(8, 2).unwrap();
    /// let mut state = RenderState::new().unwrap();
    /// let snapshot = state.update(&terminal).unwrap();
    /// let mut images = ImageIterator::new().unwrap();
    /// let mut iteration = images.update(&snapshot).unwrap();
    /// drop(snapshot); // Iteration still borrows its owning snapshot.
    /// iteration.next();
    /// ```
    pub fn update<'s>(
        &'s mut self,
        snapshot: &'s Snapshot<'alloc, '_>,
    ) -> Result<ImageIteration<'alloc, 's>> {
        // Unlike most data keys, `out` points to an existing iterator handle,
        // which libghostty refills in place.
        let result = unsafe {
            ffi::ghostty_render_state_get(
                snapshot.0.0.as_raw(),
                ffi::RenderStateData::IMAGE_ITERATOR,
                std::ptr::from_mut(&mut self.0.ptr).cast(),
            )
        };
        from_result(result)?;

        Ok(ImageIteration {
            iter: self,
            _phan: PhantomData,
        })
    }
}

impl Drop for ImageIterator<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_render_state_image_iterator_free(self.0.as_raw()) }
    }
}

// `ghostty_render_state_image_get_multi` is deliberately not wrapped, like
// every other `*_get_multi` in this crate. It is the same as calling
// `ghostty_render_state_image_get` once per key and only saves calls, and
// each of those calls is a cheap direct FFI call from Rust. A safe wrapper
// over its type-erased `void**` out-parameters would just duplicate the typed
// getters below.
impl ImageIteration<'_, '_> {
    /// Move an image iteration to the next image placement.
    ///
    /// Call this before reading the first placement, and keep calling it
    /// until it returns `None`.
    ///
    /// Returns `Some(placement)` if the iteration moved to a placement, and
    /// `None` if there are no more placements.
    ///
    /// The order of placements is not the order to draw them in. Sort them
    /// by [layer](Self::layer) and then by [z-index](Self::z) first.
    ///
    /// Only placements with something to draw are returned. For example,
    /// placements of an image the terminal hasn't fully received yet are
    /// skipped.
    ///
    /// The iteration reads from the render state that filled it. After the
    /// next update of that render state, fill the iterator again with
    /// [`ImageIterator::update`] before using it.
    #[expect(
        clippy::should_implement_trait,
        reason = "lending `next` cannot implement trait"
    )]
    pub fn next(&mut self) -> Option<&Self> {
        if unsafe { ffi::ghostty_render_state_image_iterator_next(self.iter.0.as_raw()) } {
            Some(self)
        } else {
            None
        }
    }

    fn get<T>(&self, tag: ffi::RenderStateImageData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_image_get(
                self.iter.0.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        // Since we manually model every possible query, this only fails with
        // `InvalidValue` when the iteration is not on a placement (before the
        // first `next` or after the last one).
        from_result(result)?;
        // SAFETY: Value should be initialized after successful call.
        Ok(unsafe { value.assume_init() })
    }

    /// Like [`Self::get`], for keys that only apply to one protocol.
    /// Reading one for a placement from another protocol gives `Ok(None)`.
    fn get_optional<T>(&self, tag: ffi::RenderStateImageData::Type) -> Result<Option<T>> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_image_get(
                self.iter.0.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        from_optional_result_uninit(result, value)
    }

    /// The protocol the image came from.
    ///
    /// New protocols may be added in later versions. Skip placements with a
    /// protocol you don't support. A protocol that this crate doesn't know
    /// yet gives [`Error::InvalidValue`].
    pub fn protocol(&self) -> Result<ImageProtocol> {
        self.get::<ffi::ImageProtocol::Type>(ffi::RenderStateImageData::PROTOCOL)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    /// The ID of the image to draw.
    ///
    /// IDs are only unique within one protocol, so look the image up in the
    /// storage of its protocol. For Kitty graphics, use
    // Only link to the `kitty` wrappers when the `kitty-graphics` feature
    // provides them. Otherwise the link would be broken.
    #[cfg_attr(
        feature = "kitty-graphics",
        doc = "[`Graphics::image`](crate::kitty::graphics::Graphics::image)."
    )]
    #[cfg_attr(not(feature = "kitty-graphics"), doc = "`Graphics::image`.")]
    pub fn image_id(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::IMAGE_ID)
    }

    /// The generation of the image's contents.
    ///
    /// It changes whenever the image's pixels change, including when an
    // Only link to the `kitty` wrappers when the `kitty-graphics` feature
    // provides them. Otherwise the link would be broken.
    #[cfg_attr(
        feature = "kitty-graphics",
        doc = "[animation](crate::kitty::graphics#animation) moves to a new frame. Use it as part of your texture cache"
    )]
    #[cfg_attr(
        not(feature = "kitty-graphics"),
        doc = "animation moves to a new frame. Use it as part of your texture cache"
    )]
    /// key, with the [protocol](Self::protocol) and [image ID](Self::image_id):
    /// if it matches the texture you have, you don't need to read the pixels
    /// again. For Kitty graphics, this is the same value as
    // Only link to the `kitty` wrappers when the `kitty-graphics` feature
    // provides them. Otherwise the link would be broken.
    #[cfg_attr(
        feature = "kitty-graphics",
        doc = "[`Image::generation`](crate::kitty::graphics::Image::generation)."
    )]
    #[cfg_attr(not(feature = "kitty-graphics"), doc = "`Image::generation`.")]
    ///
    /// To know when the list of placements as a whole changed, use
    /// [`Snapshot::image_placements_generation`].
    pub fn image_generation(&self) -> Result<u64> {
        self.get(ffi::RenderStateImageData::IMAGE_GENERATION)
    }

    /// The layer to draw the image in.
    pub fn layer(&self) -> Result<ImageLayer> {
        self.get::<ffi::RenderStateImageLayer::Type>(ffi::RenderStateImageData::LAYER)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    /// The z-index, which orders images within a [layer](Self::layer).
    ///
    /// Images with a lower z-index are drawn first. The value comes from the
    /// image's protocol. For Kitty graphics, it is the z-index the program
    /// asked for, and virtual placements are always -1.
    pub fn z(&self) -> Result<i32> {
        self.get(ffi::RenderStateImageData::Z)
    }

    /// The column of the top-left cell, where 0 is the leftmost column of the
    /// viewport.
    ///
    /// Fixed and relative placements are reported at their full size, so they
    /// can start to the left of the viewport, which gives a negative value.
    /// Virtual placements never do. See
    /// [Images partly outside the captured rows](RenderState#images-partly-outside-the-captured-rows).
    pub fn viewport_col(&self) -> Result<i32> {
        self.get(ffi::RenderStateImageData::VIEWPORT_COL)
    }

    /// The row of the top-left cell, where 0 is the top row of the viewport.
    ///
    /// A placement in an [overscan](RenderState#overscan) row above the
    /// viewport has a negative row, the same as [`RowIteration::viewport_y`].
    /// Fixed and relative placements are reported at their full size, so they
    /// can also start above the first overscan row. Virtual placements are
    /// always inside the captured rows. See
    /// [Images partly outside the captured rows](RenderState#images-partly-outside-the-captured-rows).
    pub fn viewport_row(&self) -> Result<i32> {
        self.get(ffi::RenderStateImageData::VIEWPORT_ROW)
    }

    /// The number of columns of cells the placement covers.
    pub fn grid_cols(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::GRID_COLS)
    }

    /// The number of rows of cells the placement covers.
    pub fn grid_rows(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::GRID_ROWS)
    }

    /// How far the image starts from the left edge of the top-left cell, in
    /// pixels.
    ///
    /// For a virtual placement, an image that is narrower than its cells is
    /// centered, so this can be larger than a cell is wide. For fixed and
    /// relative placements, this is the offset the program asked for, limited
    /// to less than one cell.
    pub fn cell_offset_x(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::CELL_OFFSET_X)
    }

    /// How far the image starts from the top edge of the top-left cell, in
    /// pixels.
    ///
    /// For a virtual placement, an image that is shorter than its cells is
    /// centered, so this can be larger than a cell is tall. For fixed and
    /// relative placements, this is the offset the program asked for, limited
    /// to less than one cell.
    pub fn cell_offset_y(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::CELL_OFFSET_Y)
    }

    /// The width to draw the image at, in pixels.
    pub fn pixel_width(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::PIXEL_WIDTH)
    }

    /// The height to draw the image at, in pixels.
    pub fn pixel_height(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::PIXEL_HEIGHT)
    }

    /// The left edge of the part of the image to draw, in image pixels.
    pub fn source_x(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::SOURCE_X)
    }

    /// The top edge of the part of the image to draw, in image pixels.
    pub fn source_y(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::SOURCE_Y)
    }

    /// The width of the part of the image to draw, in image pixels.
    ///
    /// Scale it to [`Self::pixel_width`] when drawing.
    pub fn source_width(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::SOURCE_WIDTH)
    }

    /// The height of the part of the image to draw, in image pixels.
    ///
    /// Scale it to [`Self::pixel_height`] when drawing.
    pub fn source_height(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::SOURCE_HEIGHT)
    }

    /// Kitty graphics only: the placement ID, or 0 if there isn't one.
    ///
    /// For a virtual placement, this is the placement ID that the placeholder
    /// cells ask for. Cells that don't ask for one show the image's virtual
    /// placement, whichever it is, and report 0. For a fixed or relative
    /// placement, this is the ID the program gave it, or 0 if it didn't give
    /// one.
    ///
    /// Returns `None` if the placement is from another
    /// [protocol](Self::protocol).
    pub fn kitty_placement_id(&self) -> Result<Option<u32>> {
        self.get_optional(ffi::RenderStateImageData::KITTY_PLACEMENT_ID)
    }

    /// Kitty graphics only: the kind of placement.
    ///
    /// Returns `None` if the placement is from another
    /// [protocol](Self::protocol). New kinds may be added in later versions.
    /// A kind that this crate doesn't know yet gives [`Error::InvalidValue`].
    pub fn kitty_kind(&self) -> Result<Option<KittyPlacementKind>> {
        self.get_optional::<ffi::RenderStateKittyPlacementKind::Type>(
            ffi::RenderStateImageData::KITTY_KIND,
        )?
        .map(|v| v.try_into().map_err(|_| Error::InvalidValue))
        .transpose()
    }
}

//---------------------------
// Auxiliary types
//---------------------------

/// Cursor viewport position information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorViewport {
    /// Cursor viewport x position in cells.
    pub x: u16,
    /// Cursor viewport y position in cells.
    pub y: u16,
    /// Whether the cursor is on the tail of a wide character.
    pub at_wide_tail: bool,
}

/// Render-state cursor information, as returned by [`Snapshot::cursor`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    /// The cursor position if the cursor is visible within the viewport.
    pub viewport: Option<CursorViewport>,
    /// Whether the cursor is visible based on terminal modes.
    pub visible: bool,
    /// Whether the cursor should blink based on terminal modes.
    pub blinking: bool,
    /// Whether the cursor is at a password input field.
    pub password_input: bool,
    /// The visual style of the cursor.
    pub visual_style: CursorVisualStyle,
}

/// The [identity](RenderState#row-identity) of a row across render state
/// updates, as returned by [`RowIteration::id`].
///
/// Treat this value as opaque: two ids are the same row when they are equal,
/// and no other comparison or interpretation is meaningful. The contents may
/// change between library versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RowId([u64; 2]);

/// Render-state color information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colors {
    /// The default/current background color for the render state.
    pub background: RgbColor,
    /// The default/current foreground color for the render state.
    pub foreground: RgbColor,
    /// The cursor color which may be explicitly set by terminal state.
    pub cursor: Option<RgbColor>,
    /// The active 256-color palette for this render state.
    pub palette: [RgbColor; 256],
}

/// Dirty state of a render state after update.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
pub enum Dirty {
    /// Not dirty at all; rendering can be skipped.
    Clean = ffi::RenderStateDirty::FALSE,
    /// Some rows changed; renderer can redraw incrementally.
    Partial = ffi::RenderStateDirty::PARTIAL,
    /// Global state changed; renderer should redraw everything.
    Full = ffi::RenderStateDirty::FULL,
}

/// Visual style of the cursor.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum CursorVisualStyle {
    /// Bar cursor (DECSCUSR 5, 6).
    Bar = ffi::RenderStateCursorVisualStyle::BAR,
    /// Block cursor (DECSCUSR 1, 2).
    Block = ffi::RenderStateCursorVisualStyle::BLOCK,
    /// Underline cursor (DECSCUSR 3, 4).
    Underline = ffi::RenderStateCursorVisualStyle::UNDERLINE,
    /// Hollow block cursor.
    BlockHollow = ffi::RenderStateCursorVisualStyle::BLOCK_HOLLOW,
}

/// The image protocols that a terminal supports.
///
/// The render state uses this to say which protocol each image placement came
/// from (see [`ImageIteration::protocol`]). Use it to decide where to read the
/// image's pixels.
///
/// New protocols may be added in later versions. Skip placements with a
/// protocol you don't support. [`ImageIteration::protocol`] returns
/// [`Error::InvalidValue`] for a protocol that this crate doesn't know.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, int_enum::IntEnum)]
#[non_exhaustive]
pub enum ImageProtocol {
    /// The Kitty graphics protocol. Read the image with
    // Only link to the `kitty` wrappers when the `kitty-graphics` feature
    // provides them. Otherwise the link would be broken.
    #[cfg_attr(
        feature = "kitty-graphics",
        doc = "[`Graphics::image`](crate::kitty::graphics::Graphics::image)."
    )]
    #[cfg_attr(not(feature = "kitty-graphics"), doc = "`Graphics::image`.")]
    Kitty = ffi::ImageProtocol::KITTY,
}

/// Where an image is drawn relative to the terminal's text.
///
/// Draw the layers in this order: every image below backgrounds, then cell
/// backgrounds, then images below text, then text, then images above text.
/// Within a layer, draw images in order of their [z-index](ImageIteration::z),
/// lowest first.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, int_enum::IntEnum)]
pub enum ImageLayer {
    /// Below the cell background colors.
    BelowBackground = ffi::RenderStateImageLayer::BELOW_BACKGROUND,
    /// Above the cell background colors and below the text.
    BelowText = ffi::RenderStateImageLayer::BELOW_TEXT,
    /// Above the text.
    AboveText = ffi::RenderStateImageLayer::ABOVE_TEXT,
}

/// The kinds of Kitty graphics placements in a render state's image list.
///
/// Read with [`ImageIteration::kitty_kind`]. Most renderers can draw every
/// kind the same way. The kind is useful when you need to tell them apart,
/// for example for debugging.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, int_enum::IntEnum)]
#[non_exhaustive]
pub enum KittyPlacementKind {
    /// Part of a virtual placement: one run of neighboring placeholder cells
    /// in a single row. It is always one row tall.
    Virtual = ffi::RenderStateKittyPlacementKind::VIRTUAL,
    /// A placement that a program positioned relative to another placement,
    /// its parent.
    ///
    /// Kitty graphics lets a program attach one image to another so that they
    /// move together. The parent is either a virtual or a fixed placement,
    /// possibly through a chain of other relative placements.
    ///
    /// With a virtual parent, the position depends on the placeholder cells.
    /// It is measured from the top-left corner of the parent's cells on
    /// screen. That is the leftmost column and the topmost row that any of its
    /// cells use, which matches Kitty. If none of those cells are in the
    /// captured rows (the viewport plus any overscan), the relative placement
    /// is not included.
    ///
    /// With a fixed parent, the position is measured from the parent's cell.
    /// The relative placement is included at its full size when any part of
    /// it is in the captured rows, even if the parent isn't, so it may need to
    /// be clipped when drawn.
    ///
    /// See [Kitty graphics relative placements](RenderState#kitty-graphics-relative-placements).
    Relative = ffi::RenderStateKittyPlacementKind::RELATIVE,
    /// A placement at a cell on the screen.
    ///
    /// It scrolls with the text, like a line of output would. Its z-index is
    /// the one the program chose, so it can be in any layer.
    ///
    /// It is included at its full size when any part of it is in the captured
    /// rows, so it may need to be clipped when drawn. See
    /// [Images partly outside the captured rows](RenderState#images-partly-outside-the-captured-rows).
    Fixed = ffi::RenderStateKittyPlacementKind::FIXED,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::Terminal;

    /// Guards the `set_dirty` → `update` → `dirty()` round-trip. If
    /// `Snapshot::set(value: &T)` calls `from_ref(&value)`, the result has
    /// type `*const &T` (a pointer to the local reference), not `*const T`.
    /// C reads stack-address bytes into the dirty field, the next `update`
    /// propagates them, and `dirty()` fails enum decoding.
    #[test]
    fn dirty_decodes_after_set_dirty_then_update() {
        let terminal = Terminal::new(8, 3).unwrap();
        let mut state = RenderState::new().unwrap();

        state
            .update(&terminal)
            .unwrap()
            .set_dirty(Dirty::Clean)
            .unwrap();

        assert!(state.update(&terminal).unwrap().dirty().is_ok());
    }

    /// Build the expected bulk cursor from the individual getters.
    fn cursor_from_getters(snapshot: &Snapshot<'_, '_>) -> Cursor {
        Cursor {
            viewport: snapshot.cursor_viewport().unwrap(),
            visible: snapshot.cursor_visible().unwrap(),
            blinking: snapshot.cursor_blinking().unwrap(),
            password_input: snapshot.cursor_password_input().unwrap(),
            visual_style: snapshot.cursor_visual_style().unwrap(),
        }
    }

    #[test]
    fn bulk_cursor_matches_individual_getters() {
        let mut terminal = Terminal::new(8, 2).unwrap();
        let mut state = RenderState::new().unwrap();

        // Default cursor after writing a narrow character.
        terminal.vt_write(b"hi");
        let snapshot = state.update(&terminal).unwrap();
        let cursor = snapshot.cursor().unwrap();
        assert_eq!(cursor, cursor_from_getters(&snapshot));
        assert_eq!(
            cursor.viewport,
            Some(CursorViewport {
                x: 2,
                y: 0,
                at_wide_tail: false
            })
        );

        // Hidden blinking bar cursor on the tail of a wide character.
        terminal.vt_write("\x1b[?25l\x1b[5 q\r\n中\x1b[2G".as_bytes());
        let snapshot = state.update(&terminal).unwrap();
        let cursor = snapshot.cursor().unwrap();
        assert_eq!(cursor, cursor_from_getters(&snapshot));
        assert!(!cursor.visible);
        assert_eq!(cursor.visual_style, CursorVisualStyle::Bar);
        assert_eq!(
            cursor.viewport,
            Some(CursorViewport {
                x: 1,
                y: 1,
                at_wide_tail: true
            })
        );

        // Scrolling the cursor out of the viewport leaves no position.
        terminal.vt_write(b"\r\n\r\n\r\n");
        terminal.scroll_viewport(crate::terminal::ScrollViewport::Top);
        let snapshot = state.update(&terminal).unwrap();
        let cursor = snapshot.cursor().unwrap();
        assert_eq!(cursor, cursor_from_getters(&snapshot));
        assert_eq!(cursor.viewport, None);
    }

    /// Collect the viewport rows returned by `next_dirty`.
    fn dirty_rows<'alloc>(
        rows: &mut RowIterator<'alloc>,
        snapshot: &Snapshot<'alloc, '_>,
    ) -> Vec<u16> {
        let mut iteration = rows.update(snapshot).unwrap();
        let mut ys = Vec::new();
        while let Some((y, _)) = iteration.next_dirty() {
            ys.push(y);
        }
        ys
    }

    #[test]
    fn next_dirty_follows_global_and_row_dirty_state() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        // Park the cursor on the row we write to below: moving the cursor
        // also dirties the row it leaves.
        terminal.vt_write(b"\x1b[2;1H");
        let mut state = RenderState::new().unwrap();
        let mut rows = RowIterator::new().unwrap();

        // The first update is fully dirty, so every row is returned.
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Full);
        assert_eq!(dirty_rows(&mut rows, &snapshot), [0, 1, 2]);

        // Once clean, nothing is returned.
        snapshot.clean().unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Clean);
        assert_eq!(dirty_rows(&mut rows, &snapshot), [] as [u16; 0]);
        // Cleaning is idempotent.
        snapshot.clean().unwrap();

        // Changing one row only makes that row dirty.
        terminal.vt_write(b"x");
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Partial);
        assert_eq!(dirty_rows(&mut rows, &snapshot), [1]);
    }

    /// Collect the viewport y of every row an iteration visits, along with
    /// the positions `next_dirty` reports for them.
    fn row_positions<'alloc>(
        rows: &mut RowIterator<'alloc>,
        snapshot: &Snapshot<'alloc, '_>,
    ) -> (Vec<i32>, Vec<u16>) {
        let mut viewport_ys = Vec::new();
        let mut iteration = rows.update(snapshot).unwrap();
        while let Some(row) = iteration.next() {
            viewport_ys.push(row.viewport_y().unwrap());
        }
        (viewport_ys, dirty_rows(rows, snapshot))
    }

    /// An overscan as `(above, below)`, since the FFI struct can't be compared.
    fn sides(overscan: Overscan) -> (u16, u16) {
        (overscan.above, overscan.below)
    }

    #[test]
    fn overscan_request_defaults_to_none_and_round_trips() {
        let mut state = RenderState::new().unwrap();
        assert_eq!(sides(state.overscan_request().unwrap()), (0, 0));

        let request = Overscan { above: 2, below: 1 };
        state.set_overscan(request).unwrap();
        assert_eq!(sides(state.overscan_request().unwrap()), (2, 1));
    }

    #[test]
    fn overscan_captures_only_rows_that_exist() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        for _ in 0..10 {
            terminal.vt_write(b"x\r\n");
        }
        let mut state = RenderState::new().unwrap();
        let mut rows = RowIterator::new().unwrap();

        // Without overscan, only the viewport is visited, and the position
        // from `next_dirty` is the viewport y.
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(sides(snapshot.overscan().unwrap()), (0, 0));
        assert_eq!(
            row_positions(&mut rows, &snapshot),
            (vec![0, 1, 2], vec![0, 1, 2])
        );

        // At the bottom, there is nothing below the viewport to capture.
        state.set_overscan(Overscan { above: 1, below: 1 }).unwrap();
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.rows().unwrap(), 3);
        assert_eq!(sides(snapshot.overscan().unwrap()), (1, 0));
        // `next_dirty` counts from the highest captured row instead.
        assert_eq!(
            row_positions(&mut rows, &snapshot),
            (vec![-1, 0, 1, 2], vec![0, 1, 2, 3])
        );

        // At the top, there is nothing above it.
        terminal.scroll_viewport(crate::terminal::ScrollViewport::Top);
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(sides(snapshot.overscan().unwrap()), (0, 1));
        assert_eq!(
            row_positions(&mut rows, &snapshot),
            (vec![0, 1, 2, 3], vec![0, 1, 2, 3])
        );
    }

    /// Collect the id of every row an iteration visits.
    fn row_ids<'alloc>(
        rows: &mut RowIterator<'alloc>,
        snapshot: &Snapshot<'alloc, '_>,
    ) -> Vec<RowId> {
        let mut ids = Vec::new();
        let mut iteration = rows.update(snapshot).unwrap();
        while let Some(row) = iteration.next() {
            ids.push(row.id().unwrap());
        }
        ids
    }

    #[test]
    fn row_ids_follow_rows_as_the_viewport_scrolls() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        for _ in 0..10 {
            terminal.vt_write(b"x\r\n");
        }
        let mut state = RenderState::new().unwrap();
        let mut rows = RowIterator::new().unwrap();

        let before = row_ids(&mut rows, &state.update(&terminal).unwrap());
        // Every row has its own id, and it doesn't change without a reason.
        assert!(before[0] != before[1] && before[1] != before[2] && before[0] != before[2]);
        assert_eq!(
            row_ids(&mut rows, &state.update(&terminal).unwrap()),
            before
        );

        // Scrolling up by one moves each row one position down, keeping its id.
        terminal.scroll_viewport(crate::terminal::ScrollViewport::Delta(-1));
        let after = row_ids(&mut rows, &state.update(&terminal).unwrap());
        assert_eq!(after[1..], before[..2]);
        assert!(!before.contains(&after[0]));

        // With overscan, the row scrolled out below is still captured under
        // the same id.
        state.set_overscan(Overscan { above: 0, below: 1 }).unwrap();
        let overscanned = row_ids(&mut rows, &state.update(&terminal).unwrap());
        assert_eq!(overscanned[..3], after);
        assert_eq!(overscanned[3], before[2]);
    }

    #[test]
    fn cells_raw_matches_cell_iteration_and_outlives_the_row() {
        let mut terminal = Terminal::new(4, 2).unwrap();
        terminal.vt_write(b"ab\r\ncd");
        let mut state = RenderState::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut rows = RowIterator::new().unwrap();
        let mut cells = CellIterator::new().unwrap();
        let mut iteration = rows.update(&snapshot).unwrap();

        let row = iteration.next().unwrap();
        let first_row = row.cells_raw().unwrap();
        assert_eq!(first_row.len(), 4);
        let mut expected = Vec::new();
        let mut cell_iteration = cells.update(row).unwrap();
        while let Some(cell) = cell_iteration.next() {
            expected.push(cell.raw_cell().unwrap());
        }

        // The raw cells stay usable after advancing to the next row.
        let second_row = iteration.next().unwrap().cells_raw().unwrap();
        let first_row: Vec<_> = first_row.collect();
        assert_eq!(first_row, expected);
        let codepoints =
            |cells: &[Cell]| -> Vec<u32> { cells.iter().map(|c| c.codepoint().unwrap()).collect() };
        assert_eq!(codepoints(&first_row), [0x61, 0x62, 0, 0]);
        assert_eq!(
            codepoints(&second_row.collect::<Vec<_>>()),
            [0x63, 0x64, 0, 0]
        );
    }

    #[test]
    fn graphemes_utf8_replaces_the_buffer() {
        let mut terminal = Terminal::new(4, 1).unwrap();
        // An "e" with a combining acute accent, then an empty cell.
        terminal.vt_write("e\u{301}".as_bytes());
        let mut state = RenderState::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut rows = RowIterator::new().unwrap();
        let mut cells = CellIterator::new().unwrap();
        let mut iteration = rows.update(&snapshot).unwrap();
        let mut cell_iteration = cells.update(iteration.next().unwrap()).unwrap();

        // Too small for the cluster, and holding leftovers from earlier.
        let mut text = String::from("x");
        cell_iteration.next().unwrap();
        cell_iteration.graphemes_utf8(&mut text).unwrap();
        assert_eq!(text, "e\u{301}");
        cell_iteration.next().unwrap();
        cell_iteration.graphemes_utf8(&mut text).unwrap();
        assert_eq!(text, "");
    }

    /// Upstream's `GRAPHEMES_BUF` takes no length and writes unchecked, so a
    /// short buffer must be rejected on the Rust side before calling C.
    #[test]
    fn graphemes_buf_rejects_short_buffer() {
        let mut terminal = Terminal::new(4, 1).unwrap();
        // An "e" with a combining acute accent: two codepoints in one cell.
        terminal.vt_write("e\u{301}".as_bytes());
        let mut state = RenderState::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut rows = RowIterator::new().unwrap();
        let mut cells = CellIterator::new().unwrap();
        let mut iteration = rows.update(&snapshot).unwrap();
        let mut cell_iteration = cells.update(iteration.next().unwrap()).unwrap();
        cell_iteration.next().unwrap();

        // One slot short: rejected with the needed size, buffer untouched.
        let mut short = ['x'];
        assert!(matches!(
            cell_iteration.graphemes_buf(&mut short),
            Err(Error::OutOfSpace { required: 2 })
        ));
        assert_eq!(short, ['x']);

        // Exactly the needed size: filled with base then combining mark.
        let mut exact = ['x'; 2];
        cell_iteration.graphemes_buf(&mut exact).unwrap();
        assert_eq!(exact, ['e', '\u{301}']);
    }

    /// Every value of one image placement, read through the public getters.
    #[derive(Debug, PartialEq, Eq)]
    struct Placement {
        protocol: ImageProtocol,
        image_id: u32,
        layer: ImageLayer,
        z: i32,
        viewport: (i32, i32),
        grid: (u32, u32),
        cell_offset: (u32, u32),
        pixel_size: (u32, u32),
        source: (u32, u32, u32, u32),
        kitty_placement_id: Option<u32>,
        kitty_kind: Option<KittyPlacementKind>,
    }

    /// Collect every placement of the snapshot along with its image
    /// generation, which is compared separately.
    fn placements<'alloc>(
        images: &mut ImageIterator<'alloc>,
        snapshot: &Snapshot<'alloc, '_>,
    ) -> Vec<(Placement, u64)> {
        let mut out = Vec::new();
        let mut iteration = images.update(snapshot).unwrap();
        while let Some(p) = iteration.next() {
            out.push((
                Placement {
                    protocol: p.protocol().unwrap(),
                    image_id: p.image_id().unwrap(),
                    layer: p.layer().unwrap(),
                    z: p.z().unwrap(),
                    viewport: (p.viewport_col().unwrap(), p.viewport_row().unwrap()),
                    grid: (p.grid_cols().unwrap(), p.grid_rows().unwrap()),
                    cell_offset: (p.cell_offset_x().unwrap(), p.cell_offset_y().unwrap()),
                    pixel_size: (p.pixel_width().unwrap(), p.pixel_height().unwrap()),
                    source: (
                        p.source_x().unwrap(),
                        p.source_y().unwrap(),
                        p.source_width().unwrap(),
                        p.source_height().unwrap(),
                    ),
                    kitty_placement_id: p.kitty_placement_id().unwrap(),
                    kitty_kind: p.kitty_kind().unwrap(),
                },
                p.image_generation().unwrap(),
            ));
        }
        out
    }

    #[test]
    fn image_iteration_is_empty_without_a_cell_size() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();

        // A 1x1 RGB image displayed at the cursor, but no cell size yet.
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1,q=2;////\x1b\\");
        // The image was stored, so an empty list is due to the cell size.
        // Only the check needs the `kitty` wrappers; libghostty stores the
        // image either way.
        #[cfg(feature = "kitty-graphics")]
        assert!(terminal.kitty_graphics().unwrap().image(1).is_some());
        let snapshot = state.update(&terminal).unwrap();
        let generation = snapshot.image_placements_generation().unwrap();
        {
            let mut iteration = images.update(&snapshot).unwrap();
            // Reading before moving to a placement is an error, not garbage.
            assert!(matches!(iteration.image_id(), Err(Error::InvalidValue)));
            assert!(iteration.next().is_none());
            assert!(matches!(iteration.image_id(), Err(Error::InvalidValue)));
        }

        // Once the terminal has a cell size, the same image has something
        // to draw.
        terminal.resize(10, 5, 10, 20).unwrap();
        let snapshot = state.update(&terminal).unwrap();
        assert_ne!(snapshot.image_placements_generation().unwrap(), generation);
        let found = placements(&mut images, &snapshot);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.image_id, 1);
    }

    #[test]
    fn image_iteration_reports_the_source_rectangle() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 20).unwrap();
        // Two placements of 2x2 RGB images, each showing part of its image:
        // the right column (x=1, w=1, full height) and the bottom row (y=1,
        // h=1, full width).
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=2,v=2,i=7,x=1,y=0,w=1,h=2,q=2;////////////////\x1b\\");
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=2,v=2,i=8,x=0,y=1,w=2,h=1,q=2;////////////////\x1b\\");

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut found = placements(&mut images, &snapshot)
            .into_iter()
            .map(|(p, _)| (p.image_id, p.source))
            .collect::<Vec<_>>();
        found.sort_unstable();
        assert_eq!(found, [(7, (1, 0, 1, 2)), (8, (0, 1, 2, 1))]);
    }

    #[test]
    fn image_iteration_reports_a_fixed_placement() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 20).unwrap();
        // Display a 2x2 RGB image at row 1, column 2, stretched over 4x2
        // cells, with a pixel offset within the first cell and a z-index.
        terminal.vt_write(
            b"\x1b[2;3H\
              \x1b_Ga=T,f=24,s=2,v=2,i=7,p=3,c=4,r=2,X=3,Y=4,z=5,q=2;////////////////\x1b\\",
        );

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let found = placements(&mut images, &snapshot);
        assert_eq!(
            found.iter().map(|(p, _)| p).collect::<Vec<_>>(),
            [&Placement {
                protocol: ImageProtocol::Kitty,
                image_id: 7,
                layer: ImageLayer::AboveText,
                z: 5,
                viewport: (2, 1),
                grid: (4, 2),
                cell_offset: (3, 4),
                // The 4x2 cells of 10x20 pixels, less the offset into them.
                pixel_size: (37, 36),
                source: (0, 0, 2, 2),
                kitty_placement_id: Some(3),
                kitty_kind: Some(KittyPlacementKind::Fixed),
            }],
        );

        // The image generation is the one in the terminal's storage, which
        // only the `kitty` wrappers can read.
        #[cfg(feature = "kitty-graphics")]
        {
            let image = terminal.kitty_graphics().unwrap().image(7).unwrap();
            assert_eq!(found[0].1, image.generation().unwrap());
        }

        // Place a second image 1 column right of and 2 rows below the first
        // placement (its parent), without a placement ID of its own.
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=8,P=7,Q=3,H=1,V=2,q=2;////\x1b\\");
        let snapshot = state.update(&terminal).unwrap();
        let relative = placements(&mut images, &snapshot)
            .into_iter()
            .map(|(p, _)| p)
            .find(|p| p.image_id == 8)
            .unwrap();
        assert_eq!(relative.viewport, (3, 3));
        assert_eq!(relative.kitty_placement_id, Some(0));
        assert_eq!(relative.kitty_kind, Some(KittyPlacementKind::Relative));
    }

    #[test]
    fn image_iteration_reports_a_virtual_placement() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        // A 1x2 RGB image with a 2x1 virtual placement (U=1), then two
        // placeholder cells (U+10EEEE) for it at row 1, column 3. The
        // foreground color carries the image ID and the underline color
        // the placement ID.
        terminal.vt_write(b"\x1b_Ga=T,t=d,f=24,i=1,p=1,s=1,v=2,U=1,c=2,r=1,q=2;////////\x1b\\");
        terminal.vt_write("\x1b[2;4H\x1b[38;5;1;58;5;1m\u{10EEEE}\u{10EEEE}".as_bytes());

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let found = placements(&mut images, &snapshot);
        assert_eq!(
            found.iter().map(|(p, _)| p).collect::<Vec<_>>(),
            [&Placement {
                protocol: ImageProtocol::Kitty,
                image_id: 1,
                // Virtual placements are always z = -1, below the text.
                layer: ImageLayer::BelowText,
                z: -1,
                viewport: (3, 1),
                grid: (2, 1),
                // The 1x2 image is scaled to fit 2x1 cells of 10x10 pixels:
                // 5 pixels wide, centered in the 20 pixel width.
                cell_offset: (8, 0),
                pixel_size: (5, 10),
                source: (0, 0, 1, 2),
                kitty_placement_id: Some(1),
                kitty_kind: Some(KittyPlacementKind::Virtual),
            }],
        );
    }

    #[test]
    fn image_placements_follow_scrolling_and_track_changes() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        // A 1x1 image stretched over 2x4 cells at the top-left corner.
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1,c=2,r=4,q=2;////\x1b\\");

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let generation = snapshot.image_placements_generation().unwrap();
        let found = placements(&mut images, &snapshot);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].0.viewport, found[0].0.grid), ((0, 0), (2, 4)));

        // Text alone leaves the list, and so its generation, unchanged. This
        // also moves the cursor to the bottom row, so each newline below
        // scrolls the screen up by one row.
        terminal.vt_write(b"\x1b[5;5Hhi");
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.image_placements_generation().unwrap(), generation);

        // Scrolling the top two rows away keeps the placement at its full
        // size, starting above the viewport.
        terminal.vt_write(b"\n\n");
        let snapshot = state.update(&terminal).unwrap();
        assert_ne!(snapshot.image_placements_generation().unwrap(), generation);
        let found = placements(&mut images, &snapshot);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].0.viewport, found[0].0.grid), ((0, -2), (2, 4)));
    }

    #[test]
    fn image_layer_follows_the_z_index() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        // Upstream picks the layer from the Kitty z-index: below
        // `i32::MIN / 2` (-1073741824) is below the backgrounds, from there
        // up to -1 is below the text, and 0 and above is above the text.
        // One 1x1 image per side of each threshold.
        for (id, z) in [(1, -1_073_741_825), (2, -1_073_741_824), (3, -1), (4, 0)] {
            terminal
                .vt_write(format!("\x1b_Ga=T,f=24,s=1,v=1,i={id},z={z},q=2;////\x1b\\").as_bytes());
        }

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut found = placements(&mut images, &snapshot)
            .into_iter()
            .map(|(p, _)| (p.image_id, p.z, p.layer))
            .collect::<Vec<_>>();
        found.sort_unstable_by_key(|&(id, ..)| id);
        assert_eq!(
            found,
            [
                (1, -1_073_741_825, ImageLayer::BelowBackground),
                (2, -1_073_741_824, ImageLayer::BelowText),
                (3, -1, ImageLayer::BelowText),
                (4, 0, ImageLayer::AboveText),
            ],
        );
    }

    #[test]
    fn image_iteration_stays_exhausted() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1,q=2;////\x1b\\");

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut iteration = images.update(&snapshot).unwrap();
        assert!(iteration.next().is_some());
        assert!(iteration.next().is_none());
        // Calling `next` again doesn't start over or read past the end.
        assert!(iteration.next().is_none());
        assert!(matches!(iteration.image_id(), Err(Error::InvalidValue)));
    }

    #[test]
    #[cfg(not(miri))]
    fn image_iterator_allocation_failure_is_reported() {
        // The iterator is allocated with the given allocator, so one that
        // refuses everything makes creating it fail.
        let capped = crate::alloc::testing::Capped::new(0);
        assert!(matches!(
            ImageIterator::new_with_alloc(&capped.allocator()),
            Err(Error::OutOfMemory)
        ));
    }

    #[test]
    #[cfg(all(not(miri), any(target_os = "linux", target_os = "macos")))]
    fn image_iterator_uses_its_allocator() {
        // With guard pages, any use of the iterator's memory after libghostty
        // freed it, or freeing it with another allocator, crashes.
        let alloc = crate::alloc::testing::Guard::allocator();
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1,q=2;////\x1b\\");

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new_with_alloc(&alloc).unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let found = placements(&mut images, &snapshot);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.image_id, 1);

        // Refill the same iterator after another update.
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=2,q=2;////\x1b\\");
        let snapshot = state.update(&terminal).unwrap();
        let mut ids = placements(&mut images, &snapshot)
            .into_iter()
            .map(|(p, _)| p.image_id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, [1, 2]);
        drop(images);
    }
}
