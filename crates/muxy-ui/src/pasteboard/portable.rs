use super::Content;

pub fn read_content(cx: &gpui::App) -> Result<Content, String> {
    let Some(item) = cx.read_from_clipboard() else {
        return Ok(Content::Text);
    };
    for entry in item.entries() {
        if let gpui::ClipboardEntry::ExternalPaths(files) = entry
            && !files.paths().is_empty()
        {
            return Ok(Content::Files(files.paths().to_vec()));
        }
    }
    for entry in item.entries() {
        if let gpui::ClipboardEntry::Image(image) = entry {
            if image.bytes().len() > 25 * 1024 * 1024 {
                return Err(crate::tr!("Copied image is larger than 25 MiB").to_string());
            }
            return Ok(Content::Image(image.bytes().to_vec()));
        }
    }
    Ok(Content::Text)
}

pub const IMAGE_SUBMISSION: muxy_core::capability::Capability =
    muxy_core::capability::Capability::Unsupported;

#[derive(Debug)]
pub enum Lease {}

impl Lease {
    pub fn capture() -> Result<Self, String> {
        Err(muxy_core::capability::Feature::ClipboardImages
            .unsupported_reason()
            .into_owned())
    }

    pub fn write_png(&mut self, _png: Vec<u8>) -> Result<(), String> {
        match *self {}
    }
}
