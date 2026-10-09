use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use gpui::{Bounds, ContentMask, Pixels, Point, RenderImage, Window, point, px, size};
use muxy_protocol::Graphics;

#[derive(Default)]
pub(crate) struct Textures {
    images: BTreeMap<(u32, u64), (muxy_protocol::GraphicImage, Arc<RenderImage>)>,
    retired: Vec<Arc<RenderImage>>,
}

pub(super) struct Placement {
    image: Arc<RenderImage>,
    bounds: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    pub(super) z: i32,
}

impl Textures {
    pub(crate) fn drain(&mut self) -> impl Iterator<Item = Arc<RenderImage>> {
        self.retired.drain(..).chain(
            std::mem::take(&mut self.images)
                .into_values()
                .map(|(_, image)| image),
        )
    }

    pub(super) fn retired(&mut self) -> impl Iterator<Item = Arc<RenderImage>> {
        self.retired.drain(..)
    }

    #[allow(clippy::cast_precision_loss)]
    pub(super) fn prepare(
        &mut self,
        graphics: &Graphics,
        origin: Point<Pixels>,
        cell: gpui::Size<Pixels>,
    ) -> Vec<Placement> {
        let used: BTreeSet<_> = graphics
            .images
            .iter()
            .map(|image| (image.id, image.generation))
            .collect();
        self.images.retain(|key, (_, image)| {
            if used.contains(key) {
                return true;
            }
            self.retired.push(image.clone());
            false
        });
        for image in &graphics.images {
            let key = (image.id, image.generation);
            if self
                .images
                .get(&key)
                .is_some_and(|(source, _)| source == image)
            {
                continue;
            }
            let mut bgra = image.rgba.to_vec();
            for pixel in bgra.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            let buffer =
                image::RgbaImage::from_raw(image.width, image.height, bgra).unwrap_or_default();
            if let Some((_, previous)) = self.images.insert(
                key,
                (
                    image.clone(),
                    Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])),
                ),
            ) {
                self.retired.push(previous);
            }
        }
        let mut result = Vec::new();
        for p in &graphics.placements {
            let Some(image) = graphics.images.iter().find(|image| image.id == p.image) else {
                continue;
            };
            let sx = cell.width / f32::from(graphics.cell.width);
            let sy = cell.height / f32::from(graphics.cell.height);
            let offset = point(
                cell.width * p.column as f32 + sx * p.offset[0] as f32,
                cell.height * p.row as f32 + sy * p.offset[1] as f32,
            );
            let clip = Bounds::new(
                origin + offset,
                size(sx * p.size[0] as f32, sy * p.size[1] as f32),
            );
            let xscale = clip.size.width / p.source[2].max(1) as f32;
            let yscale = clip.size.height / p.source[3].max(1) as f32;
            let bounds = Bounds::new(
                clip.origin - point(xscale * p.source[0] as f32, yscale * p.source[1] as f32),
                size(xscale * image.width as f32, yscale * image.height as f32),
            );
            if let Some((_, texture)) = self.images.get(&(image.id, image.generation)) {
                result.push(Placement {
                    image: texture.clone(),
                    bounds,
                    clip,
                    z: p.z,
                });
            }
        }
        result.sort_by_key(|p| p.z);
        result
    }
}

pub(super) fn paint(placements: &[Placement], z: std::ops::Range<i64>, window: &mut Window) {
    for placement in placements.iter().filter(|p| z.contains(&i64::from(p.z))) {
        window.with_content_mask(
            Some(ContentMask {
                bounds: placement.clip,
            }),
            |window| {
                let _ = window.paint_image(
                    placement.bounds,
                    gpui::Corners::all(px(0.0)),
                    placement.image.clone(),
                    0,
                    false,
                );
            },
        );
    }
}
