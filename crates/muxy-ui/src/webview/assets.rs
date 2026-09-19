use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

pub const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Source {
    pub owner: String,
    pub directory: PathBuf,
    pub entry: String,
}

impl Source {
    pub fn entry_url(&self) -> io::Result<String> {
        if self.owner.is_empty()
            || !self
                .owner
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
        {
            return Err(io::Error::other("invalid webview owner"));
        }
        relative_path(&self.entry)?;
        let escaped: String = self
            .entry
            .trim_start_matches('/')
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
                    char::from(byte).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect();
        Ok(format!("muxy-ext://{}/{escaped}", self.owner))
    }
}

fn relative_path(relative: &str) -> io::Result<&Path> {
    let relative = Path::new(relative.trim_start_matches('/'));
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(io::Error::other("asset path escapes its content root"));
    }
    Ok(relative)
}

pub fn resolve(root: &Path, relative: &str) -> io::Result<PathBuf> {
    let relative = relative_path(relative)?;
    let root = root.canonicalize()?;
    let path = root.join(relative).canonicalize()?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(io::Error::other(
            "asset is outside its content root or not a file",
        ));
    }
    Ok(path)
}

pub(super) fn read(root: &Path, relative: &str) -> io::Result<(Vec<u8>, &'static str)> {
    let path = resolve(root, relative)?;
    let file = File::open(&path)?;
    if file.metadata()?.len() > MAX_ASSET_BYTES {
        return Err(io::Error::other("asset exceeds 64 MiB"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_ASSET_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_ASSET_BYTES {
        return Err(io::Error::other("asset exceeds 64 MiB"));
    }
    Ok((bytes, mime(&path)))
}

fn mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|part| part.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn containment_and_symlinks() -> io::Result<()> {
        let root = std::env::temp_dir().join(format!("muxy-web-assets-{}", std::process::id()));
        std::fs::create_dir_all(root.join("inside"))?;
        std::fs::write(root.join("inside/index.html"), "hello")?;
        std::os::unix::fs::symlink("/etc/passwd", root.join("outside"))?;
        assert!(resolve(&root, "../etc/passwd").is_err());
        assert!(resolve(&root, "outside").is_err());
        assert!(resolve(&root, "inside").is_err());
        assert_eq!(read(&root, "inside/index.html")?.0, b"hello");
        assert!(
            Source {
                owner: "bad/owner".into(),
                directory: root.clone(),
                entry: "inside/index.html".into()
            }
            .entry_url()
            .is_err()
        );
        std::fs::remove_dir_all(root)
    }
}
