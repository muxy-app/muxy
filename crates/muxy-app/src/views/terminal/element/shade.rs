use std::{collections::BTreeMap, sync::Arc};

use gpui::{Bounds, Hsla, Pixels, RenderImage, Window, px};

#[derive(Clone, Copy)]
pub(super) struct Shade {
    pub(super) bounds: Bounds<Pixels>,
    pub(super) color: Hsla,
    pub(super) density: u8,
}

pub(super) fn prepare(
    text: &str,
    bounds: Bounds<Pixels>,
    color: Hsla,
    shades: &mut Vec<Shade>,
) -> bool {
    let density = match text {
        "░" => 1,
        "▒" => 2,
        "▓" => 3,
        _ => return false,
    };
    if let Some(previous) = shades.last_mut()
        && previous.density == density
        && previous.color == color
        && previous.bounds.right() == bounds.left()
        && previous.bounds.top() == bounds.top()
        && previous.bounds.size.height == bounds.size.height
    {
        previous.bounds.size.width += bounds.size.width;
    } else {
        shades.push(Shade {
            bounds,
            color,
            density,
        });
    }
    true
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct Key {
    width: u32,
    height: u32,
    phase: [u32; 2],
    color: u32,
    density: u8,
}

#[derive(Default)]
pub(crate) struct Textures {
    images: BTreeMap<Key, Arc<RenderImage>>,
    retired: Vec<Arc<RenderImage>>,
}

pub(super) struct Sprite {
    bounds: Bounds<Pixels>,
    image: Arc<RenderImage>,
}

impl Textures {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(super) fn prepare(
        &mut self,
        shades: &[Shade],
        clip: Bounds<Pixels>,
        scale: f32,
    ) -> Vec<Sprite> {
        let mut previous = std::mem::take(&mut self.images);
        let mut sprites = Vec::new();
        for shade in shades {
            let bounds = shade.bounds.intersect(&clip);
            if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                continue;
            }
            let snap = |value: Pixels| px((f32::from(value) * scale).round() / scale);
            let bounds =
                Bounds::from_corners(bounds.origin.map(snap), bounds.bottom_right().map(snap));
            let key = Key {
                width: (f32::from(bounds.size.width) * scale).round() as u32,
                height: (f32::from(bounds.size.height) * scale).round() as u32,
                phase: [
                    (f32::from(bounds.left()) * scale).round().rem_euclid(2.0) as u32,
                    (f32::from(bounds.top()) * scale).round().rem_euclid(2.0) as u32,
                ],
                color: u32::from(gpui::Rgba::from(shade.color)),
                density: shade.density,
            };
            if key.width == 0 || key.height == 0 {
                continue;
            }
            let image = self
                .images
                .entry(key)
                .or_insert_with(|| previous.remove(&key).unwrap_or_else(|| rasterize(key)));
            sprites.push(Sprite {
                bounds,
                image: image.clone(),
            });
        }
        self.retired.extend(previous.into_values());
        sprites
    }

    pub(crate) fn drain(&mut self) -> impl Iterator<Item = Arc<RenderImage>> {
        self.retired
            .drain(..)
            .chain(std::mem::take(&mut self.images).into_values())
    }

    pub(super) fn retired(&mut self) -> impl Iterator<Item = Arc<RenderImage>> {
        self.retired.drain(..)
    }
}

fn rasterize(key: Key) -> Arc<RenderImage> {
    let [r, g, b, a] = key.color.to_be_bytes();
    let pixels = image::RgbaImage::from_fn(key.width, key.height, |x, y| {
        let rank =
            [[0, 3], [2, 1]][((y + key.phase[1]) % 2) as usize][((x + key.phase[0]) % 2) as usize];
        image::Rgba(if rank < key.density {
            [b, g, r, a]
        } else {
            [0; 4]
        })
    });
    Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]))
}

pub(super) fn paint(sprites: &[Sprite], window: &mut Window) {
    for sprite in sprites {
        let _ = window.paint_image(
            sprite.bounds,
            gpui::Corners::all(px(0.0)),
            sprite.image.clone(),
            0,
            false,
        );
    }
}
