use crate::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, DevicePixels, PlatformAtlas,
    Point, Size, platform::AtlasTextureList,
};
use anyhow::Result;
use blade_graphics as gpu;
use blade_util::{BufferBelt, BufferBeltDescriptor};
use collections::FxHashMap;
use etagere::BucketedAtlasAllocator;
use parking_lot::Mutex;
use std::{borrow::Cow, ops, sync::Arc};

pub(crate) struct BladeAtlas(Mutex<BladeAtlasState>);

struct PendingUpload {
    id: AtlasTextureId,
    bounds: Bounds<DevicePixels>,
    data: gpu::BufferPiece,
}

struct BladeAtlasState {
    gpu: Arc<gpu::Context>,
    upload_belt: BufferBelt,
    storage: BladeAtlasStorage,
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
    initializations: Vec<AtlasTextureId>,
    uploads: Vec<PendingUpload>,
    /// Textures that lost their last tile since the last frame was submitted.
    /// A frame still on the GPU may sample them.
    retired: Vec<BladeAtlasTexture>,
    /// Retired textures, each with the frame after which nothing samples it.
    expiring: Vec<(gpu::SyncPoint, BladeAtlasTexture)>,
}

#[cfg(gles)]
unsafe impl Send for BladeAtlasState {}

impl BladeAtlasState {
    fn destroy(&mut self) {
        for mut texture in self.retired.drain(..) {
            texture.destroy(&self.gpu);
        }
        for (_, mut texture) in self.expiring.drain(..) {
            texture.destroy(&self.gpu);
        }
        self.storage.destroy(&self.gpu);
        self.upload_belt.destroy(&self.gpu);
    }

    /// Keeps a texture alive until the GPU finishes the frames that may still
    /// sample it. Destroying it earlier is undefined behavior in Vulkan, and on
    /// Intel GPUs it faults the frame in flight and loses the device.
    fn retire(&mut self, texture: BladeAtlasTexture) {
        let id = texture.id;
        self.initializations.retain(|pending| *pending != id);
        self.uploads.retain(|upload| upload.id != id);
        self.retired.push(texture);
    }

    fn expire_retired(&mut self, sync_point: &gpu::SyncPoint) {
        self.expiring.extend(
            self.retired
                .drain(..)
                .map(|texture| (sync_point.clone(), texture)),
        );
    }

    fn destroy_expired(&mut self) {
        let gpu = &self.gpu;
        self.expiring.retain_mut(|(sync_point, texture)| {
            if gpu.wait_for(sync_point, 0) {
                texture.destroy(gpu);
                false
            } else {
                true
            }
        });
    }
}

pub struct BladeTextureInfo {
    pub raw_view: gpu::TextureView,
}

impl BladeAtlas {
    pub(crate) fn new(gpu: &Arc<gpu::Context>) -> Self {
        BladeAtlas(Mutex::new(BladeAtlasState {
            gpu: Arc::clone(gpu),
            upload_belt: BufferBelt::new(BufferBeltDescriptor {
                memory: gpu::Memory::Upload,
                min_chunk_size: 0x10000,
                alignment: 64, // Vulkan `optimalBufferCopyOffsetAlignment` on Intel XE
            }),
            storage: BladeAtlasStorage::default(),
            tiles_by_key: Default::default(),
            initializations: Vec::new(),
            uploads: Vec::new(),
            retired: Vec::new(),
            expiring: Vec::new(),
        }))
    }

    pub(crate) fn destroy(&self) {
        self.0.lock().destroy();
    }

    pub fn before_frame(&self, gpu_encoder: &mut gpu::CommandEncoder) {
        let mut lock = self.0.lock();
        lock.destroy_expired();
        lock.flush(gpu_encoder);
    }

    pub fn after_frame(&self, sync_point: &gpu::SyncPoint) {
        let mut lock = self.0.lock();
        lock.upload_belt.flush(sync_point);
        lock.expire_retired(sync_point);
    }

