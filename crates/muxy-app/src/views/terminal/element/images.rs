use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{Bounds, ContentMask, Pixels, Point, RenderImage, Window, point, px, size};
use muxy_protocol::{GraphicImage, Graphics, ImageFrame, Playback, PlaybackState};

type FrameKey = (u32, u64);

const MAX_REQUESTS: usize = 4;

#[derive(Default)]
pub(crate) struct Textures {
    frames: BTreeMap<FrameKey, Arc<RenderImage>>,
    asked: BTreeSet<FrameKey>,
    failed: BTreeSet<FrameKey>,
    unshared: BTreeSet<FrameKey>,
    answering: BTreeSet<FrameKey>,
    converting: BTreeSet<FrameKey>,
    requests: usize,
    drawn: BTreeMap<u32, u64>,
    players: BTreeMap<u32, Player>,
    retired: Vec<Arc<RenderImage>>,
    last: Vec<Placement>,
    epoch: u64,
    listed: BTreeSet<FrameKey>,
    painted: BTreeSet<FrameKey>,
}

#[derive(Clone)]
pub(super) struct Placement {
    image: Arc<RenderImage>,
    bounds: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    pub(super) z: i32,
}

pub(super) struct Prepared {
    pub(super) placements: Vec<Placement>,
    pub(super) wanted: Vec<(u32, Vec<u64>)>,
    pub(super) conversions: Vec<Conversion>,
    pub(super) next: Option<Instant>,
}

pub(crate) struct Conversion {
    key: FrameKey,
    width: u32,
    height: u32,
    source: Source,
    epoch: u64,
}

enum Source {
    Inline(Arc<[u8]>),
    Shared(String),
}

pub(crate) struct Converted {
    key: FrameKey,
    texture: Option<Arc<RenderImage>>,
    epoch: u64,
}

impl Conversion {
    pub(crate) fn run(self) -> Converted {
        let texture = match &self.source {
            Source::Inline(rgba) => Some(texture(self.width, self.height, rgba)),
            Source::Shared(name) => (self.width as usize)
                .checked_mul(self.height as usize)
                .and_then(|pixels| pixels.checked_mul(4))
                .and_then(|len| {
                    muxy_protocol::shared_memory::take(name, len, |rgba| {
                        texture(self.width, self.height, rgba)
                    })
                    .ok()
                }),
        };
        Converted {
            key: self.key,
            texture,
            epoch: self.epoch,
        }
    }
}

struct Player {
    playback: Playback,
    index: usize,
    since: Instant,
    loops: u32,
}

impl Textures {
    pub(crate) fn drain(&mut self) -> impl Iterator<Item = Arc<RenderImage>> {
        self.asked.clear();
        self.failed.clear();
        self.unshared.clear();
        self.answering.clear();
        self.converting.clear();
        self.requests = 0;
        self.drawn.clear();
        self.players.clear();
        self.last.clear();
        self.listed.clear();
        self.painted.clear();
        self.epoch = self.epoch.wrapping_add(1);
        self.retired
            .drain(..)
            .chain(std::mem::take(&mut self.frames).into_values())
    }

    pub(crate) fn reset(&mut self) {
        let frames: Vec<_> = self.drain().collect();
        self.retired = frames;
    }

    pub(super) fn retired(&mut self) -> impl Iterator<Item = Arc<RenderImage>> {
        self.retired.drain(..)
    }

    #[cfg(test)]
    pub(crate) fn has_frame(&self, image: u32, key: u64) -> bool {
        self.frames.contains_key(&(image, key))
    }

    pub(crate) fn idle(&self) -> bool {
        self.converting.is_empty()
    }

    pub(crate) fn retry(&mut self) {
        for key in std::mem::take(&mut self.failed) {
            self.asked.remove(&key);
        }
    }

