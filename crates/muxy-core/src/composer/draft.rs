use super::image_storage::{
    ImageStorage, PreparedImageSource, prepare_image_source, validate_image_filename,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DraftId(String);

impl DraftId {
    pub fn new(project_id: &str, worktree_id: &str) -> Option<Self> {
        let project = crate::notifications::canonical_uuid(project_id)?;
        let worktree = crate::notifications::canonical_uuid(worktree_id)?;
        Some(Self(format!("{project}:{worktree}")))
    }

    pub fn parse(value: &str) -> Option<Self> {
        let (project, worktree) = value.split_once(':')?;
        let id = Self::new(project, worktree)?;
        (id.0 == value).then_some(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImageAttachment {
    pub number: u64,
    pub filename: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposerDraft {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub file_attachments: Vec<String>,
    #[serde(default)]
    pub image_attachments: Vec<ImageAttachment>,
    #[serde(default = "default_next_image_number")]
    pub next_image_number: u64,
}

impl Default for ComposerDraft {
    fn default() -> Self {
        Self {
            text: String::new(),
            file_attachments: Vec::new(),
            image_attachments: Vec::new(),
            next_image_number: default_next_image_number(),
        }
    }
}

impl ComposerDraft {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.file_attachments.is_empty()
            && self.image_attachments.is_empty()
    }

    pub fn validate(&self) -> std::io::Result<()> {
        if self
            .file_attachments
            .iter()
            .any(|path| !Path::new(path).is_absolute())
        {
            return Err(invalid_draft("file attachment is not an absolute path"));
        }
        let placeholders = placeholder_numbers(&self.text);
        let mut numbers = BTreeSet::new();
        let mut largest = 0;
        for attachment in &self.image_attachments {
            if attachment.number == 0
                || !numbers.insert(attachment.number)
                || !placeholders.contains(&attachment.number)
            {
                return Err(invalid_draft("image attachment number is invalid"));
            }
            validate_image_filename(&attachment.filename)?;
            largest = largest.max(attachment.number);
        }
        if self.next_image_number == 0 || self.next_image_number <= largest {
            return Err(invalid_draft("next image number is invalid"));
        }
        Ok(())
    }
}

fn default_next_image_number() -> u64 {
    1
}

fn invalid_draft(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComposerLoadStatus {
    pub overwrite_blocked: bool,
    pub malformed_keys: Vec<String>,
    pub warnings: Vec<String>,
}

impl ComposerLoadStatus {
    pub fn is_ready(&self) -> bool {
        !self.overwrite_blocked && self.malformed_keys.is_empty() && self.warnings.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingImageRemoval {
    attachment: ImageAttachment,
    removal_revision: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DraftState {
    draft: ComposerDraft,
    pending_removals: BTreeMap<u64, PendingImageRemoval>,
}

#[derive(Debug)]
pub struct ComposerStore {
    path: PathBuf,
    drafts: BTreeMap<DraftId, DraftState>,
    malformed: BTreeMap<String, Value>,
    revisions: BTreeMap<DraftId, u64>,
    dirty_revision: u64,
    flushed_revision: u64,
    load_status: ComposerLoadStatus,
    image_storage: Option<ImageStorage>,
}

impl ComposerStore {
    pub fn load() -> Self {
        Self::load_from(crate::prefs::app_support_dir())
    }

    pub fn load_from(profile_root: impl Into<PathBuf>) -> Self {
        let profile_root = profile_root.into();
        let path = profile_root.join(super::DRAFTS_FILE_NAME);
        let mut status = ComposerLoadStatus::default();
        let image_storage = match ImageStorage::open(&profile_root) {
            Ok(storage) => Some(storage),
            Err(error) => {
                status
                    .warnings
                    .push(format!("failed to open Composer image storage: {error}"));
                None
            }
        };
        let (drafts, malformed) = load_document(&path, &mut status);
        let mut store = Self {
            path,
            drafts,
            malformed,
            revisions: BTreeMap::new(),
            dirty_revision: 0,
            flushed_revision: 0,
            load_status: status,
            image_storage,
        };
        for id in store.drafts.keys() {
            store.revisions.insert(id.clone(), 0);
        }
        store.sweep_startup();
        store
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_status(&self) -> &ComposerLoadStatus {
        &self.load_status
    }

    pub fn image_storage(&self) -> Option<&ImageStorage> {
        self.image_storage.as_ref()
    }

    pub fn draft(&self, id: &DraftId) -> Option<&ComposerDraft> {
        self.drafts
            .get(id)
            .map(|state| &state.draft)
            .filter(|draft| !draft.is_empty())
    }

    pub fn drafts(&self) -> impl Iterator<Item = (&DraftId, &ComposerDraft)> {
        self.drafts
            .iter()
            .filter(|(_, state)| !state.draft.is_empty())
            .map(|(id, state)| (id, &state.draft))
    }

    pub fn draft_revision(&self, id: &DraftId) -> u64 {
        self.revisions.get(id).copied().unwrap_or(0)
    }

    pub fn dirty_revision(&self) -> u64 {
        self.dirty_revision
    }

    pub fn flushed_revision(&self) -> u64 {
        self.flushed_revision
    }

    pub fn needs_flush(&self) -> bool {
        self.dirty_revision != self.flushed_revision
    }

    pub fn replace_draft(&mut self, id: DraftId, draft: ComposerDraft) -> std::io::Result<u64> {
        draft.validate()?;
        let raw_removed = self.malformed.remove(id.as_str()).is_some();
        let current = self.drafts.get(&id).cloned().unwrap_or_default();
        if current.draft == draft && current.pending_removals.is_empty() && !raw_removed {
            return Ok(self.draft_revision(&id));
        }
        let revision = self.next_draft_revision(&id);
        self.drafts.insert(
            id.clone(),
            DraftState {
                draft,
                pending_removals: BTreeMap::new(),
            },
        );
        self.revisions.insert(id, revision);
        self.touch();
        Ok(revision)
    }

    pub fn edit_content(
        &mut self,
        id: DraftId,
        text: String,
        file_attachments: Vec<String>,
    ) -> std::io::Result<u64> {
        if file_attachments
            .iter()
            .any(|path| !Path::new(path).is_absolute())
        {
            return Err(invalid_draft("file attachment is not an absolute path"));
        }
        let raw_removed = self.malformed.remove(id.as_str()).is_some();
        let mut state = self.drafts.get(&id).cloned().unwrap_or_default();
        let previous = state.clone();
        let revision = self.next_draft_revision(&id);
        state.draft.text = text;
        state.draft.file_attachments = file_attachments;
        reconcile_images(&mut state, revision);
        if state == previous && !raw_removed {
            return Ok(self.draft_revision(&id));
        }
        state.draft.validate()?;
        self.drafts.insert(id.clone(), state);
        self.revisions.insert(id, revision);
        self.touch();
        Ok(revision)
    }

    pub fn clear_if_revision(&mut self, id: &DraftId, revision: u64) -> std::io::Result<bool> {
        if self.draft_revision(id) != revision {
            return Ok(false);
        }
        self.edit_content(id.clone(), String::new(), Vec::new())?;
        Ok(true)
    }

    pub fn attach_image(
        &mut self,
        id: DraftId,
        contents: &[u8],
        insertion_offset: usize,
    ) -> std::io::Result<(u64, u64)> {
        let source = prepare_image_source(contents.to_vec())?;
        self.attach_prepared_image(id, &source, insertion_offset..insertion_offset)
    }

    pub fn attach_prepared_image(
        &mut self,
        id: DraftId,
        source: &PreparedImageSource,
        selection: Range<usize>,
    ) -> std::io::Result<(u64, u64)> {
        let filename = self
            .image_storage
            .as_ref()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Composer image storage is unavailable",
                )
            })?
            .write_prepared(source)?;
        let previous_state = self.drafts.get(&id).cloned();
        let previous_raw = self.malformed.get(id.as_str()).cloned();
        let previous_draft_revision = self.revisions.get(&id).copied();
        let previous_dirty_revision = self.dirty_revision;
        let previous_overwrite_blocked = self.load_status.overwrite_blocked;
        let mut state = previous_state.clone().unwrap_or_default();
        if selection.start > selection.end
            || selection.end > state.draft.text.len()
            || !state.draft.text.is_char_boundary(selection.start)
            || !state.draft.text.is_char_boundary(selection.end)
            || state.draft.next_image_number == u64::MAX
        {
            let _ = self
                .image_storage
                .as_ref()
                .and_then(|storage| storage.remove(&filename).ok());
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Composer image insertion offset or number is invalid",
            ));
        }
        let number = state.draft.next_image_number;
        let placeholder = format!("[Image {number}]");
        state.draft.text.replace_range(selection, &placeholder);
        state.draft.image_attachments.push(ImageAttachment {
            number,
            filename: filename.clone(),
        });
        state
            .draft
            .image_attachments
            .sort_by_key(|attachment| attachment.number);
        state.draft.next_image_number = number + 1;
        if let Err(error) = state.draft.validate() {
            if let Some(storage) = &self.image_storage {
                let _ = storage.remove(&filename);
            }
            return Err(error);
        }
        let draft_revision = self.next_draft_revision(&id);
        self.malformed.remove(id.as_str());
        self.drafts.insert(id.clone(), state);
        self.revisions.insert(id.clone(), draft_revision);
        self.touch();
        let publication = self.flush().and_then(|published| {
            if published {
                Ok(())
            } else {
                Err(std::io::Error::other(
                    "Composer image draft publication did not run",
                ))
            }
        });
        if let Err(error) = publication {
            if !previous_overwrite_blocked && self.load_status.overwrite_blocked {
                return Err(error);
            }
            match previous_state {
                Some(state) => {
                    self.drafts.insert(id.clone(), state);
                }
                None => {
                    self.drafts.remove(&id);
                }
            }
            match previous_raw {
                Some(raw) => {
                    self.malformed.insert(id.as_str().to_owned(), raw);
                }
                None => {
                    self.malformed.remove(id.as_str());
                }
            }
            match previous_draft_revision {
                Some(revision) => {
                    self.revisions.insert(id, revision);
                }
                None => {
                    self.revisions.remove(&id);
                }
            }
            self.dirty_revision = previous_dirty_revision;
            if let Some(storage) = &self.image_storage {
                let _ = storage.remove(&filename);
            }
            return Err(error);
        }
        Ok((number, draft_revision))
    }

    pub fn flush(&mut self) -> std::io::Result<bool> {
        self.flush_if_revision(self.dirty_revision)
    }

    pub fn flush_if_revision(&mut self, revision: u64) -> std::io::Result<bool> {
        if revision != self.dirty_revision || !self.needs_flush() {
            return Ok(false);
        }
        if self.load_status.overwrite_blocked {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Composer draft overwrite is blocked by a whole-file parse failure",
            ));
        }
        let contents = self.serialize()?;
        if let Err(error) = crate::store::write_private_durable(&self.path, &contents) {
            if error.publication_may_have_succeeded() {
                self.load_status.overwrite_blocked = true;
                self.load_status.warnings.push(
                    "Composer draft publication could not be proven durable; copied images were preserved"
                        .to_owned(),
                );
            }
            return Err(error.into());
        }
        self.flushed_revision = revision;
        for state in self.drafts.values_mut() {
            let current_revision = state
                .pending_removals
                .values()
                .map(|pending| pending.removal_revision)
                .max()
                .unwrap_or(0);
            if current_revision <= self.dirty_revision {
                state.pending_removals.clear();
            }
        }
        if let Err(error) = self.sweep_after_publication() {
            self.load_status.warnings.push(format!(
                "failed to remove unreferenced Composer images: {error}"
            ));
        }
        Ok(true)
    }

    fn serialize(&self) -> std::io::Result<Vec<u8>> {
        let mut root = serde_json::Map::new();
        let mut keys: BTreeSet<String> = self.malformed.keys().cloned().collect();
        keys.extend(
            self.drafts
                .iter()
                .filter(|(_, state)| !state.draft.is_empty())
                .map(|(id, _)| id.as_str().to_owned()),
        );
        for key in keys {
            if let Some(state) = DraftId::parse(&key)
                .and_then(|id| self.drafts.get(&id))
                .filter(|state| !state.draft.is_empty())
            {
                root.insert(
                    key,
                    serde_json::to_value(&state.draft).map_err(std::io::Error::other)?,
                );
            } else if let Some(raw) = self.malformed.get(&key) {
                root.insert(key, raw.clone());
            }
        }
        serde_json::to_vec_pretty(&Value::Object(root)).map_err(std::io::Error::other)
    }

    fn next_draft_revision(&self, id: &DraftId) -> u64 {
        self.draft_revision(id).saturating_add(1)
    }

    fn touch(&mut self) {
        self.dirty_revision = self.dirty_revision.saturating_add(1);
    }

    fn referenced_filenames(&self) -> HashSet<String> {
        self.drafts
            .values()
            .flat_map(|state| state.draft.image_attachments.iter())
            .map(|attachment| attachment.filename.clone())
            .collect()
    }

    fn sweep_startup(&mut self) {
        if self.load_status.overwrite_blocked || !self.malformed.is_empty() {
            return;
        }
        let referenced = self.referenced_filenames();
        let Some(storage) = &self.image_storage else {
            return;
        };
        if let Err(error) = storage.sweep(&referenced) {
            self.load_status
                .warnings
                .push(format!("failed to sweep Composer images: {error}"));
        }
    }

    fn sweep_after_publication(&self) -> std::io::Result<()> {
        if !self.malformed.is_empty() {
            return Ok(());
        }
        let Some(storage) = &self.image_storage else {
            return Ok(());
        };
        storage.sweep(&self.referenced_filenames()).map(|_| ())
    }
}

fn load_document(
    path: &Path,
    status: &mut ComposerLoadStatus,
) -> (BTreeMap<DraftId, DraftState>, BTreeMap<String, Value>) {
    let contents = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (BTreeMap::new(), BTreeMap::new());
        }
        Err(error) => {
            status.overwrite_blocked = true;
            status
                .warnings
                .push(format!("failed to read Composer drafts: {error}"));
            return (BTreeMap::new(), BTreeMap::new());
        }
    };
    let root = match serde_json::from_slice::<Value>(&contents) {
        Ok(Value::Object(root)) => root,
        Ok(_) => {
            status.overwrite_blocked = true;
            status
                .warnings
                .push("Composer drafts must be a top-level object".to_owned());
            return (BTreeMap::new(), BTreeMap::new());
        }
        Err(error) => {
            status.overwrite_blocked = true;
            status
                .warnings
                .push(format!("failed to parse Composer drafts: {error}"));
            return (BTreeMap::new(), BTreeMap::new());
        }
    };
    let mut drafts = BTreeMap::new();
    let mut malformed = BTreeMap::new();
    for (key, raw) in root {
        let parsed = DraftId::parse(&key)
            .and_then(|id| decode_draft(raw.clone()).ok().map(|draft| (id, draft)));
        if let Some((id, draft)) = parsed {
            if !draft.is_empty() {
                drafts.insert(
                    id,
                    DraftState {
                        draft,
                        pending_removals: BTreeMap::new(),
                    },
                );
            }
        } else {
            status.malformed_keys.push(key.clone());
            malformed.insert(key, raw);
        }
    }
    status.malformed_keys.sort();
    (drafts, malformed)
}