    pub fn get_texture_info(&self, id: AtlasTextureId) -> BladeTextureInfo {
        let lock = self.0.lock();
        let texture = &lock.storage[id];
        BladeTextureInfo {
            raw_view: texture.raw_view,
        }
    }
}

impl PlatformAtlas for BladeAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        let mut lock = self.0.lock();
        if let Some(tile) = lock.tiles_by_key.get(key) {
            Ok(Some(tile.clone()))
        } else {
            profiling::scope!("new tile");
            let Some((size, bytes)) = build()? else {
                return Ok(None);
            };
            let tile = lock.allocate(size, key.texture_kind());
            lock.upload_texture(tile.texture_id, tile.bounds, &bytes);
            lock.tiles_by_key.insert(key.clone(), tile.clone());
            Ok(Some(tile))
        }
    }

    fn remove(&self, key: &AtlasKey) {
        let mut lock = self.0.lock();

        let Some(id) = lock.tiles_by_key.remove(key).map(|tile| tile.texture_id) else {
            return;
        };

        let Some(texture_slot) = lock.storage[id.kind].textures.get_mut(id.index as usize) else {
            return;
        };

        if let Some(mut texture) = texture_slot.take() {
            texture.decrement_ref_count();
            if texture.is_unreferenced() {
                lock.storage[id.kind]
                    .free_list
                    .push(texture.id.index as usize);
                lock.retire(texture);
            } else {
                *texture_slot = Some(texture);
            }
        }
    }
}

impl BladeAtlasState {
    fn allocate(&mut self, size: Size<DevicePixels>, texture_kind: AtlasTextureKind) -> AtlasTile {
        {
            let textures = &mut self.storage[texture_kind];

            if let Some(tile) = textures
                .iter_mut()
                .rev()
                .find_map(|texture| texture.allocate(size))
            {
                return tile;
            }
        }

        let texture = self.push_texture(size, texture_kind);
        texture.allocate(size).unwrap()
    }

    fn push_texture(
        &mut self,
        min_size: Size<DevicePixels>,
        kind: AtlasTextureKind,
    ) -> &mut BladeAtlasTexture {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };

        let size = min_size.max(&DEFAULT_ATLAS_SIZE);
        let format;
        let usage;
        match kind {
            AtlasTextureKind::Monochrome => {
                format = gpu::TextureFormat::R8Unorm;
                usage = gpu::TextureUsage::COPY | gpu::TextureUsage::RESOURCE;
            }
            AtlasTextureKind::Polychrome => {
                format = gpu::TextureFormat::Bgra8Unorm;
                usage = gpu::TextureUsage::COPY | gpu::TextureUsage::RESOURCE;
            }
        }

        let raw = self.gpu.create_texture(gpu::TextureDesc {
            name: "atlas",
            format,
            size: gpu::Extent {
                width: size.width.into(),
                height: size.height.into(),
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            usage,
            external: None,
        });
        let raw_view = self.gpu.create_texture_view(
            raw,
            gpu::TextureViewDesc {
                name: "",
                format,
                dimension: gpu::ViewDimension::D2,
                subresources: &Default::default(),
            },
        );

        let texture_list = &mut self.storage[kind];
        let index = texture_list.free_list.pop();

        let atlas_texture = BladeAtlasTexture {
            id: AtlasTextureId {
                index: index.unwrap_or(texture_list.textures.len()) as u32,
                kind,
            },
            allocator: etagere::BucketedAtlasAllocator::new(size.into()),
            format,
            raw,
            raw_view,
            live_atlas_keys: 0,
        };

        self.initializations.push(atlas_texture.id);

        if let Some(ix) = index {
            texture_list.textures[ix] = Some(atlas_texture);
            texture_list.textures.get_mut(ix).unwrap().as_mut().unwrap()
        } else {
            texture_list.textures.push(Some(atlas_texture));
            texture_list.textures.last_mut().unwrap().as_mut().unwrap()
        }
    }

    fn upload_texture(&mut self, id: AtlasTextureId, bounds: Bounds<DevicePixels>, bytes: &[u8]) {
        let data = self.upload_belt.alloc_bytes(bytes, &self.gpu);
        self.uploads.push(PendingUpload { id, bounds, data });
    }