    pub(crate) fn convert(&mut self, shown: &[&Graphics]) -> Vec<Conversion> {
        let images = || shown.iter().flat_map(|graphics| &graphics.images);
        self.listed = images()
            .flat_map(|image| image.frame_keys().map(|key| (image.id, key)))
            .collect();
        self.drawn
            .retain(|id, _| images().any(|image| image.id == *id));
        self.forget();
        let mut conversions = Vec::new();
        let listed = &self.listed;
        self.unshared.retain(|key| listed.contains(key));
        for image in images() {
            let Some(key) = image.shown_key() else {
                continue;
            };
            let frame = (image.id, key);
            let source = match &image.shared {
                _ if !image.rgba.is_empty() => Source::Inline(image.rgba.clone()),
                Some(shared) if !self.unshared.contains(&frame) => {
                    Source::Shared(shared.name.clone())
                }
                _ => continue,
            };
            if self.frames.contains_key(&frame) || !self.converting.insert(frame) {
                continue;
            }
            conversions.push(Conversion {
                key: frame,
                width: image.width,
                height: image.height,
                source,
                epoch: self.epoch,
            });
        }
        conversions
    }

    pub(crate) fn converted(&mut self, converted: Converted) -> bool {
        if converted.epoch == self.epoch
            && self.converting.remove(&converted.key)
            && self.listed.contains(&converted.key)
        {
            match converted.texture {
                Some(texture) => {
                    self.frames.entry(converted.key).or_insert(texture);
                    if !self.players.contains_key(&converted.key.0) {
                        self.drawn.insert(converted.key.0, converted.key.1);
                    }
                }
                None => {
                    self.unshared.insert(converted.key);
                }
            }
        }
        self.converting.is_empty()
    }

    fn forget(&mut self) {
        let (listed, drawn, last) = (&self.listed, &self.drawn, &self.last);
        let (painted, retired) = (&mut self.painted, &mut self.retired);
        self.frames.retain(|key, frame| {
            if listed.contains(key)
                || drawn.get(&key.0) == Some(&key.1)
                || last
                    .iter()
                    .any(|placement| Arc::ptr_eq(&placement.image, frame))
            {
                return true;
            }
            if painted.remove(key) {
                retired.push(frame.clone());
            }
            false
        });
    }

    pub(crate) fn receive(
        &mut self,
        image: u32,
        keys: &[u64],
        frames: Option<Vec<ImageFrame>>,
    ) -> bool {
        self.requests = self.requests.saturating_sub(1);
        for key in keys {
            self.answering.remove(&(image, *key));
        }
        let Some(frames) = frames else {
            self.failed.extend(keys.iter().map(|key| (image, *key)));
            return true;
        };
        if frames.iter().any(|frame| keys.contains(&frame.key)) {
            for key in keys {
                self.asked.remove(&(image, *key));
            }
        }
        for frame in &frames {
            self.frames
                .entry((image, frame.key))
                .or_insert_with(|| texture(frame.width, frame.height, &frame.rgba));
            if keys.contains(&frame.key) {
                self.drawn.entry(image).or_insert(frame.key);
            } else {
                self.drawn.insert(image, frame.key);
            }
        }
        false
    }

    fn arriving(&self, image: &GraphicImage) -> bool {
        image.frame_keys().any(|key| {
            let frame = (image.id, key);
            self.converting.contains(&frame) || self.answering.contains(&frame)
        })
    }

    fn paint(
        &mut self,
        graphics: &Graphics,
        shown: &BTreeMap<u32, (&GraphicImage, Arc<RenderImage>)>,
        origin: Point<Pixels>,
        cell: gpui::Size<Pixels>,
    ) -> Vec<Placement> {
        let waiting = graphics.placements.iter().any(|placement| {
            !shown.contains_key(&placement.image)
                && graphics
                    .images
                    .iter()
                    .find(|image| image.id == placement.image)
                    .is_some_and(|image| self.arriving(image))
        });
        if !waiting {
            self.last = place(graphics, shown, cell);
        }
        for (key, frame) in &self.frames {
            if self
                .last
                .iter()
                .any(|placement| Arc::ptr_eq(&placement.image, frame))
            {
                self.painted.insert(*key);
            }
        }
        self.forget();
        self.last
            .iter()
            .map(|placement| Placement {
                bounds: placement.bounds + origin,
                clip: placement.clip + origin,
                ..placement.clone()
            })
            .collect()
    }

