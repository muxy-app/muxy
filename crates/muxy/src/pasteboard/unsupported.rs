use super::{PasteboardContent, PasteboardError, PasteboardSnapshot};

pub fn read_content() -> Result<PasteboardContent, PasteboardError> {
    Ok(PasteboardContent::Empty)
}

pub fn capture() -> Result<PasteboardSnapshot, PasteboardError> {
    Ok(PasteboardSnapshot::default())
}

pub fn replace_with_png(_: &[u8]) -> Result<PasteboardSnapshot, PasteboardError> {
    Err(PasteboardError::Unavailable)
}

pub fn restore(_: &PasteboardSnapshot) -> Result<(), PasteboardError> {
    Err(PasteboardError::Unavailable)
}