fn decode_draft(value: Value) -> std::io::Result<ComposerDraft> {
    let next_number_missing = value.get("nextImageNumber").is_none();
    let mut draft: ComposerDraft = serde_json::from_value(value).map_err(std::io::Error::other)?;
    if next_number_missing {
        draft.next_image_number = draft
            .image_attachments
            .iter()
            .map(|attachment| attachment.number)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
    }
    draft.validate()?;
    Ok(draft)
}

fn reconcile_images(state: &mut DraftState, revision: u64) {
    let placeholders = placeholder_numbers(&state.draft.text);
    let mut retained = Vec::new();
    for attachment in state.draft.image_attachments.drain(..) {
        if placeholders.contains(&attachment.number) {
            retained.push(attachment);
        } else {
            state.pending_removals.insert(
                attachment.number,
                PendingImageRemoval {
                    attachment,
                    removal_revision: revision,
                },
            );
        }
    }
    let restored: Vec<u64> = state
        .pending_removals
        .keys()
        .copied()
        .filter(|number| placeholders.contains(number))
        .collect();
    for number in restored {
        if let Some(pending) = state.pending_removals.remove(&number) {
            retained.push(pending.attachment);
        }
    }
    retained.sort_by_key(|attachment| attachment.number);
    state.draft.image_attachments = retained;
}

