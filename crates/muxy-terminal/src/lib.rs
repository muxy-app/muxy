//! Native terminal emulation, retained history, and PTY process I/O.
//!
//! Screen data uses the shared protocol types; session lifecycle and
//! application policy stay with their owners.

mod error;
mod events;
mod ghostty;
mod input;
mod links;
mod progress;
mod runs;
mod screen;

pub use error::{TerminalError, TerminalStep};
pub use events::TerminalEvent;
pub use ghostty::{Terminal, TerminalArchive};
pub use links::{LinkRow, LinkSpan, MAX_LINK_SPANS, MAX_LINK_URI};
pub use screen::{
    Color, Cursor, CursorShape, InputModes, Modes, Modifiers, MouseAction, MouseButton, MouseEvent,
    Row, Run, ScrollDirection, Size, Style, Underline,
};

mod graphics;
pub use graphics::{
    CellSize, GraphicImage, GraphicPlacement, Graphics, MAX_GRAPHICS_BYTES, MAX_GRAPHICS_PLACEMENTS,
};

pub mod pty;
