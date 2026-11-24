use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::wire::cbor::open_enum;

pub const MAX_GRAPHICS_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SHARED_GRAPHICS_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_GRAPHICS_PLACEMENTS: usize = 1024;
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
    #[n(4)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub rgba: Arc<[u8]>,
    #[n(5)]
    #[cbor(default)]
    #[serde(default)]
    pub frames: Vec<GraphicFrame>,
    #[n(6)]
    #[cbor(default)]
    #[serde(default)]
    pub playback: Playback,
    #[n(7)]
    #[serde(default)]
    pub shared: Option<SharedPixels>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SharedPixels {
    #[n(0)]
    pub name: String,
}

impl GraphicImage {
    pub fn frame_keys(&self) -> impl Iterator<Item = u64> + '_ {
        let still = self.frames.is_empty().then_some(self.generation);
        still
            .into_iter()
            .chain(self.frames.iter().map(|frame| frame.key))
    }

    pub fn shown_key(&self) -> Option<u64> {
        self.frame_keys().nth(self.playback.frame as usize)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GraphicFrame {
    #[n(0)]
    pub key: u64,
    #[n(1)]
    pub gap: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Playback {
    #[n(0)]
    pub state: PlaybackState,
    #[n(1)]
    pub frame: u32,
    #[n(2)]
    pub loops: u32,
}

open_enum! {
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
    pub enum PlaybackState {
        #[default]
        Stopped = 0,
        Loading = 1,
        Running = 2,
    }
}

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

impl GraphicPlacement {
    pub fn rows(&self, cell: CellSize) -> i64 {
        let height = i64::from(cell.height.max(1));
        let tall = i64::from(self.offset[1]) + i64::from(self.size[1]);
        (tall + height - 1) / height
    }

    pub fn covers(&self, cell: CellSize, rows: i64) -> bool {
        let row = i64::from(self.row);
        row < rows && row + self.rows(cell) > 0
    }
}

impl Graphics {
    pub fn shift_up(&mut self, rows: i64, within: i64) {
        let cell = self.cell;
        self.placements.retain_mut(|placement| {
            let Ok(row) = i32::try_from(i64::from(placement.row) - rows) else {
                return false;
            };
            placement.row = row;
            placement.covers(cell, within)
        });
        let placed: std::collections::BTreeSet<u32> = self
            .placements
            .iter()
            .map(|placement| placement.image)
            .collect();
        self.images.retain(|image| placed.contains(&image.id));
    }

    pub fn defer_pixels(&mut self) {
        for image in &mut self.images {
            image.rgba = Arc::default();
        }
    }

    pub fn keep_pixels(&mut self, older: &Self) {
        for image in &mut self.images {
            if !image.rgba.is_empty() || image.shared.is_some() {
                continue;
            }
            let Some(key) = image.shown_key() else {
                continue;
            };
            if let Some(kept) = older.images.iter().find(|kept| {
                kept.id == image.id
                    && (kept.width, kept.height) == (image.width, image.height)
                    && (!kept.rgba.is_empty() || kept.shared.is_some())
                    && kept.shown_key() == Some(key)
            }) {
                image.rgba = kept.rgba.clone();
                image.shared.clone_from(&kept.shared);
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
        let mut inline = 0_usize;
        let mut listed = 0_u64;
        for image in &self.images {
            let bytes = u64::from(image.width)
                .checked_mul(u64::from(image.height))
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or(BadRequest)?;
            inline = inline.checked_add(image.rgba.len()).ok_or(BadRequest)?;
            listed = listed.checked_add(bytes).ok_or(BadRequest)?;
            if image.width == 0
                || image.height == 0
                || listed > MAX_SHARED_GRAPHICS_BYTES as u64
                || (!image.rgba.is_empty() && bytes != image.rgba.len() as u64)
                || inline > MAX_GRAPHICS_BYTES
                || image.shared.as_ref().is_some_and(|shared| {
                    !image.rgba.is_empty() || !crate::shared_memory::is_region_name(&shared.name)
                })
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
