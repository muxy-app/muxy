use crate::store::{PrivateDirectory, ensure_private_directory};
use image::{DynamicImage, ImageFormat};
use std::collections::HashSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct PreparedImageSource {
    contents: Vec<u8>,
    extension: &'static str,
}

impl PreparedImageSource {
    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}

pub const MAX_ENCODED_IMAGE_BYTES: usize = 25 * 1024 * 1024;
pub const MAX_DECODED_IMAGE_PIXELS: u64 = 64_000_000;
const ALLOWED_EXTENSIONS: [&str; 5] = ["png", "jpg", "gif", "tiff", "webp"];

#[derive(Debug)]
pub struct ImageStorage {
    directory: PrivateDirectory,
}

impl ImageStorage {
    pub fn open(profile_root: &Path) -> std::io::Result<Self> {
        let directory = ensure_private_directory(&profile_root.join(super::IMAGES_DIRECTORY_NAME))?;
        Ok(Self { directory })
    }

    pub fn directory_path(&self) -> &Path {
        self.directory.path()
    }

    pub fn write_source(&self, contents: &[u8]) -> std::io::Result<String> {
        let source = prepare_image_source(contents.to_vec())?;
        self.write_prepared(&source)
    }

    pub fn write_prepared(&self, source: &PreparedImageSource) -> std::io::Result<String> {
        self.write_with_extension(source.extension, source.contents())
    }

    pub fn read(&self, filename: &str) -> std::io::Result<Vec<u8>> {
        validate_image_filename(filename)?;
        self.directory.read_regular(filename)
    }

    pub fn normalize_png(&self, filename: &str) -> std::io::Result<Vec<u8>> {
        normalize_png(&self.read(filename)?)
    }

    pub fn path_for(&self, filename: &str) -> std::io::Result<PathBuf> {
        validate_image_filename(filename)?;
        Ok(self.directory.path().join(filename))
    }

    pub fn remove(&self, filename: &str) -> std::io::Result<bool> {
        validate_image_filename(filename)?;
        self.directory.remove_regular(filename)
    }

    pub fn sweep(&self, referenced: &HashSet<String>) -> std::io::Result<Vec<String>> {
        let mut removed = Vec::new();
        for filename in self.directory.regular_file_names()? {
            if !referenced.contains(&filename) && self.directory.remove_regular(&filename)? {
                removed.push(filename);
            }
        }
        Ok(removed)
    }

    pub fn regular_file_names(&self) -> std::io::Result<Vec<String>> {
        self.directory.regular_file_names()
    }

    fn write_with_extension(&self, extension: &str, contents: &[u8]) -> std::io::Result<String> {
        if !ALLOWED_EXTENSIONS.contains(&extension) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unsupported Composer image extension",
            ));
        }
        loop {
            let filename = format!("{}.{}", crate::store::new_uuid(), extension);
            match self.directory.write_new_atomic(&filename, contents) {
                Ok(()) => return Ok(filename),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

pub fn validate_image_filename(filename: &str) -> std::io::Result<()> {
    let path = Path::new(filename);
    if path.file_name().and_then(std::ffi::OsStr::to_str) != Some(filename)
        || path.components().count() != 1
        || filename.contains(['/', '\\'])
        || filename == "."
        || filename == ".."
    {
        return Err(invalid_filename());
    }
    let Some((stem, extension)) = filename.rsplit_once('.') else {
        return Err(invalid_filename());
    };
    if crate::notifications::canonical_uuid(stem).as_deref() != Some(stem)
        || !ALLOWED_EXTENSIONS.contains(&extension)
    {
        return Err(invalid_filename());
    }
    Ok(())
}

fn invalid_filename() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid Composer image filename",
    )
}

pub fn prepare_image_source(contents: Vec<u8>) -> std::io::Result<PreparedImageSource> {
    let (format, extension) = source_format(&contents)?;
    decode_image(&contents, format)?;
    Ok(PreparedImageSource {
        contents,
        extension,
    })
}

pub fn normalize_png(contents: &[u8]) -> std::io::Result<Vec<u8>> {
    let (format, _) = source_format(contents)?;
    let image = decode_image(contents, format)?;
    let mut normalized = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut normalized), ImageFormat::Png)
        .map_err(std::io::Error::other)?;
    Ok(normalized)
}

