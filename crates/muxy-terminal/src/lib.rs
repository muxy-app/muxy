//! Native terminal emulation, retained history, and PTY process I/O.
//!
//! Screen data uses the shared protocol types; session lifecycle and
//! application policy stay with their owners.

mod error;
mod events;
mod ghostty;
mod input;
mod links;
mod program_status;
mod progress;
mod runs;
mod screen;

pub use error::{TerminalError, TerminalStep};
pub use events::TerminalEvent;
pub use ghostty::{Terminal, TerminalArchive, TerminalHistory, TerminalSnapshot};
pub use links::{LinkRow, LinkSpan, MAX_LINK_SPANS, MAX_LINK_URI};
pub use program_status::{ProgramState, ProgramStatus};
pub use screen::{
    Color, Cursor, CursorShape, InputModes, Modes, Modifiers, MouseAction, MouseButton, MouseEvent,
    Row, Run, ScrollDirection, Size, Style, Underline,
};

mod graphics;
pub use graphics::{
    CellSize, GraphicFrame, GraphicImage, GraphicPlacement, Graphics, ImageFrame,
    MAX_GRAPHICS_BYTES, MAX_GRAPHICS_PLACEMENTS, Playback, PlaybackState,
};

pub mod pty;