    fn flush_initializations(&mut self, encoder: &mut gpu::CommandEncoder) {
        for id in self.initializations.drain(..) {
            let texture = &self.storage[id];
            encoder.init_texture(texture.raw);
        }
    }

    fn flush(&mut self, encoder: &mut gpu::CommandEncoder) {
        self.flush_initializations(encoder);

        let mut transfers = encoder.transfer("atlas");
        for upload in self.uploads.drain(..) {
            let texture = &self.storage[upload.id];
            transfers.copy_buffer_to_texture(
                upload.data,
                upload.bounds.size.width.to_bytes(texture.bytes_per_pixel()),
                gpu::TexturePiece {
                    texture: texture.raw,
                    mip_level: 0,
                    array_layer: 0,
                    origin: [
                        upload.bounds.origin.x.into(),
                        upload.bounds.origin.y.into(),
                        0,
                    ],
                },
                gpu::Extent {
                    width: upload.bounds.size.width.into(),
                    height: upload.bounds.size.height.into(),
                    depth: 1,
                },
            );
        }
    }
}

#[derive(Default)]
struct BladeAtlasStorage {
    monochrome_textures: AtlasTextureList<BladeAtlasTexture>,
    polychrome_textures: AtlasTextureList<BladeAtlasTexture>,
}

impl ops::Index<AtlasTextureKind> for BladeAtlasStorage {
    type Output = AtlasTextureList<BladeAtlasTexture>;
    fn index(&self, kind: AtlasTextureKind) -> &Self::Output {
        match kind {
            crate::AtlasTextureKind::Monochrome => &self.monochrome_textures,
            crate::AtlasTextureKind::Polychrome => &self.polychrome_textures,
        }
    }
}

impl ops::IndexMut<AtlasTextureKind> for BladeAtlasStorage {
    fn index_mut(&mut self, kind: AtlasTextureKind) -> &mut Self::Output {
        match kind {
            crate::AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            crate::AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
        }
    }
}

impl ops::Index<AtlasTextureId> for BladeAtlasStorage {
    type Output = BladeAtlasTexture;
    fn index(&self, id: AtlasTextureId) -> &Self::Output {
        let textures = match id.kind {
            crate::AtlasTextureKind::Monochrome => &self.monochrome_textures,
            crate::AtlasTextureKind::Polychrome => &self.polychrome_textures,
        };
        textures[id.index as usize].as_ref().unwrap()
    }
}

impl BladeAtlasStorage {
    fn destroy(&mut self, gpu: &gpu::Context) {
        for mut texture in self.monochrome_textures.drain().flatten() {
            texture.destroy(gpu);
        }
        for mut texture in self.polychrome_textures.drain().flatten() {
            texture.destroy(gpu);
        }
    }
}

struct BladeAtlasTexture {
    id: AtlasTextureId,
    allocator: BucketedAtlasAllocator,
    raw: gpu::Texture,
    raw_view: gpu::TextureView,
    format: gpu::TextureFormat,
    live_atlas_keys: u32,
}

impl BladeAtlasTexture {
    fn allocate(&mut self, size: Size<DevicePixels>) -> Option<AtlasTile> {
        let allocation = self.allocator.allocate(size.into())?;
        let tile = AtlasTile {
            texture_id: self.id,
            tile_id: allocation.id.into(),
            padding: 0,
            bounds: Bounds {
                origin: allocation.rectangle.min.into(),
                size,
            },
        };
        self.live_atlas_keys += 1;
        Some(tile)
    }

    fn destroy(&mut self, gpu: &gpu::Context) {
        gpu.destroy_texture_view(self.raw_view);
        gpu.destroy_texture(self.raw);
    }

    fn bytes_per_pixel(&self) -> u8 {
        self.format.block_info().size
    }

    fn decrement_ref_count(&mut self) {
        self.live_atlas_keys -= 1;
    }

    fn is_unreferenced(&mut self) -> bool {
        self.live_atlas_keys == 0
    }
}