    pub(super) fn prepare(
        &mut self,
        graphics: &Graphics,
        also: Option<&Graphics>,
        origin: Point<Pixels>,
        cell: gpui::Size<Pixels>,
        now: Instant,
        fetch: bool,
    ) -> Prepared {
        let listed: Vec<&Graphics> = std::iter::once(graphics).chain(also).collect();
        let conversions = self.convert(&listed);
        let listed = &self.listed;
        self.asked.retain(|key| listed.contains(key));
        self.failed.retain(|key| listed.contains(key));
        self.players.retain(|id, _| {
            graphics
                .images
                .iter()
                .any(|image| image.id == *id && !image.frames.is_empty())
        });
        let mut wanted = Vec::new();
        let mut next: Option<Instant> = None;
        let mut shown = BTreeMap::new();
        for image in &graphics.images {
            let keys: Vec<u64> = image.frame_keys().collect();
            let mut missing: Vec<u64> = Vec::new();
            for &key in &keys {
                let frame = (image.id, key);
                if !self.frames.contains_key(&frame)
                    && !self.asked.contains(&frame)
                    && !self.converting.contains(&frame)
                    && !missing.contains(&key)
                {
                    missing.push(key);
                }
            }
            if fetch && !missing.is_empty() && self.requests < MAX_REQUESTS {
                self.requests += 1;
                for &key in &missing {
                    self.asked.insert((image.id, key));
                    self.answering.insert((image.id, key));
                }
                wanted.push((image.id, missing));
            }
            let index = if image.frames.is_empty() {
                0
            } else {
                let frames = &self.frames;
                let (index, due) = self
                    .players
                    .entry(image.id)
                    .or_insert_with(|| Player::new(image.playback, now))
                    .advance(image, now, |key| frames.contains_key(&(image.id, key)));
                next = [next, due].into_iter().flatten().min();
                index
            };
            let current = keys
                .get(index)
                .copied()
                .filter(|key| self.frames.contains_key(&(image.id, *key)));
            if let Some(key) = current {
                self.drawn.insert(image.id, key);
            }
            if let Some(texture) = self
                .drawn
                .get(&image.id)
                .and_then(|key| self.frames.get(&(image.id, *key)))
            {
                shown.insert(image.id, (image, texture.clone()));
            }
        }
        let placements = self.paint(graphics, &shown, origin, cell);
        Prepared {
            placements,
            wanted,
            conversions,
            next,
        }
    }
}

impl Player {
    fn new(playback: Playback, now: Instant) -> Self {
        Self {
            playback,
            index: playback.frame as usize,
            since: now,
            loops: 0,
        }
    }

    fn advance(
        &mut self,
        image: &GraphicImage,
        now: Instant,
        arrived: impl Fn(u64) -> bool,
    ) -> (usize, Option<Instant>) {
        let frames = &image.frames;
        let playback = image.playback;
        if self.playback.frame != playback.frame {
            *self = Self::new(playback, now);
        } else if self.playback != playback {
            if self.playback.state == PlaybackState::Stopped {
                self.since = now;
            }
            self.playback = playback;
            self.loops = 0;
        }
        self.index = self.index.min(frames.len().saturating_sub(1));
        let moving = matches!(
            playback.state,
            PlaybackState::Running | PlaybackState::Loading
        );
        if !moving
            || frames.iter().all(|frame| frame.gap == 0)
            || (playback.loops > 0 && self.loops >= playback.loops)
        {
            return (self.index, None);
        }
        let gap = |index: usize| Duration::from_millis(frames[index].gap.into());
        let due = self.since + gap(self.index);
        if now < due {
            return (self.index, Some(due));
        }
        let mut index = self.index;
        let mut loops = self.loops;
        loop {
            index = (index + 1) % frames.len();
            if index == 0 {
                if playback.state == PlaybackState::Loading {
                    return (self.index, None);
                }
                loops += 1;
                if playback.loops > 0 && loops >= playback.loops {
                    self.loops = loops;
                    return (self.index, None);
                }
            }
            if frames[index].gap > 0 {
                break;
            }
        }
        if !arrived(frames[index].key) {
            return (self.index, None);
        }
        self.index = index;
        self.loops = loops;
        self.since = now;
        (index, Some(now + gap(index)))
    }
}

