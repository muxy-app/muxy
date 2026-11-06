use std::io::Read;
use std::path::{Path, PathBuf};

use muxy_client::Client;
use muxy_protocol::{ProjectId, ServerPath};

mod icns;
#[cfg(test)]
mod tests;

const MAX_BYTES: u64 = 20 * 1024 * 1024;
const DIRECTORIES: &[&str] = &[
    "",
    "public",
    "static",
    "app",
    "src/app",
    "src",
    "src/assets",
    "assets",
    "public/assets",
    "public/images",
    "static/images",
    "resources",
];
const NAMES: &[&str] = &[
    "favicon",
    "icon",
    "logo",
    "apple-touch-icon",
    "apple-icon",
    "android-chrome-192x192",
    "android-chrome-512x512",
    "favicon-32x32",
    "favicon-16x16",
    "apple-touch-icon-precomposed",
];
const EXTENSIONS: &[&str] = &["svg", "ico", "png", "jpg", "jpeg", "webp", "gif", "icns"];

pub(crate) enum Source {
    Local(PathBuf),
    Remote(Client, ProjectId),
}

impl Source {
    fn names(&self, directory: &Path) -> Vec<PathBuf> {
        match self {
            Self::Local(root) => std::fs::read_dir(root.join(directory))
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|entry| PathBuf::from(entry.file_name()))
                .collect(),
            Self::Remote(client, project) => client
                .list_directory(*project, ServerPath::from(directory))
                .unwrap_or_default()
                .into_iter()
                .filter(|entry| !entry.is_directory)
                .map(|entry| PathBuf::from(entry.name))
                .collect(),
        }
    }

    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        match self {
            Self::Local(root) => {
                let root = root.canonicalize().ok()?;
                let path = root.join(path).canonicalize().ok()?;
                if !path.starts_with(&root) {
                    return None;
                }
                let metadata = path.metadata().ok()?;
                if !metadata.is_file() || metadata.len() > MAX_BYTES {
                    return None;
                }
                let mut bytes = Vec::new();
                std::fs::File::open(path)
                    .ok()?
                    .take(MAX_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .ok()?;
                (bytes.len() as u64 <= MAX_BYTES).then_some(bytes)
            }
            Self::Remote(client, project) => {
                let path = ServerPath::from(path);
                let info = client.file_info(*project, path.clone()).ok()?;
                if info.is_directory || info.size > MAX_BYTES {
                    return None;
                }
                let bytes = client.read_file_bytes(*project, path).ok()?.bytes;
                (bytes.len() as u64 <= MAX_BYTES).then_some(bytes)
            }
        }
    }
}

fn rank(path: &Path) -> Option<(usize, usize)> {
    if path.components().count() != 1 {
        return None;
    }
    let name = path.file_stem()?.to_str()?.to_ascii_lowercase();
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some((
        NAMES.iter().position(|candidate| *candidate == name)?,
        EXTENSIONS
            .iter()
            .position(|candidate| *candidate == extension)?,
    ))
}

pub(crate) fn discover(source: &Source) -> Option<gpui::RenderImage> {
    for directory in DIRECTORIES {
        let directory = Path::new(directory);
        let mut candidates: Vec<_> = source
            .names(directory)
            .into_iter()
            .filter_map(|path| Some((rank(&path)?, path)))
            .collect();
        candidates.sort();
        for (_, path) in candidates {
            let Some(bytes) = source.read(&directory.join(path)) else {
                continue;
            };
            let Some(image) = decode(&bytes) else {
                continue;
            };
            let mut bgra = image::DynamicImage::ImageRgba8(image)
                .thumbnail(64, 64)
                .into_rgba8();
            for pixel in bgra.pixels_mut() {
                pixel.0.swap(0, 2);
            }
            return Some(gpui::RenderImage::new([image::Frame::new(bgra)]));
        }
    }
    None
}

fn decode(bytes: &[u8]) -> Option<image::RgbaImage> {
    if bytes.starts_with(b"icns") {
        return icns::decode(bytes);
    }
    crate::views::project_editor::logo::decode(bytes).ok()
}