impl From<Size<DevicePixels>> for etagere::Size {
    fn from(size: Size<DevicePixels>) -> Self {
        etagere::Size::new(size.width.into(), size.height.into())
    }
}

impl From<etagere::Point> for Point<DevicePixels> {
    fn from(value: etagere::Point) -> Self {
        Point {
            x: DevicePixels::from(value.x),
            y: DevicePixels::from(value.y),
        }
    }
}

impl From<etagere::Size> for Size<DevicePixels> {
    fn from(size: etagere::Size) -> Self {
        Size {
            width: DevicePixels::from(size.width),
            height: DevicePixels::from(size.height),
        }
    }
}

impl From<etagere::Rectangle> for Bounds<DevicePixels> {
    fn from(rectangle: etagere::Rectangle) -> Self {
        Bounds {
            origin: rectangle.min.into(),
            size: rectangle.size().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImageId, RenderImageParams, size};

    fn key(id: usize) -> AtlasKey {
        AtlasKey::Image(RenderImageParams {
            image_id: ImageId(id),
            frame_index: 0,
        })
    }

    /// A headless GPU, or `None` on a machine without one.
    fn gpu() -> Option<Arc<gpu::Context>> {
        unsafe {
            gpu::Context::init(gpu::ContextDesc {
                presentation: false,
                validation: false,
                ..Default::default()
            })
        }
        .ok()
        .map(Arc::new)
    }

    fn insert(atlas: &BladeAtlas, id: usize, width: i32, height: i32) -> AtlasTile {
        atlas
            .get_or_insert_with(&key(id), &mut || {
                Ok(Some((
                    size(DevicePixels(width), DevicePixels(height)),
                    Cow::Owned(vec![0; (width * height * 4) as usize]),
                )))
            })
            .unwrap()
            .unwrap()
    }

    /// Submits a frame the way the renderer does.
    fn frame(
        gpu: &gpu::Context,
        atlas: &BladeAtlas,
        encoder: &mut gpu::CommandEncoder,
    ) -> gpu::SyncPoint {
        encoder.start();
        atlas.before_frame(encoder);
        let sync_point = gpu.submit(encoder);
        atlas.after_frame(&sync_point);
        sync_point
    }

    #[test]
    fn a_removed_texture_outlives_the_frames_that_may_sample_it() {
        let Some(gpu) = gpu() else {
            return;
        };
        let atlas = BladeAtlas::new(&gpu);
        let mut encoder = gpu.create_command_encoder(gpu::CommandEncoderDesc {
            name: "test",
            buffer_count: 2,
        });
        insert(&atlas, 0, 1200, 1600);
        frame(&gpu, &atlas, &mut encoder);
        atlas.remove(&key(0));
        assert_eq!(atlas.0.lock().retired.len(), 1);

        let next = frame(&gpu, &atlas, &mut encoder);
        {
            let state = atlas.0.lock();
            assert!(state.retired.is_empty());
            assert_eq!(state.expiring.len(), 1);
        }

        assert!(gpu.wait_for(&next, MAX_WAIT_MS));
        let last = frame(&gpu, &atlas, &mut encoder);
        assert!(atlas.0.lock().expiring.is_empty());

        assert!(gpu.wait_for(&last, MAX_WAIT_MS));
        gpu.destroy_command_encoder(&mut encoder);
        atlas.destroy();
    }

    #[test]
    fn a_texture_removed_before_its_first_frame_takes_its_pending_work() {
        let Some(gpu) = gpu() else {
            return;
        };
        let atlas = BladeAtlas::new(&gpu);
        let snapshot = insert(&atlas, 0, 1200, 1600);
        atlas.remove(&key(0));
        let icon = insert(&atlas, 1, 34, 32);
        assert_eq!(icon.texture_id, snapshot.texture_id);
        {
            let state = atlas.0.lock();
            assert_eq!(state.initializations, [icon.texture_id]);
            assert_eq!(state.uploads.len(), 1);
            assert_eq!(state.uploads[0].bounds, icon.bounds);
        }
        atlas.destroy();
    }

    const MAX_WAIT_MS: u32 = 10_000;
}
