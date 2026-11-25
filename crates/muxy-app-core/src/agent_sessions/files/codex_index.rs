use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::SystemTime;

use muxy_core::files::FileId;

use crate::agent_sessions::{Session, normalize_title};

const INDEX_RECORD_BYTES: usize = 1024 * 1024;

#[derive(Debug, Default)]
pub(super) struct CodexIndex {
    snapshot: Option<Snapshot>,
}

impl CodexIndex {
    pub(super) fn apply(
        &mut self,
        session_root: &Path,
        sessions: &mut [Session],
        budget: u64,
    ) -> io::Result<bool> {
        let previous = self.snapshot.take();
        let path = session_root.with_file_name("session_index.jsonl");
        if sessions.is_empty()
            || !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file())
        {
            return Ok(false);
        }
        let file = fs::File::open(path)?;
        let metadata = file.metadata()?;
        let file_id = FileId::of(&file)?;
        let modified = metadata.modified()?;
        let length = metadata.len();
        let ids: HashSet<_> = sessions.iter().map(|session| session.id.as_str()).collect();
        let mut snapshot = previous
            .filter(|index| index.reusable(file_id, length, modified, &ids))
            .unwrap_or_else(|| Snapshot {
                file: file_id,
                length,
                modified,
                offset: 0,
                partial: Vec::new(),
                skip_record: false,
                names: ids.into_iter().map(|id| (id.to_owned(), None)).collect(),
            });
        snapshot.length = length;
        snapshot.modified = modified;
        snapshot.read(file, budget)?;
        for session in sessions {
            if let Some(name) = snapshot.names.get(&session.id).and_then(Option::as_ref) {
                session.title.clone_from(name);
            }
        }
        let pending = snapshot.offset < length;
        self.snapshot = Some(snapshot);
        Ok(pending)
    }
}

#[derive(Debug)]
struct Snapshot {
    file: FileId,
    length: u64,
    modified: SystemTime,
    offset: u64,
    partial: Vec<u8>,
    skip_record: bool,
    names: HashMap<String, Option<String>>,
}

impl Snapshot {
    fn reusable(
        &self,
        file: FileId,
        length: u64,
        modified: SystemTime,
        ids: &HashSet<&str>,
    ) -> bool {
        file == self.file
            && (length > self.length || (length == self.length && modified == self.modified))
            && self.names.len() == ids.len()
            && ids.iter().all(|id| self.names.contains_key(*id))
    }

    fn read(&mut self, mut file: fs::File, budget: u64) -> io::Result<()> {
        #[derive(serde::Deserialize)]
        struct Entry {
            id: String,
            thread_name: String,
        }

        let remaining = (self.length - self.offset).min(budget);
        if remaining == 0 {
            return Ok(());
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let end = self.offset + remaining;
        let mut reader = BufReader::new(file.take(remaining));
        let mut line = Vec::new();
        while reader.read_until(b'\n', &mut line)? > 0 {
            self.offset += line.len() as u64;
            if !self.skip_record && self.partial.len() + line.len() <= INDEX_RECORD_BYTES {
                self.partial.extend_from_slice(&line);
            } else {
                self.partial.clear();
                self.skip_record = true;
            }
            if line.last() == Some(&b'\n') {
                if !self.skip_record
                    && let Ok(entry) = serde_json::from_slice::<Entry>(&self.partial)
                    && let Some(slot) = self.names.get_mut(&entry.id)
                    && let Some(name) = normalize_title(&entry.thread_name)
                {
                    *slot = Some(name);
                }
                self.partial.clear();
                self.skip_record = false;
            }
            line.clear();
        }
        if self.offset != end {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(())
    }
}