#[allow(clippy::cast_precision_loss)]
fn place(
    graphics: &Graphics,
    shown: &BTreeMap<u32, (&GraphicImage, Arc<RenderImage>)>,
    cell: gpui::Size<Pixels>,
) -> Vec<Placement> {
    let mut placements = Vec::new();
    for p in &graphics.placements {
        let Some((image, texture)) = shown.get(&p.image) else {
            continue;
        };
        let sx = cell.width / f32::from(graphics.cell.width);
        let sy = cell.height / f32::from(graphics.cell.height);
        let offset = point(
            cell.width * p.column as f32 + sx * p.offset[0] as f32,
            cell.height * p.row as f32 + sy * p.offset[1] as f32,
        );
        let clip = Bounds::new(offset, size(sx * p.size[0] as f32, sy * p.size[1] as f32));
        let xscale = clip.size.width / p.source[2].max(1) as f32;
        let yscale = clip.size.height / p.source[3].max(1) as f32;
        let bounds = Bounds::new(
            clip.origin - point(xscale * p.source[0] as f32, yscale * p.source[1] as f32),
            size(xscale * image.width as f32, yscale * image.height as f32),
        );
        placements.push(Placement {
            image: texture.clone(),
            bounds,
            clip,
            z: p.z,
        });
    }
    placements.sort_by_key(|p| p.z);
    placements
}

fn texture(width: u32, height: u32, rgba: &[u8]) -> Arc<RenderImage> {
    let mut bgra = rgba.to_vec();
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let buffer = image::RgbaImage::from_raw(width, height, bgra).unwrap_or_default();
    Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]))
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

#[cfg(test)]
mod tests {
    use muxy_protocol::{GraphicFrame, GraphicPlacement};

    use super::*;

    const CELL: gpui::Size<Pixels> = gpui::Size {
        width: px(8.0),
        height: px(16.0),
    };

    fn animated(gaps: &[u32], state: PlaybackState, loops: u32) -> Graphics {
        let frames = gaps
            .iter()
            .zip(10..)
            .map(|(&gap, key)| GraphicFrame { key, gap })
            .collect();
        Graphics {
            images: vec![GraphicImage {
                id: 1,
                generation: 1,
                width: 1,
                height: 1,
                frames,
                playback: Playback {
                    state,
                    frame: 0,
                    loops,
                },
                ..GraphicImage::default()
            }],
            placements: vec![GraphicPlacement {
                image: 1,
                id: 0,
                column: 0,
                row: 0,
                offset: [0, 0],
                size: [8, 16],
                source: [0, 0, 1, 1],
                z: 0,
            }],
            ..Graphics::default()
        }
    }

    fn frame(key: u64) -> ImageFrame {
        ImageFrame {
            key,
            width: 1,
            height: 1,
            rgba: vec![0; 4].into(),
        }
    }

    fn prepare(textures: &mut Textures, graphics: &Graphics, now: Instant) -> Prepared {
        textures.prepare(graphics, None, Point::default(), CELL, now, true)
    }

    fn showing(textures: &Textures, prepared: &Prepared) -> Option<u64> {
        let placed = &prepared.placements.first()?.image;
        textures
            .frames
            .iter()
            .find(|(_, texture)| Arc::ptr_eq(texture, placed))
            .map(|((_, key), _)| *key)
    }

