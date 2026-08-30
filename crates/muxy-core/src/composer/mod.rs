mod draft;
pub mod image_storage;
pub mod submission;

pub use draft::{
    ComposerDraft, ComposerLoadStatus, ComposerStore, DraftId, ImageAttachment, SAVE_DEBOUNCE,
    placeholder_numbers,
};

pub const PANEL_ID: &str = "builtin:richInput";
pub const DRAFTS_FILE_NAME: &str = "rich-input-drafts.json";
pub const IMAGES_DIRECTORY_NAME: &str = "RichInputImages";
