#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(target_os = "macos"))]
use unsupported as platform;

const STRING_TYPE: &str = "public.utf8-plain-text";
const FILE_URL_TYPE: &str = "public.file-url";
const IMAGE_TYPES: [&str; 5] = [
    "public.png",
    "public.tiff",
    "public.jpeg",
    "com.compuserve.gif",
    "org.webmproject.webp",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasteboardRepresentation {
    pub type_identifier: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PasteboardItem {
    pub representations: Vec<PasteboardRepresentation>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PasteboardSnapshot {
    pub items: Vec<PasteboardItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PasteboardContent {
    Empty,
    Text(String),
    Files(Vec<String>),
    Image(Vec<u8>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PasteboardError {
    #[error("native pasteboard is unavailable")]
    Unavailable,
    #[error("native pasteboard capture failed")]
    CaptureFailed,
    #[error("native pasteboard write failed")]
    WriteFailed,
}

pub fn read_content() -> Result<PasteboardContent, PasteboardError> {
    platform::read_content()
}

pub fn capture() -> Result<PasteboardSnapshot, PasteboardError> {
    platform::capture()
}

pub struct PasteboardReplacement {
    snapshot: Option<PasteboardSnapshot>,
}

impl PasteboardReplacement {
    pub fn restore(mut self) -> Result<(), PasteboardError> {
        let result = platform::restore(self.snapshot.as_ref().unwrap());
        if result.is_ok() {
            self.snapshot = None;
        }
        result
    }
}

impl Drop for PasteboardReplacement {
    fn drop(&mut self) {
        if let Some(snapshot) = self.snapshot.take() {
            let _ = platform::restore(&snapshot);
        }
    }
}

pub fn replace_with_png(contents: &[u8]) -> Result<PasteboardReplacement, PasteboardError> {
    platform::replace_with_png(contents).map(|snapshot| PasteboardReplacement {
        snapshot: Some(snapshot),
    })
}

fn classify(snapshot: &PasteboardSnapshot) -> PasteboardContent {
    for item in &snapshot.items {
        if let Some(representation) = item
            .representations
            .iter()
            .find(|representation| representation.type_identifier == STRING_TYPE)
            && let Ok(text) = String::from_utf8(representation.data.clone())
            && !text.is_empty()
        {
            return PasteboardContent::Text(text);
        }
    }
    let file_urls = snapshot
        .items
        .iter()
        .flat_map(|item| &item.representations)
        .filter(|representation| representation.type_identifier == FILE_URL_TYPE)
        .filter_map(|representation| String::from_utf8(representation.data.clone()).ok())
        .collect::<Vec<_>>();
    let paths = muxy_core::dropped_paths::parse_with(&file_urls, None, |_| true);
    if !paths.is_empty() {
        return PasteboardContent::Files(paths);
    }
    for item in &snapshot.items {
        if let Some(representation) = item
            .representations
            .iter()
            .find(|representation| IMAGE_TYPES.contains(&representation.type_identifier.as_str()))
        {
            return PasteboardContent::Image(representation.data.clone());
        }
    }
    PasteboardContent::Empty
}

#[cfg(test)]
mod tests {
    use super::*;

    fn representation(type_identifier: &str, data: &[u8]) -> PasteboardRepresentation {
        PasteboardRepresentation {
            type_identifier: type_identifier.to_owned(),
            data: data.to_vec(),
        }
    }

    #[test]
    fn native_content_precedence_is_text_then_decoded_files_then_images() {
        let image = PasteboardItem {
            representations: vec![representation("public.png", b"png")],
        };
        let file = PasteboardItem {
            representations: vec![representation(
                FILE_URL_TYPE,
                b"file:///tmp/file%20name.txt",
            )],
        };
        let text = PasteboardItem {
            representations: vec![representation(STRING_TYPE, b"text")],
        };
        assert_eq!(
            classify(&PasteboardSnapshot {
                items: vec![image.clone(), file.clone(), text],
            }),
            PasteboardContent::Text("text".to_owned())
        );
        assert_eq!(
            classify(&PasteboardSnapshot {
                items: vec![image.clone(), file],
            }),
            PasteboardContent::Files(vec!["/tmp/file name.txt".to_owned()])
        );
        assert_eq!(
            classify(&PasteboardSnapshot { items: vec![image] }),
            PasteboardContent::Image(b"png".to_vec())
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn unsupported_pasteboard_is_neutral() {
        assert_eq!(read_content(), Ok(PasteboardContent::Empty));
        assert!(matches!(
            replace_with_png(b"png"),
            Err(PasteboardError::Unavailable)
        ));
    }
}