fn source_format(contents: &[u8]) -> std::io::Result<(ImageFormat, &'static str)> {
    if contents.is_empty() || contents.len() > MAX_ENCODED_IMAGE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Composer image source size is invalid",
        ));
    }
    let format = image::guess_format(contents).map_err(std::io::Error::other)?;
    let extension = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Gif => "gif",
        ImageFormat::Tiff => "tiff",
        ImageFormat::WebP => "webp",
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported Composer image format",
            ));
        }
    };
    Ok((format, extension))
}

fn decode_image(contents: &[u8], format: ImageFormat) -> std::io::Result<DynamicImage> {
    let reader = image::ImageReader::with_format(Cursor::new(contents), format);
    let (width, height) = reader.into_dimensions().map_err(std::io::Error::other)?;
    validate_dimensions(width, height)?;
    image::ImageReader::with_format(Cursor::new(contents), format)
        .decode()
        .map_err(std::io::Error::other)
}

fn validate_dimensions(width: u32, height: u32) -> std::io::Result<()> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels == 0 || pixels > MAX_DECODED_IMAGE_PIXELS {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Composer image dimensions are invalid",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    fn encoded(format: ImageFormat) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 2, Rgba([1, 2, 3, 255])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), format)
            .unwrap();
        bytes
    }

    fn png() -> Vec<u8> {
        encoded(ImageFormat::Png)
    }

    #[test]
    fn composer_image_filenames_require_uppercase_uuid_basenames_and_allowed_extensions() {
        let valid = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.png";
        assert!(validate_image_filename(valid).is_ok());
        for invalid in [
            "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee.png",
            "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.bmp",
            "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.PNG",
            "../AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.png",
            "folder/AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.png",
            "..",
        ] {
            assert!(validate_image_filename(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn composer_image_storage_writes_reads_and_removes_private_sources() {
        let profile = tempfile::tempdir().unwrap();
        let storage = ImageStorage::open(profile.path()).unwrap();
        let filename = storage.write_source(&png()).unwrap();
        validate_image_filename(&filename).unwrap();
        assert_eq!(storage.read(&filename).unwrap(), png());
        assert!(storage.remove(&filename).unwrap());
        assert!(!storage.remove(&filename).unwrap());
    }

    #[test]
    fn composer_image_storage_rejects_empty_invalid_and_oversized_sources() {
        let profile = tempfile::tempdir().unwrap();
        let storage = ImageStorage::open(profile.path()).unwrap();
        assert!(storage.write_source(&[]).is_err());
        assert!(storage.write_source(b"not an image").is_err());
        let mut corrupted = png();
        let corrupted_index = corrupted.len() - 13;
        corrupted[corrupted_index] ^= 0xff;
        assert!(
            image::ImageReader::with_format(Cursor::new(&corrupted), ImageFormat::Png)
                .into_dimensions()
                .is_ok()
        );
        assert!(storage.write_source(&corrupted).is_err());
        assert!(
            storage
                .write_source(&vec![0; MAX_ENCODED_IMAGE_BYTES + 1])
                .is_err()
        );
    }

    #[test]
    fn composer_image_normalization_accepts_supported_sources_and_outputs_png() {
        for format in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Gif,
            ImageFormat::Tiff,
            ImageFormat::WebP,
        ] {
            let normalized = normalize_png(&encoded(format)).unwrap();
            assert_eq!(image::guess_format(&normalized).unwrap(), ImageFormat::Png);
            let decoded =
                image::load_from_memory_with_format(&normalized, ImageFormat::Png).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (2, 2));
        }
    }

    #[test]
    fn composer_image_pixel_limit_accepts_the_boundary_and_rejects_larger_or_empty_dimensions() {
        assert!(validate_dimensions(8_000, 8_000).is_ok());
        assert!(validate_dimensions(8_001, 8_000).is_err());
        assert!(validate_dimensions(0, 8_000).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn composer_image_sweep_ignores_symlinks_and_removes_only_regular_orphans() {
        use std::os::unix::fs::symlink;

        let profile = tempfile::tempdir().unwrap();
        let storage = ImageStorage::open(profile.path()).unwrap();
        let retained = storage.write_source(&png()).unwrap();
        let orphan = storage.write_source(&png()).unwrap();
        let outside = profile.path().join("outside.png");
        std::fs::write(&outside, b"outside").unwrap();
        let linked = storage.directory_path().join("linked.png");
        symlink(&outside, &linked).unwrap();
        let removed = storage.sweep(&HashSet::from([retained.clone()])).unwrap();
        assert_eq!(removed, vec![orphan]);
        assert!(storage.read(&retained).is_ok());
        assert_eq!(std::fs::read(outside).unwrap(), b"outside");
        assert!(linked.symlink_metadata().unwrap().file_type().is_symlink());
    }
}