    #[test]
    fn asks_for_each_frame_once_and_shows_it_when_it_arrives() {
        let graphics = animated(&[40, 40], PlaybackState::Running, 0);
        let mut textures = Textures::default();
        let start = Instant::now();
        let prepared = prepare(&mut textures, &graphics, start);
        assert_eq!(prepared.wanted, [(1, vec![10, 11])]);
        assert!(prepared.placements.is_empty());
        assert!(prepare(&mut textures, &graphics, start).wanted.is_empty());
        textures.receive(1, &[10, 11], Some(vec![frame(10)]));
        let prepared = prepare(&mut textures, &graphics, start);
        assert_eq!(prepared.wanted, [(1, vec![11])]);
        assert_eq!(showing(&textures, &prepared), Some(10));
        textures.receive(1, &[11], Some(Vec::new()));
        assert!(prepare(&mut textures, &graphics, start).wanted.is_empty());
    }

    #[test]
    fn plays_frames_by_their_gaps_and_skips_those_without_one() {
        let graphics = animated(&[40, 0, 100], PlaybackState::Running, 0);
        let mut textures = Textures::default();
        let start = Instant::now();
        prepare(&mut textures, &graphics, start);
        textures.receive(
            1,
            &[10, 11, 12],
            Some(vec![frame(10), frame(11), frame(12)]),
        );
        let at = |textures: &mut Textures, millis: u64| {
            let prepared = prepare(textures, &graphics, start + Duration::from_millis(millis));
            (showing(textures, &prepared), prepared.next)
        };
        let due = |millis: u64| Some(start + Duration::from_millis(millis));
        assert_eq!(at(&mut textures, 0), (Some(10), due(40)));
        assert_eq!(at(&mut textures, 39), (Some(10), due(40)));
        assert_eq!(at(&mut textures, 40), (Some(12), due(140)));
        assert_eq!(at(&mut textures, 140), (Some(10), due(180)));
        assert_eq!(at(&mut textures, 900), (Some(12), due(1000)));
    }

    #[test]
    fn stops_after_its_loops_and_waits_while_loading() {
        let mut textures = Textures::default();
        let start = Instant::now();
        let later = |millis: u64| start + Duration::from_millis(millis);
        let once = animated(&[40, 40], PlaybackState::Running, 1);
        prepare(&mut textures, &once, start);
        textures.receive(1, &[10, 11], Some(vec![frame(10), frame(11)]));
        assert!(prepare(&mut textures, &once, later(40)).next.is_some());
        let rested = prepare(&mut textures, &once, later(80));
        assert_eq!((showing(&textures, &rested), rested.next), (Some(11), None));

        let mut textures = Textures::default();
        let loading = animated(&[40, 40], PlaybackState::Loading, 0);
        prepare(&mut textures, &loading, start);
        textures.receive(1, &[10, 11], Some(vec![frame(10), frame(11)]));
        prepare(&mut textures, &loading, later(40));
        let waiting = prepare(&mut textures, &loading, later(80));
        assert_eq!(
            (showing(&textures, &waiting), waiting.next),
            (Some(11), None)
        );
        let mut longer = animated(&[40, 40, 40], PlaybackState::Loading, 0);
        longer.images[0].playback = loading.images[0].playback;
        let prepared = prepare(&mut textures, &longer, later(80));
        assert_eq!(prepared.wanted, [(1, vec![12])]);
        textures.receive(1, &[12], Some(vec![frame(12)]));
        let prepared = prepare(&mut textures, &longer, later(80));
        assert_eq!(showing(&textures, &prepared), Some(12));
    }

    #[test]
    fn a_stopped_animation_shows_the_frame_the_program_picked() {
        let mut graphics = animated(&[40, 40], PlaybackState::Stopped, 0);
        let mut textures = Textures::default();
        let start = Instant::now();
        prepare(&mut textures, &graphics, start);
        textures.receive(1, &[10, 11], Some(vec![frame(10), frame(11)]));
        let prepared = prepare(&mut textures, &graphics, start);
        assert_eq!(
            (showing(&textures, &prepared), prepared.next),
            (Some(10), None)
        );
        graphics.images[0].playback.frame = 1;
        let prepared = prepare(&mut textures, &graphics, start);
        assert_eq!(showing(&textures, &prepared), Some(11));
    }

