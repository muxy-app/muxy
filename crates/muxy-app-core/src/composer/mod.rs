mod draft;
pub mod image_storage;
mod storage;
pub mod submission;

pub use draft::{
    ComposerDraft, ComposerLoadStatus, ComposerStore, DraftId, ImageAttachment, SAVE_DEBOUNCE,
    placeholder_numbers,
};

const MAX_DRAFT_BYTES: usize = 32 * 1024 * 1024;

pub const DRAFTS_FILE_NAME: &str = "composer-drafts.json";
pub const IMAGES_DIRECTORY_NAME: &str = "ComposerImages";

fn canonical_uuid(value: &str) -> Option<String> {
    uuid::Uuid::parse_str(value)
        .ok()
        .map(|value| value.hyphenated().to_string().to_uppercase())
}
