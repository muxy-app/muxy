//! How a saved screen is laid out on disk.
//!
//! These types are frozen: a change to them needs a new file magic. Protocol
//! changes only touch the conversions below. Rows keep the protocol's postcard
//! layout, which is frozen on the wire as well.

use std::sync::Arc;

use muxy_protocol::{
    CellSize, Cursor, CursorShape, ExitReason, GraphicImage, GraphicPlacement, Graphics, Row,
    SavedScreen, Size,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct StoredScreen {
    graphics: StoredGraphics,
    size: StoredSize,
    rows: Vec<Row>,
    cursor: StoredCursor,
    reason: Option<StoredExitReason>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredGraphics {
    cell: StoredCellSize,
    images: Vec<StoredImage>,
    placements: Vec<StoredPlacement>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredCellSize {
    width: u16,
    height: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredImage {
    id: u32,
    generation: u64,
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredPlacement {
    image: u32,
    id: u32,
    column: i32,
    row: i32,
    offset: [u32; 2],
    size: [u32; 2],
    source: [u32; 4],
    z: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct StoredSize {
    cols: u16,
    rows: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredCursor {
    shape: StoredCursorShape,
    row: u16,
    col: u16,
    visible: bool,
}

/// Unknown shapes keep their protocol number.
#[derive(Debug, Serialize, Deserialize)]
enum StoredCursorShape {
    Block,
    Bar,
    Underline,
    Hollow,
    Unrecognized(u32),
}

/// Unknown reasons keep their protocol number.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(super) enum StoredExitReason {
    Exited(i32),
    Signaled(i32),
    Ended,
    ServerStopped,
    Unrecognized(u32),
}

impl From<&SavedScreen> for StoredScreen {
    fn from(screen: &SavedScreen) -> Self {
        let SavedScreen {
            graphics,
            size,
            rows,
            cursor,
            reason,
        } = screen;
        Self {
            graphics: graphics.into(),
            size: (*size).into(),
            rows: rows.clone(),
            cursor: (*cursor).into(),
            reason: reason.map(Into::into),
        }
    }
}

impl From<StoredScreen> for SavedScreen {
    fn from(screen: StoredScreen) -> Self {
        Self {
            graphics: screen.graphics.into(),
            size: screen.size.into(),
            rows: screen.rows,
            cursor: screen.cursor.into(),
            reason: screen.reason.map(Into::into),
        }
    }
}

impl From<&Graphics> for StoredGraphics {
    fn from(graphics: &Graphics) -> Self {
        let Graphics {
            cell: CellSize { width, height },
            images,
            placements,
        } = graphics;
        Self {
            cell: StoredCellSize {
                width: *width,
                height: *height,
            },
            images: images.iter().map(Into::into).collect(),
            placements: placements.iter().map(Into::into).collect(),
        }
    }
}

impl From<StoredGraphics> for Graphics {
    fn from(graphics: StoredGraphics) -> Self {
        Self {
            cell: CellSize {
                width: graphics.cell.width,
                height: graphics.cell.height,
            },
            images: graphics.images.into_iter().map(Into::into).collect(),
            placements: graphics.placements.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<&GraphicImage> for StoredImage {
    fn from(image: &GraphicImage) -> Self {
        let GraphicImage {
            id,
            generation,
            width,
            height,
            rgba,
        } = image;
        Self {
            id: *id,
            generation: *generation,
            width: *width,
            height: *height,
            rgba: Arc::clone(rgba),
        }
    }
}

impl From<StoredImage> for GraphicImage {
    fn from(image: StoredImage) -> Self {
        Self {
            id: image.id,
            generation: image.generation,
            width: image.width,
            height: image.height,
            rgba: image.rgba,
        }
    }
}

impl From<&GraphicPlacement> for StoredPlacement {
    fn from(placement: &GraphicPlacement) -> Self {
        let GraphicPlacement {
            image,
            id,
            column,
            row,
            offset,
            size,
            source,
            z,
        } = *placement;
        Self {
            image,
            id,
            column,
            row,
            offset,
            size,
            source,
            z,
        }
    }
}

impl From<StoredPlacement> for GraphicPlacement {
    fn from(placement: StoredPlacement) -> Self {
        Self {
            image: placement.image,
            id: placement.id,
            column: placement.column,
            row: placement.row,
            offset: placement.offset,
            size: placement.size,
            source: placement.source,
            z: placement.z,
        }
    }
}

impl From<Size> for StoredSize {
    fn from(Size { cols, rows }: Size) -> Self {
        Self { cols, rows }
    }
}

impl From<StoredSize> for Size {
    fn from(size: StoredSize) -> Self {
        Self {
            cols: size.cols,
            rows: size.rows,
        }
    }
}

impl From<Cursor> for StoredCursor {
    fn from(
        Cursor {
            shape,
            row,
            col,
            visible,
        }: Cursor,
    ) -> Self {
        Self {
            shape: match shape {
                CursorShape::Block => StoredCursorShape::Block,
                CursorShape::Bar => StoredCursorShape::Bar,
                CursorShape::Underline => StoredCursorShape::Underline,
                CursorShape::Hollow => StoredCursorShape::Hollow,
                CursorShape::Unrecognized(number) => StoredCursorShape::Unrecognized(number),
            },
            row,
            col,
            visible,
        }
    }
}

impl From<StoredCursor> for Cursor {
    fn from(cursor: StoredCursor) -> Self {
        Self {
            shape: match cursor.shape {
                StoredCursorShape::Block => CursorShape::Block,
                StoredCursorShape::Bar => CursorShape::Bar,
                StoredCursorShape::Underline => CursorShape::Underline,
                StoredCursorShape::Hollow => CursorShape::Hollow,
                StoredCursorShape::Unrecognized(number) => CursorShape::Unrecognized(number),
            },
            row: cursor.row,
            col: cursor.col,
            visible: cursor.visible,
        }
    }
}

impl From<ExitReason> for StoredExitReason {
    fn from(reason: ExitReason) -> Self {
        match reason {
            ExitReason::Exited(code) => Self::Exited(code),
            ExitReason::Signaled(signal) => Self::Signaled(signal),
            ExitReason::Ended => Self::Ended,
            ExitReason::ServerStopped => Self::ServerStopped,
            ExitReason::Unrecognized(number) => Self::Unrecognized(number),
        }
    }
}

impl From<StoredExitReason> for ExitReason {
    fn from(reason: StoredExitReason) -> Self {
        match reason {
            StoredExitReason::Exited(code) => Self::Exited(code),
            StoredExitReason::Signaled(signal) => Self::Signaled(signal),
            StoredExitReason::Ended => Self::Ended,
            StoredExitReason::ServerStopped => Self::ServerStopped,
            StoredExitReason::Unrecognized(number) => Self::Unrecognized(number),
        }
    }
}