    fn convert(textures: &mut Textures, prepared: Prepared) -> bool {
        let mut done = true;
        for conversion in prepared.conversions {
            done = textures.converted(conversion.run());
        }
        done
    }

    fn still(id: u32, generation: u64, pixels: bool) -> Graphics {
        let mut graphics = animated(&[], PlaybackState::Stopped, 0);
        graphics.images[0].id = id;
        graphics.images[0].generation = generation;
        graphics.placements[0].image = id;
        if pixels {
            graphics.images[0].rgba = vec![0; 4].into();
        }
        graphics
    }

    #[test]
    fn pixels_sent_with_the_screen_need_no_request_and_old_frames_retire() {
        let mut graphics = still(1, 1, true);
        let mut textures = Textures::default();
        let start = Instant::now();
        let prepared = prepare(&mut textures, &graphics, start);
        assert!(prepared.wanted.is_empty());
        assert_eq!(prepared.conversions.len(), 1);
        assert!(convert(&mut textures, prepared));
        let prepared = prepare(&mut textures, &graphics, start);
        assert!(prepared.conversions.is_empty());
        assert_eq!(showing(&textures, &prepared), Some(1));
        graphics.images[0].generation = 2;
        let prepared = prepare(&mut textures, &graphics, start);
        assert_eq!(showing(&textures, &prepared), Some(1));
        assert!(convert(&mut textures, prepared));
        let prepared = prepare(&mut textures, &graphics, start);
        assert_eq!(showing(&textures, &prepared), Some(2));
        assert_eq!(textures.retired().count(), 1);
    }