pub fn placeholder_numbers(text: &str) -> BTreeSet<u64> {
    let bytes = text.as_bytes();
    let mut numbers = BTreeSet::new();
    let mut index = 0;
    while index + 8 <= bytes.len() {
        if !bytes[index..].starts_with(b"[Image ") {
            index += 1;
            continue;
        }
        let start = index + 7;
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end > start
            && bytes.get(end) == Some(&b']')
            && let Ok(raw) = std::str::from_utf8(&bytes[start..end])
            && let Ok(number) = raw.parse::<u64>()
        {
            numbers.insert(number);
            index = end + 1;
        } else {
            index += 1;
        }
    }
    numbers
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    const PROJECT: &str = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE";
    const WORKTREE: &str = "11111111-2222-4333-8444-555555555555";
    const OTHER_WORKTREE: &str = "99999999-AAAA-4BBB-8CCC-DDDDDDDDDDDD";
    const IMAGE: &str = "22222222-3333-4444-8555-666666666666.png";

    fn id() -> DraftId {
        DraftId::new(PROJECT, WORKTREE).unwrap()
    }

    fn image_draft(text: &str) -> ComposerDraft {
        image_draft_with(text, IMAGE)
    }

    fn image_draft_with(text: &str, filename: &str) -> ComposerDraft {
        ComposerDraft {
            text: text.to_owned(),
            file_attachments: Vec::new(),
            image_attachments: vec![ImageAttachment {
                number: 1,
                filename: filename.to_owned(),
            }],
            next_image_number: 2,
        }
    }

    fn png() -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 2, Rgba([1, 2, 3, 4])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn composer_store_save_debounce_is_exactly_four_hundred_milliseconds() {
        assert_eq!(SAVE_DEBOUNCE, Duration::from_millis(400));
    }

    #[test]
    fn composer_draft_ids_are_exact_uppercase_project_and_worktree_uuids() {
        let id = id();
        assert_eq!(id.as_str(), format!("{PROJECT}:{WORKTREE}"));
        assert_eq!(DraftId::parse(id.as_str()), Some(id));
        assert!(DraftId::parse(&format!("{}:{WORKTREE}", PROJECT.to_lowercase())).is_none());
        assert!(DraftId::parse("bad").is_none());
    }

    #[test]
    fn composer_placeholder_scanning_accepts_exact_numbers_only() {
        assert_eq!(
            placeholder_numbers("x [Image 1] [Image 42] [Image x] [Image 1]"),
            BTreeSet::from([1, 42])
        );
    }

    #[test]
    fn composer_store_defaults_missing_fields_and_ignores_unknown_fields() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join(super::super::DRAFTS_FILE_NAME);
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                id().as_str(): {"text": "hello", "future": true}
            }))
            .unwrap(),
        )
        .unwrap();
        let store = ComposerStore::load_from(profile.path());
        assert!(store.load_status().is_ready());
        assert_eq!(store.draft(&id()).unwrap().text, "hello");
        assert_eq!(store.draft(&id()).unwrap().next_image_number, 1);
    }

    #[test]
    fn composer_store_carries_malformed_entries_and_sorts_output() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join(super::super::DRAFTS_FILE_NAME);
        let valid_id = id();
        let malformed_id =
            "BBBBBBBB-CCCC-4DDD-8EEE-FFFFFFFFFFFF:33333333-4444-4555-8666-777777777777";
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                malformed_id: {"text": 7},
                valid_id.as_str(): {"text": "before"}
            }))
            .unwrap(),
        )
        .unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        assert_eq!(store.load_status().malformed_keys, vec![malformed_id]);
        store
            .edit_content(valid_id, "after".to_owned(), Vec::new())
            .unwrap();
        store.flush().unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.find(PROJECT).unwrap() < text.find(malformed_id).unwrap());
        let root: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(root[malformed_id], serde_json::json!({"text": 7}));
    }

    #[test]
    fn composer_store_whole_file_failure_blocks_overwrite_and_preserves_bytes() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join(super::super::DRAFTS_FILE_NAME);
        std::fs::write(&path, b"not json").unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        assert!(store.load_status().overwrite_blocked);
        store
            .edit_content(id(), "new".to_owned(), Vec::new())
            .unwrap();
        assert!(store.flush().is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"not json");
    }

    #[test]
    fn composer_store_removes_empty_drafts_and_conditionally_clears_revisions() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        let first = store
            .edit_content(id(), "hello".to_owned(), Vec::new())
            .unwrap();
        assert_eq!(first, 1);
        assert!(!store.clear_if_revision(&id(), 0).unwrap());
        assert!(store.clear_if_revision(&id(), first).unwrap());
        assert!(store.draft(&id()).is_none());
        store.flush().unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(store.path()).unwrap()).unwrap(),
            serde_json::json!({})
        );
    }

    #[test]
    fn composer_store_flush_is_revision_conditional_and_tracks_dirty_state() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        store
            .edit_content(id(), "one".to_owned(), Vec::new())
            .unwrap();
        let stale = store.dirty_revision();
        store
            .edit_content(id(), "two".to_owned(), Vec::new())
            .unwrap();
        assert!(!store.flush_if_revision(stale).unwrap());
        assert!(store.needs_flush());
        assert!(store.flush().unwrap());
        assert_eq!(store.dirty_revision(), store.flushed_revision());
    }

    #[test]
    fn composer_store_pending_image_removal_can_be_undone_before_publication() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        store.replace_draft(id(), image_draft("[Image 1]")).unwrap();
        store
            .edit_content(id(), "removed".to_owned(), Vec::new())
            .unwrap();
        assert!(store.draft(&id()).unwrap().image_attachments.is_empty());
        store
            .edit_content(id(), "[Image 1]".to_owned(), Vec::new())
            .unwrap();
        assert_eq!(store.draft(&id()).unwrap().image_attachments.len(), 1);
    }

    #[test]
    fn composer_store_duplicate_placeholder_keeps_image_until_the_last_token_is_removed() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        store
            .replace_draft(id(), image_draft("[Image 1] [Image 1]"))
            .unwrap();
        store
            .edit_content(id(), "[Image 1]".to_owned(), Vec::new())
            .unwrap();
        assert_eq!(store.draft(&id()).unwrap().image_attachments.len(), 1);
        store
            .edit_content(id(), "gone".to_owned(), Vec::new())
            .unwrap();
        assert!(store.draft(&id()).unwrap().image_attachments.is_empty());
    }

    #[test]
    fn composer_store_relaunches_per_worktree_drafts() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        store
            .edit_content(id(), "restored".to_owned(), vec!["/tmp/file".to_owned()])
            .unwrap();
        store.flush().unwrap();
        let restored = ComposerStore::load_from(profile.path());
        assert_eq!(restored.draft(&id()).unwrap().text, "restored");
        assert_eq!(
            restored.draft(&id()).unwrap().file_attachments,
            ["/tmp/file"]
        );
    }

    #[test]
    fn composer_store_image_attachment_is_durable_and_inserted_at_a_utf8_boundary() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        store
            .edit_content(id(), "aéz".to_owned(), Vec::new())
            .unwrap();
        store.flush().unwrap();
        let (number, revision) = store.attach_image(id(), &png(), 3).unwrap();
        assert_eq!(number, 1);
        assert_eq!(revision, 2);
        assert_eq!(store.draft(&id()).unwrap().text, "aé[Image 1]z");
        assert!(!store.needs_flush());
        let restored = ComposerStore::load_from(profile.path());
        assert_eq!(restored.draft(&id()).unwrap().text, "aé[Image 1]z");
        assert_eq!(restored.draft(&id()).unwrap().image_attachments.len(), 1);
    }

    #[test]
    fn composer_store_image_attachment_replaces_the_selected_utf8_range() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        store
            .edit_content(id(), "before chosen after".to_owned(), Vec::new())
            .unwrap();
        store.flush().unwrap();
        let source = prepare_image_source(png()).unwrap();
        let (number, _) = store.attach_prepared_image(id(), &source, 7..13).unwrap();
        assert_eq!(number, 1);
        assert_eq!(store.draft(&id()).unwrap().text, "before [Image 1] after");
    }

    #[test]
    fn composer_store_failed_image_draft_publication_rolls_back_new_file_and_text() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join(super::super::DRAFTS_FILE_NAME);
        std::fs::write(&path, b"not json").unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        assert!(store.attach_image(id(), &png(), 0).is_err());
        assert!(store.draft(&id()).is_none());
        assert!(
            store
                .image_storage()
                .unwrap()
                .regular_file_names()
                .unwrap()
                .is_empty()
        );
        assert_eq!(std::fs::read(path).unwrap(), b"not json");
    }

    #[test]
    fn composer_store_deletes_images_only_after_durable_final_reference_removal() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        let filename = store.image_storage().unwrap().write_source(&png()).unwrap();
        store
            .replace_draft(id(), image_draft_with("[Image 1]", &filename))
            .unwrap();
        store.flush().unwrap();
        store
            .edit_content(id(), "removed".to_owned(), Vec::new())
            .unwrap();
        assert!(store.image_storage().unwrap().read(&filename).is_ok());
        store.flush().unwrap();
        assert!(store.image_storage().unwrap().read(&filename).is_err());
        store
            .edit_content(id(), "[Image 1]".to_owned(), Vec::new())
            .unwrap();
        assert!(store.draft(&id()).unwrap().image_attachments.is_empty());
    }

    #[test]
    fn composer_store_stale_publication_cannot_delete_a_restored_image() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        let filename = store.image_storage().unwrap().write_source(&png()).unwrap();
        store
            .replace_draft(id(), image_draft_with("[Image 1]", &filename))
            .unwrap();
        store.flush().unwrap();
        store
            .edit_content(id(), "removed".to_owned(), Vec::new())
            .unwrap();
        let stale = store.dirty_revision();
        store
            .edit_content(id(), "[Image 1]".to_owned(), Vec::new())
            .unwrap();
        assert!(!store.flush_if_revision(stale).unwrap());
        assert!(store.image_storage().unwrap().read(&filename).is_ok());
        store.flush().unwrap();
        assert!(store.image_storage().unwrap().read(&filename).is_ok());
    }

    #[test]
    fn composer_store_shared_filename_survives_until_every_draft_removes_it() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        let filename = store.image_storage().unwrap().write_source(&png()).unwrap();
        let first = id();
        let second = DraftId::new(PROJECT, OTHER_WORKTREE).unwrap();
        store
            .replace_draft(first.clone(), image_draft_with("[Image 1]", &filename))
            .unwrap();
        store
            .replace_draft(second.clone(), image_draft_with("[Image 1]", &filename))
            .unwrap();
        store.flush().unwrap();
        store
            .edit_content(first, "removed".to_owned(), Vec::new())
            .unwrap();
        store.flush().unwrap();
        assert!(store.image_storage().unwrap().read(&filename).is_ok());
        store
            .edit_content(second, "removed".to_owned(), Vec::new())
            .unwrap();
        store.flush().unwrap();
        assert!(store.image_storage().unwrap().read(&filename).is_err());
    }

    #[test]
    fn composer_store_failed_publication_retains_pending_image_files() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        let filename = store.image_storage().unwrap().write_source(&png()).unwrap();
        store
            .replace_draft(id(), image_draft_with("[Image 1]", &filename))
            .unwrap();
        store.flush().unwrap();
        store
            .edit_content(id(), "removed".to_owned(), Vec::new())
            .unwrap();
        std::fs::remove_file(store.path()).unwrap();
        std::fs::create_dir(store.path()).unwrap();
        assert!(store.flush().is_err());
        assert!(store.image_storage().unwrap().read(&filename).is_ok());
    }

    #[test]
    fn composer_store_startup_sweeps_only_proven_regular_orphans() {
        let profile = tempfile::tempdir().unwrap();
        let mut store = ComposerStore::load_from(profile.path());
        let retained = store.image_storage().unwrap().write_source(&png()).unwrap();
        store
            .replace_draft(id(), image_draft_with("[Image 1]", &retained))
            .unwrap();
        store.flush().unwrap();
        let orphan = store.image_storage().unwrap().write_source(&png()).unwrap();
        assert!(store.image_storage().unwrap().read(&orphan).is_ok());
        drop(store);
        let restored = ComposerStore::load_from(profile.path());
        assert!(restored.image_storage().unwrap().read(&retained).is_ok());
        assert!(restored.image_storage().unwrap().read(&orphan).is_err());
    }

    #[test]
    fn composer_store_malformed_entries_disable_destructive_startup_sweep() {
        let profile = tempfile::tempdir().unwrap();
        let storage = ImageStorage::open(profile.path()).unwrap();
        let orphan = storage.write_source(&png()).unwrap();
        drop(storage);
        std::fs::write(
            profile.path().join(super::super::DRAFTS_FILE_NAME),
            serde_json::to_vec(&serde_json::json!({"bad": {"text": 7}})).unwrap(),
        )
        .unwrap();
        let store = ComposerStore::load_from(profile.path());
        assert_eq!(store.load_status().malformed_keys, ["bad"]);
        assert!(store.image_storage().unwrap().read(&orphan).is_ok());
    }
}
