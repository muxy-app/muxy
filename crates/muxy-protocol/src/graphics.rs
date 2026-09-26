use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const MAX_GRAPHICS_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_GRAPHICS_PLACEMENTS: usize = 1024;

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
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
                || bytes != image.rgba.len() as u64
                || total > MAX_GRAPHICS_BYTES
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