    #[test]
    fn a_new_image_in_place_of_another_shows_once_its_texture_is_made() {
        let mut textures = Textures::default();
        let start = Instant::now();
        let first = still(1, 1, true);
        let prepared = prepare(&mut textures, &first, start);
        convert(&mut textures, prepared);
        let shown = prepare(&mut textures, &first, start).placements[0]
            .image
            .clone();

        let second = still(2, 7, true);
        let prepared = prepare(&mut textures, &second, start);
        assert!(Arc::ptr_eq(&prepared.placements[0].image, &shown));
        assert_eq!(textures.retired().count(), 0);
        assert!(convert(&mut textures, prepared));
        let prepared = prepare(&mut textures, &second, start);
        assert_eq!(showing(&textures, &prepared), Some(7));
        assert!(!Arc::ptr_eq(&prepared.placements[0].image, &shown));
        assert_eq!(textures.retired().count(), 1);

        let prepared = prepare(&mut textures, &Graphics::default(), start);
        assert!(prepared.placements.is_empty());
        assert_eq!(textures.retired().count(), 1);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn shared_pixels_become_a_texture_and_lost_ones_are_asked_for() -> std::io::Result<()> {
        let mut textures = Textures::default();
        let start = Instant::now();
        let region = muxy_protocol::shared_memory::Region::create(&[1, 2, 3, 255])?;
        let mut shared = still(1, 1, false);
        shared.images[0].shared = Some(muxy_protocol::SharedPixels {
            name: region.name().to_owned(),
        });
        let prepared = prepare(&mut textures, &shared, start);
        assert!(prepared.wanted.is_empty());
        assert!(convert(&mut textures, prepared));
        let texture = textures
            .frames
            .get(&(1, 1))
            .ok_or(std::io::ErrorKind::NotFound)?;
        assert_eq!(texture.as_bytes(0), Some([3, 2, 1, 255].as_slice()));

        let mut lost = still(2, 2, false);
        lost.images[0].shared = Some(muxy_protocol::SharedPixels {
            name: region.name().to_owned(),
        });
        let prepared = prepare(&mut textures, &lost, start);
        assert!(prepared.wanted.is_empty());
        assert!(convert(&mut textures, prepared));
        assert!(!textures.has_frame(2, 2));
        let prepared = prepare(&mut textures, &lost, start);
        assert!(prepared.conversions.is_empty());
        assert_eq!(prepared.wanted, [(2, vec![2])]);
        Ok(())
    }

    #[test]
    fn a_picture_converted_before_it_was_painted_shows_until_a_newer_one_is_ready() {
        let mut textures = Textures::default();
        let start = Instant::now();
        let first = still(1, 1, true);
        let prepared = prepare(&mut textures, &first, start);
        assert!(convert(&mut textures, prepared));
        let second = still(1, 2, true);
        let conversions = textures.convert(&[&second]);
        let prepared = prepare(&mut textures, &second, start);
        assert_eq!(showing(&textures, &prepared), Some(1));
        for conversion in conversions {
            textures.converted(conversion.run());
        }
        let third = still(1, 3, true);
        let conversions = textures.convert(&[&third]);
        assert!(textures.has_frame(1, 2));
        for conversion in conversions {
            textures.converted(conversion.run());
        }
        let prepared = prepare(&mut textures, &third, start);
        assert_eq!(showing(&textures, &prepared), Some(3));
    }

    #[test]
    fn an_image_converting_while_scrolled_back_is_kept_for_the_live_screen() {
        let mut textures = Textures::default();
        let start = Instant::now();
        let live = still(1, 1, true);
        let history = still(2, 2, true);
        let started = textures.convert(&[&live, &history]);
        let painted = textures.prepare(&history, Some(&live), Point::default(), CELL, start, true);
        for conversion in started.into_iter().chain(painted.conversions) {
            textures.converted(conversion.run());
        }
        assert!(textures.has_frame(1, 1));
    }

    #[test]
    fn a_new_image_read_from_the_server_shows_the_old_one_until_answered() {
        let mut textures = Textures::default();
        let start = Instant::now();
        let first = still(1, 1, false);
        prepare(&mut textures, &first, start);
        textures.receive(1, &[1], Some(vec![frame(1)]));
        let shown = prepare(&mut textures, &first, start).placements[0]
            .image
            .clone();
        let second = still(2, 2, false);
        let prepared = prepare(&mut textures, &second, start);
        assert_eq!(prepared.wanted, [(2, vec![2])]);
        assert!(Arc::ptr_eq(&prepared.placements[0].image, &shown));
        textures.receive(2, &[2], Some(Vec::new()));
        let prepared = prepare(&mut textures, &second, start);
        assert!(prepared.wanted.is_empty());
        assert!(prepared.placements.is_empty());
    }

    #[test]
    fn screens_taken_without_painting_keep_only_what_they_list() {
        let mut textures = Textures::default();
        let start = Instant::now();
        let shown = still(1, 1, true);
        let prepared = prepare(&mut textures, &shown, start);
        convert(&mut textures, prepared);
        prepare(&mut textures, &shown, start);
        for id in 2..10 {
            let conversions = textures.convert(&[&still(id, id.into(), true)]);
            for conversion in conversions {
                textures.converted(conversion.run());
            }
        }
        assert_eq!(textures.frames.len(), 2);
        assert_eq!(textures.retired().count(), 0);
        let prepared = prepare(&mut textures, &still(9, 9, true), start);
        assert_eq!(showing(&textures, &prepared), Some(9));
        assert_eq!(textures.frames.len(), 1);
        assert_eq!(textures.retired().count(), 1);
    }

    #[test]
    fn a_texture_started_before_a_reset_is_dropped() {
        let mut textures = Textures::default();
        let start = Instant::now();
        let graphics = still(1, 1, true);
        let prepared = prepare(&mut textures, &graphics, start);
        textures.reset();
        assert!(convert(&mut textures, prepared));
        assert!(textures.frames.is_empty());
        let prepared = prepare(&mut textures, &graphics, start);
        assert_eq!(prepared.conversions.len(), 1);
    }

    #[test]
    fn a_replaced_picture_stays_on_screen_until_its_pixels_arrive() {
        let mut graphics = animated(&[], PlaybackState::Stopped, 0);
        let mut textures = Textures::default();
        let start = Instant::now();
        prepare(&mut textures, &graphics, start);
        textures.receive(1, &[1], Some(vec![frame(1)]));
        graphics.images[0].generation = 2;
        let prepared = prepare(&mut textures, &graphics, start);
        assert_eq!(prepared.wanted, [(1, vec![2])]);
        assert_eq!(showing(&textures, &prepared), Some(1));
        assert!(!textures.receive(1, &[2], Some(vec![frame(3)])));
        let prepared = prepare(&mut textures, &graphics, start);
        assert!(prepared.wanted.is_empty());
        assert_eq!(showing(&textures, &prepared), Some(3));
        assert_eq!(textures.retired().count(), 1);
        graphics.images[0].generation = 3;
        let prepared = prepare(&mut textures, &graphics, start);
        assert!(prepared.wanted.is_empty());
        assert_eq!(showing(&textures, &prepared), Some(3));
    }

    #[test]
    fn a_failed_request_is_asked_again_and_few_are_out_at_once() {
        let mut graphics = animated(&[], PlaybackState::Stopped, 0);
        let first = graphics.images[0].clone();
        let placed = graphics.placements[0].clone();
        for id in 2..=6 {
            graphics.images.push(GraphicImage {
                id,
                ..first.clone()
            });
            graphics.placements.push(GraphicPlacement {
                image: id,
                ..placed.clone()
            });
        }
        let mut textures = Textures::default();
        let start = Instant::now();
        let wanted = prepare(&mut textures, &graphics, start).wanted;
        assert_eq!(wanted.len(), MAX_REQUESTS);
        assert!(prepare(&mut textures, &graphics, start).wanted.is_empty());
        assert!(textures.receive(1, &[1], None));
        assert!(!textures.receive(2, &[1], Some(vec![frame(1)])));
        let images = |prepared: Prepared| -> Vec<u32> {
            prepared.wanted.iter().map(|(image, _)| *image).collect()
        };
        assert_eq!(images(prepare(&mut textures, &graphics, start)), [5, 6]);
        assert!(prepare(&mut textures, &graphics, start).wanted.is_empty());
        assert!(!textures.receive(3, &[1], Some(vec![frame(1)])));
        assert!(prepare(&mut textures, &graphics, start).wanted.is_empty());
        textures.retry();
        assert_eq!(images(prepare(&mut textures, &graphics, start)), [1]);
        let mut idle = Textures::default();
        let prepared = idle.prepare(&graphics, None, Point::default(), CELL, start, false);
        assert!(prepared.wanted.is_empty());
        assert!(
            !idle
                .prepare(&graphics, None, Point::default(), CELL, start, true)
                .wanted
                .is_empty()
        );
    }

    #[test]
    fn an_animation_that_finishes_loading_plays_on_from_its_frame() {
        let mut graphics = animated(&[40, 40, 40], PlaybackState::Loading, 0);
        let mut textures = Textures::default();
        let start = Instant::now();
        let later = |millis: u64| start + Duration::from_millis(millis);
        prepare(&mut textures, &graphics, start);
        textures.receive(
            1,
            &[10, 11, 12],
            Some(vec![frame(10), frame(11), frame(12)]),
        );
        prepare(&mut textures, &graphics, later(40));
        graphics.images[0].playback.state = PlaybackState::Running;
        let prepared = prepare(&mut textures, &graphics, later(50));
        assert_eq!(showing(&textures, &prepared), Some(11));
        let prepared = prepare(&mut textures, &graphics, later(80));
        assert_eq!(showing(&textures, &prepared), Some(12));
    }
}
