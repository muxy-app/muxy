use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::wire::cbor::open_enum;

pub const MAX_GRAPHICS_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_GRAPHICS_PLACEMENTS: usize = 1024;
/// The most frames one animation lists.
pub const MAX_GRAPHICS_FRAMES: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct CellSize {
    #[n(0)]
    pub width: u16,
    #[n(1)]
    pub height: u16,
}

impl Default for CellSize {
    fn default() -> Self {
        Self {
            width: 8,
            height: 16,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Graphics {
    #[n(0)]
    pub cell: CellSize,
    #[n(1)]
    pub images: Vec<GraphicImage>,
    #[n(2)]
    pub placements: Vec<GraphicPlacement>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GraphicImage {
    #[n(0)]
    pub id: u32,
    #[n(1)]
    pub generation: u64,
    #[n(2)]
    pub width: u32,
    #[n(3)]
    pub height: u32,
    /// The pixels of the frame on show. Empty for an app that asked with
    /// `RequestBody::DeferImages`, which reads them with `ImageFrames`, and
    /// for one that asked with `RequestBody::StreamImages` once it was sent
    /// them.
    #[n(4)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub rgba: Arc<[u8]>,
    /// The frames of an animation, first to last. Empty for a still image,
    /// whose only frame has the image's generation as its key.
    #[n(5)]
    #[cbor(default)]
    #[serde(default)]
    pub frames: Vec<GraphicFrame>,
    /// How the app plays `frames`.
    #[n(6)]
    #[cbor(default)]
    #[serde(default)]
    pub playback: Playback,
}

impl GraphicImage {
    /// The key of each frame, first to last.
    pub fn frame_keys(&self) -> impl Iterator<Item = u64> + '_ {
        let still = self.frames.is_empty().then_some(self.generation);
        still
            .into_iter()
            .chain(self.frames.iter().map(|frame| frame.key))
    }

    /// The key of the frame on show, whose pixels `rgba` holds.
    pub fn shown_key(&self) -> Option<u64> {
        self.frame_keys().nth(self.playback.frame as usize)
    }
}

/// One frame of an animation. Frames with the same key in the same image
/// have the same pixels, so an app reads each key once.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GraphicFrame {
    #[n(0)]
    pub key: u64,
    /// Milliseconds the frame shows before the next one. A playing animation
    /// skips a frame with no gap.
    #[n(1)]
    pub gap: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Playback {
    #[n(0)]
    pub state: PlaybackState,
    /// The frame to show while stopped, and to start playing from.
    #[n(1)]
    pub frame: u32,
    /// Times to play through the frames before resting on the last one.
    /// Zero plays forever.
    #[n(2)]
    pub loops: u32,
}

open_enum! {
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
    pub enum PlaybackState {
        /// Shows one frame until the program picks another.
        #[default]
        Stopped = 0,
        /// Plays to the last frame and waits there for more.
        Loading = 1,
        Running = 2,
    }
}

/// The pixels of one frame, as `RequestBody::ImageFrames` returns them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ImageFrame {
    #[n(0)]
    pub key: u64,
    #[n(1)]
    pub width: u32,
    #[n(2)]
    pub height: u32,
    #[n(3)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub rgba: Arc<[u8]>,
}

/// Checks the frames of one `ImageFrames` reply.
pub fn validate_image_frames(frames: &[ImageFrame]) -> Result<(), crate::ErrorCode> {
    use crate::ErrorCode::BadRequest;
    let mut total = 0_usize;
    for frame in frames {
        let bytes = u64::from(frame.width)
            .checked_mul(u64::from(frame.height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(BadRequest)?;
        total = total.checked_add(frame.rgba.len()).ok_or(BadRequest)?;
        if bytes == 0 || bytes != frame.rgba.len() as u64 || total > MAX_GRAPHICS_BYTES {
            return Err(BadRequest);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GraphicPlacement {
    #[n(0)]
    pub image: u32,
    #[n(1)]
    pub id: u32,
    #[n(2)]
    pub column: i32,
    #[n(3)]
    pub row: i32,
    #[n(4)]
    pub offset: [u32; 2],
    #[n(5)]
    pub size: [u32; 2],
    #[n(6)]
    pub source: [u32; 4],
    #[n(7)]
    pub z: i32,
}

impl Graphics {
    /// Leaves the pixels out, for an app that reads them with `ImageFrames`.
    pub fn defer_pixels(&mut self) {
        for image in &mut self.images {
            image.rgba = Arc::default();
        }
    }

    /// Takes the pixels `older` holds for the frames on show this lists
    /// without them, as a connection that streams images is sent each once.
    pub fn keep_pixels(&mut self, older: &Self) {
        for image in &mut self.images {
            if !image.rgba.is_empty() {
                continue;
            }
            let Some(key) = image.shown_key() else {
                continue;
            };
            if let Some(kept) = older.images.iter().find(|kept| {
                kept.id == image.id && !kept.rgba.is_empty() && kept.shown_key() == Some(key)
            }) {
                image.rgba = kept.rgba.clone();
            }
        }
    }

    pub fn validate(&self) -> Result<(), crate::ErrorCode> {
        use crate::ErrorCode::BadRequest;
        if !(1..=4096).contains(&self.cell.width)
            || !(1..=4096).contains(&self.cell.height)
            || self.images.len() > MAX_GRAPHICS_PLACEMENTS
            || self.placements.len() > MAX_GRAPHICS_PLACEMENTS
        {
            return Err(BadRequest);
        }
        let mut images = std::collections::BTreeMap::new();
        let mut total = 0_usize;
        for image in &self.images {
            let bytes = u64::from(image.width)
                .checked_mul(u64::from(image.height))
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or(BadRequest)?;
            total = total.checked_add(image.rgba.len()).ok_or(BadRequest)?;
            if image.width == 0
                || image.height == 0
                || bytes > MAX_GRAPHICS_BYTES as u64
                || (!image.rgba.is_empty() && bytes != image.rgba.len() as u64)
                || total > MAX_GRAPHICS_BYTES
                || image.frames.len() > MAX_GRAPHICS_FRAMES
                || image.playback.frame as usize >= image.frames.len().max(1)
                || images.insert(image.id, image).is_some()
            {
                return Err(BadRequest);
            }
        }
        for placement in &self.placements {
            let image = images.get(&placement.image).ok_or(BadRequest)?;
            let [x, y, width, height] = placement.source;
            if width == 0
                || height == 0
                || placement.size.contains(&0)
                || x.checked_add(width).is_none_or(|end| end > image.width)
                || y.checked_add(height).is_none_or(|end| end > image.height)
            {
                return Err(BadRequest);
            }
        }
        Ok(())
    }
}
