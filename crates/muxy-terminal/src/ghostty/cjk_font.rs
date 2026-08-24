use std::fs::{self, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(all(unix, test))]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};

use objc2_core_foundation::{CFRange, CFString};
use objc2_core_text::CTFont;

pub const REQUIRED_CJK_GLYPHS: &str = "中文简体繁體专业專業，。！？";

pub const CJK_CODEPOINT_RANGES: [&str; 5] = [
    "U+3000-U+303F",
    "U+3400-U+4DBF",
    "U+4E00-U+9FFF",
    "U+F900-U+FAFF",
    "U+FF00-U+FFEF",
];

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

pub fn font_families(config: &str) -> Vec<String> {
    let config = config.strip_prefix('\u{feff}').unwrap_or(config);
    let mut families = Vec::new();

    for line in config.lines() {
        let Some(value) = config_value("font-family", line) else {
            continue;
        };
        let family = unquoted(value);
        if family.is_empty() {
            families.clear();
        } else {
            families.push(family.to_owned());
        }
    }

    families
}

pub fn resolve_font_family(
    configured: &[String],
    mut supports: impl FnMut(&str, &str) -> bool,
    fallback: impl FnOnce(Option<&str>, &str) -> Option<String>,
) -> Option<String> {
    if let Some(family) = configured
        .iter()
        .find(|family| supports(family, REQUIRED_CJK_GLYPHS))
    {
        return Some(family.clone());
    }

    fallback(configured.first().map(String::as_str), REQUIRED_CJK_GLYPHS)
        .filter(|family| !family.is_empty() && supports(family, REQUIRED_CJK_GLYPHS))
}

pub fn resolve_system_font_family(configured: &[String]) -> Option<String> {
    resolve_font_family(configured, font_supports, |base, glyphs| {
        let base = make_font(base.unwrap_or("Menlo"));
        let sample = CFString::from_str(glyphs);
        let length = isize::try_from(glyphs.encode_utf16().count()).ok()?;

        let fallback = unsafe { base.for_string(&sample, CFRange::new(0, length)) };
        if !font_covers(&fallback, glyphs) {
            return None;
        }

        Some(unsafe { fallback.family_name() }.to_string())
    })
}

fn font_supports(family: &str, glyphs: &str) -> bool {
    font_covers(&make_font(family), glyphs)
}

fn make_font(family: &str) -> objc2_core_foundation::CFRetained<CTFont> {
    let name = CFString::from_str(family);

    unsafe { CTFont::with_name(&name, 13.0, ptr::null()) }
}

fn font_covers(font: &CTFont, glyphs: &str) -> bool {
    let characters = unsafe { font.character_set() };
    glyphs
        .chars()
        .all(|character| characters.is_long_character_member(u32::from(character)))
}

pub fn config_text(family: &str) -> Option<String> {
    if family.is_empty() || family.contains(['\n', '\r']) {
        return None;
    }

    Some(format!(
        "font-codepoint-map = {}={family}\n",
        CJK_CODEPOINT_RANGES.join(",")
    ))
}

pub fn config_text_for_user(
    user_config: &str,
    resolve: impl FnOnce(&[String]) -> Option<String>,
) -> Option<String> {
    let configured = font_families(user_config);
    config_text(&resolve(&configured)?)
}

fn config_value<'a>(key: &str, line: &'a str) -> Option<&'a str> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let remainder = trimmed.strip_prefix(key)?.trim_start_matches([' ', '\t']);
    remainder
        .strip_prefix('=')
        .map(|value| value.trim_matches([' ', '\t']))
}

fn unquoted(value: &str) -> &str {
    if value.len() < 2 {
        return value;
    }

    let bytes = value.as_bytes();
    let matching_quotes = matches!(
        (bytes[0], bytes[bytes.len() - 1]),
        (b'"', b'"') | (b'\'', b'\'')
    );
    if matching_quotes {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

#[derive(Debug)]
pub struct TemporaryConfigFile {
    path: Option<PathBuf>,
}

impl TemporaryConfigFile {
    pub fn create(contents: &str) -> io::Result<Self> {
        Self::create_in(std::env::temp_dir(), contents)
    }

    pub fn create_in(directory: impl AsRef<Path>, contents: &str) -> io::Result<Self> {
        for _ in 0..128 {
            let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
            let path = directory.as_ref().join(format!(
                "muxy-cjk-font-{}-{sequence}.conf",
                std::process::id()
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);

            match options.open(&path) {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(contents.as_bytes()) {
                        let _ = fs::remove_file(&path);
                        return Err(error);
                    }
                    if let Err(error) = file.sync_all() {
                        let _ = fs::remove_file(&path);
                        return Err(error);
                    }
                    return Ok(Self { path: Some(path) });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique CJK config file",
        ))
    }

    pub fn path(&self) -> &Path {
        self.path
            .as_deref()
            .expect("temporary config path is unavailable after cleanup")
    }

    fn remove(&mut self) -> io::Result<()> {
        let Some(path) = self.path.take() else {
            return Ok(());
        };
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Drop for TemporaryConfigFile {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bom_quotes_and_reset_declarations() {
        let config = "\u{feff}font-family = Menlo\n\
                      font-family='PingFang SC'\n\
                      font-family =\n\
                      font-family = \"Noto Sans CJK SC\"\n";

        assert_eq!(font_families(config), ["Noto Sans CJK SC"]);
    }

    #[test]
    fn ignores_similar_keys_comments_and_mismatched_quotes() {
        let config = "# font-family = Hidden\n\
                      font-family-bold = Wrong\n\
                      font-family = \"literal'\n";

        assert_eq!(font_families(config), ["\"literal'"]);
    }

    #[test]
    fn resolves_configured_family_before_fallback() {
        let configured = vec!["Menlo".to_owned(), "PingFang SC".to_owned()];
        let resolved = resolve_font_family(
            &configured,
            |family, _| family == "PingFang SC",
            |_, _| panic!("fallback must not run"),
        );

        assert_eq!(resolved.as_deref(), Some("PingFang SC"));
    }

    #[test]
    fn uses_first_configured_family_as_fallback_base() {
        let configured = vec!["Menlo".to_owned()];
        let resolved = resolve_font_family(
            &configured,
            |family, _| family == "PingFang SC",
            |base, glyphs| {
                assert_eq!(base, Some("Menlo"));
                assert_eq!(glyphs, REQUIRED_CJK_GLYPHS);
                Some("PingFang SC".to_owned())
            },
        );

        assert_eq!(resolved.as_deref(), Some("PingFang SC"));
    }

    #[test]
    fn generated_config_has_all_ranges_and_rejects_line_injection() {
        let text = config_text("PingFang SC").expect("valid family");
        for range in CJK_CODEPOINT_RANGES {
            assert!(text.contains(range));
        }
        assert!(text.ends_with("=PingFang SC\n"));
        assert_eq!(config_text("bad\nfont-family = Other"), None);
    }

    #[test]
    fn temporary_file_is_owner_only_and_cleans_up() {
        let directory = tempfile::tempdir().expect("temp directory");
        let file = TemporaryConfigFile::create_in(directory.path(), "test").expect("temp config");
        let path = file.path().to_owned();
        assert_eq!(fs::read_to_string(&path).expect("contents"), "test");
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path).expect("metadata").permissions().mode() & 0o777,
            0o600
        );

        drop(file);
        assert!(!path.exists());
    }
}
