mod source;

pub use source::{
    MAX_DECODED_IMAGE_PIXELS, MAX_ENCODED_IMAGE_BYTES, PreparedImageSource, normalize_png,
    prepare_image_source,
};

use super::storage::{PrivateDirectory, ensure_private_directory};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
        self.directory.verify()?;
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

    fn write_with_extension(&self, extension: &str, contents: &[u8]) -> std::io::Result<String> {
        if !ALLOWED_EXTENSIONS.contains(&extension) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unsupported Composer image extension",
            ));
        }
        loop {
            let filename = format!(
                "{}.{}",
                uuid::Uuid::new_v4().hyphenated().to_string().to_uppercase(),
                extension
            );
            match self.directory.write_new_atomic(&filename, contents) {
                Ok(()) => return Ok(filename),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
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
    if super::canonical_uuid(stem).as_deref() != Some(stem)
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    fn encoded(format: ImageFormat) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 2, Rgba([1, 2, 3, 255])));
        let image = if format == ImageFormat::Jpeg {
            DynamicImage::ImageRgb8(image.to_rgb8())
        } else {
            image
        };
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
    fn prepared_sources_preserve_formats_and_normalize_without_changing_pixels() {
        let profile = tempfile::tempdir().unwrap();
        let storage = ImageStorage::open(profile.path()).unwrap();
        for (format, extension) in [
            (ImageFormat::Png, "png"),
            (ImageFormat::Jpeg, "jpg"),
            (ImageFormat::Gif, "gif"),
            (ImageFormat::Tiff, "tiff"),
            (ImageFormat::WebP, "webp"),
        ] {
            let bytes = encoded(format);
            let source = prepare_image_source(bytes.clone()).unwrap();
            assert_eq!(source.contents(), bytes);
            let filename = storage.write_prepared(&source).unwrap();
            assert_eq!(Path::new(&filename).extension().unwrap(), extension);
            assert_eq!(storage.read(&filename).unwrap(), bytes);
            let normalized = storage.normalize_png(&filename).unwrap();
            assert_eq!(image::guess_format(&normalized).unwrap(), ImageFormat::Png);
            assert_eq!(
                image::load_from_memory(&normalized).unwrap().to_rgba8(),
                image::load_from_memory(&bytes).unwrap().to_rgba8(),
                "{format:?}"
            );
        }
    }

    #[test]
    fn invalid_sources_are_rejected_before_publishing_an_image() {
        let profile = tempfile::tempdir().unwrap();
        let storage = ImageStorage::open(profile.path()).unwrap();
        let mut oversized = png();
        oversized.resize(MAX_ENCODED_IMAGE_BYTES + 1, 0);
        for (bytes, kind) in [
            (Vec::new(), std::io::ErrorKind::InvalidData),
            (oversized, std::io::ErrorKind::InvalidData),
            (b"BM".to_vec(), std::io::ErrorKind::InvalidData),
            (png()[..16].to_vec(), std::io::ErrorKind::Other),
        ] {
            assert_eq!(
                prepare_image_source(bytes.clone()).unwrap_err().kind(),
                kind
            );
            assert_eq!(normalize_png(&bytes).unwrap_err().kind(), kind);
            assert_eq!(storage.write_source(&bytes).unwrap_err().kind(), kind);
        }
        assert_eq!(
            std::fs::read_dir(storage.directory_path()).unwrap().count(),
            0
        );
    }

    #[test]
    fn oversized_decoded_images_are_rejected_before_decoding_pixels() {
        let mut bytes = encoded(ImageFormat::Gif);
        bytes[6..8].copy_from_slice(&8001_u16.to_le_bytes());
        bytes[8..10].copy_from_slice(&8000_u16.to_le_bytes());
        for error in [
            prepare_image_source(bytes.clone()).unwrap_err(),
            normalize_png(&bytes).unwrap_err(),
        ] {
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
            assert_eq!(error.to_string(), "Composer image dimensions are invalid");
        }
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
    fn composer_images_never_follow_symlinks_and_sweep_only_regular_orphans() {
        let profile = tempfile::tempdir().unwrap();
        let storage = ImageStorage::open(profile.path()).unwrap();
        let retained = storage.write_source(&png()).unwrap();
        let orphan = storage.write_source(&png()).unwrap();
        let outside = profile.path().join("outside.png");
        std::fs::write(&outside, b"outside").unwrap();
        let linked_name = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.png";
        let linked = storage.directory_path().join(linked_name);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &linked).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&outside, &linked).unwrap();
        assert!(storage.read(linked_name).is_err());
        assert!(!storage.remove(linked_name).unwrap());
        let removed = storage.sweep(&HashSet::from([retained.clone()])).unwrap();
        assert_eq!(removed, vec![orphan]);
        assert!(storage.read(&retained).is_ok());
        assert_eq!(std::fs::read(outside).unwrap(), b"outside");
        assert!(linked.symlink_metadata().unwrap().file_type().is_symlink());
    }

    #[test]
    fn a_published_image_name_is_never_replaced() {
        let profile = tempfile::tempdir().unwrap();
        let storage = ImageStorage::open(profile.path()).unwrap();
        let name = storage.write_source(&png()).unwrap();
        let error = storage
            .directory
            .write_new_atomic(&name, b"other")
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(storage.read(&name).unwrap(), png());
        let leftovers = std::fs::read_dir(storage.directory_path())
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().file_name() != name.as_str())
            .count();
        assert_eq!(leftovers, 0);
    }
}
