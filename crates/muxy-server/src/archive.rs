mod layout;
mod legacy;
mod storage;

use storage::StoredRecord;

use std::collections::BTreeMap;
use std::fs::{self, DirBuilder, File};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, BufWriter, Read, Write};
use std::ops::Range;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use muxy_protocol::{
    ErrorCode, ExitReason, HistoryCursor, HistoryPage, Message, ReplyBody, RequestId, Row, Run,
    SavedScreen, SearchPage, SessionId,
};
use muxy_terminal::{TerminalArchive, TerminalSnapshot};

use crate::ServerError;
use crate::search::Search;

/// The only whole-file record version, written before records were indexed.
const LEGACY_VERSION: u32 = 1;
const SCREEN_LIMIT: u64 = 16 * 1024 * 1024 - 1024;
type Completion = Sender<Result<(), String>>;
type Queue = Arc<Mutex<BTreeMap<SessionId, Job>>>;

#[derive(Clone, Debug)]
struct Record {
    screen: SavedScreen,
    history: Vec<Vec<Run>>,
}

impl Record {
    fn new(terminal: TerminalArchive, reason: Option<ExitReason>) -> Self {
        Self {
            screen: SavedScreen {
                graphics: terminal.graphics,
                size: terminal.size,
                rows: terminal.rows,
                cursor: terminal.cursor,
                reason,
            },
            history: terminal.history,
        }
    }

    fn validate(&self) -> io::Result<()> {
        validate_screen(&self.screen)
    }
}

fn validate_screen(screen: &SavedScreen) -> io::Result<()> {
    if storage::serialized_size(screen)? > SCREEN_LIMIT {
        return Err(io::Error::other("saved screen exceeds the size limit"));
    }
    Message::Reply {
        id: RequestId(0),
        body: ReplyBody::SavedScreen(screen.clone()),
    }
    .validate()
    .map_err(|error| io::Error::other(format!("invalid saved screen: {error:?}")))
}

/// A terminal copy waiting to become a record, off the session's thread.
#[derive(Debug)]
struct Pending {
    snapshot: TerminalSnapshot,
    reason: Option<ExitReason>,
}

impl Pending {
    fn record(&self) -> io::Result<Record> {
        let terminal = self.snapshot.archive().map_err(io::Error::other)?;
        Ok(Record::new(terminal, self.reason))
    }
}

#[derive(Debug)]
enum Operation {
    Save(Pending),
    Write(Record),
    Discard,
}

#[derive(Debug)]
struct Job {
    operation: Operation,
    completions: Vec<Completion>,
}

#[derive(Debug)]
struct Disk {
    directory: PathBuf,
    budget: u64,
    queue: Queue,
    wake: Option<SyncSender<()>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    _lock: File,
}

impl Drop for Disk {
    fn drop(&mut self) {
        self.wake.take();
        if let Some(worker) = lock(&self.worker).take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug)]
enum Backend {
    Memory {
        records: Mutex<BTreeMap<SessionId, Record>>,
        revision: AtomicU64,
    },
    Disk(Disk),
}

#[derive(Clone, Debug)]
pub(crate) struct Archive(Arc<Backend>);

#[derive(Debug, Default)]
pub(crate) struct SearchCache {
    record: Option<(SessionId, [u64; 3], StoredRecord, u64)>,
}

impl Archive {
    pub(crate) fn memory(_budget: u64) -> Self {
        Self(Arc::new(Backend::Memory {
            records: Mutex::default(),
            revision: AtomicU64::new(0),
        }))
    }

    pub(crate) fn open(directory: &Path, budget: u64) -> io::Result<Self> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        let directory_lock = File::open(directory)?;
        muxy_core::file_lock::acquire_file(&directory_lock, Duration::from_secs(3))?;
        let queue = Queue::default();
        let jobs = Arc::clone(&queue);
        let (wake, receiver) = mpsc::sync_channel(1);
        let root = directory.to_owned();
        let worker = thread::Builder::new()
            .name("muxy-terminal-save".into())
            .spawn(move || {
                write_jobs(&root, &jobs, &receiver);
            })?;
        Ok(Self(Arc::new(Backend::Disk(Disk {
            directory: directory.to_owned(),
            budget,
            queue,
            wake: Some(wake),
            worker: Mutex::new(Some(worker)),
            _lock: directory_lock,
        }))))
    }

    pub(crate) fn ids(&self) -> io::Result<Vec<SessionId>> {
        match self.0.as_ref() {
            Backend::Memory { records, .. } => Ok(lock(records).keys().copied().collect()),
            Backend::Disk(disk) => {
                let mut ids = Vec::new();
                for entry in fs::read_dir(&disk.directory)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    if let Some(stem) = name
                        .to_str()
                        .and_then(|name| name.strip_suffix(".postcard"))
                        && stem.len() == 16
                        && let Ok(value) = u64::from_str_radix(stem, 16)
                        && let Some(id) = SessionId::new(value)
                    {
                        ids.push(id);
                    }
                }
                Ok(ids)
            }
        }
    }

    pub(crate) fn contains(&self, session: SessionId) -> bool {
        match self.0.as_ref() {
            Backend::Memory { records, .. } => lock(records).contains_key(&session),
            Backend::Disk(disk) => record_path(&disk.directory, session).exists(),
        }
    }

    pub(crate) fn read(&self, session: SessionId) -> io::Result<SavedScreen> {
        self.load(session).map(|record| record.screen().clone())
    }

    pub(crate) fn history_page(
        &self,
        session: SessionId,
        before: HistoryCursor,
        max_rows: u16,
    ) -> Result<HistoryPage, ServerError> {
        let failed = |error| {
            ServerError::new(
                ErrorCode::SavedContentUnavailable,
                format!("saved history: {error}"),
            )
        };
        let mut record = self.load(session).map_err(failed)?;
        let generation = record.generation();
        let total = record.total();
        let (range, _) = history_range(session, generation, before, max_rows, total)?;
        let mut rows = Vec::new();
        let mut used = 128_u64;
        for index in range.rev() {
            let runs = record.row(index).map_err(failed)?;
            used = used.saturating_add(storage::serialized_size(&runs).map_err(failed)? + 10);
            if used > SCREEN_LIMIT {
                break;
            }
            rows.push(Row { index: 0, runs });
        }
        if rows.is_empty() && total > 0 {
            return Err(ServerError::new(
                ErrorCode::HistoryUnavailable,
                "history row exceeds the page size limit",
            ));
        }
        rows.reverse();
        for (index, row) in rows.iter_mut().enumerate() {
            row.index = u16::try_from(index).unwrap_or(u16::MAX);
        }
        let next = if rows.is_empty() {
            None
        } else {
            history_range(
                session,
                generation,
                before,
                u16::try_from(rows.len()).unwrap_or(max_rows),
                total,
            )?
            .1
        };
        Ok(HistoryPage {
            links: None,
            prompts: Vec::new(),
            rows,
            next,
            total_rows: total as u64,
            screen: None,
        })
    }

    pub(crate) fn search(
        &self,
        session: SessionId,
        query: &str,
        ignore_case: bool,
        before: HistoryCursor,
        max_results: u16,
        cache: &mut SearchCache,
    ) -> Result<SearchPage, ServerError> {
        let failed = |error| {
            ServerError::new(
                ErrorCode::SavedContentUnavailable,
                format!("saved search: {error}"),
            )
        };
        let stamp = match self.0.as_ref() {
            Backend::Memory { revision, .. } => [revision.load(Ordering::Acquire), 0, 0],
            Backend::Disk(disk) => {
                let metadata =
                    fs::metadata(record_path(&disk.directory, session)).map_err(failed)?;
                [metadata.dev(), metadata.ino(), metadata.len()]
            }
        };
        if cache
            .record
            .as_ref()
            .is_none_or(|(id, previous, _, _)| *id != session || *previous != stamp)
        {
            let record = self.load(session).map_err(failed)?;
            let mut hasher = DefaultHasher::new();
            record.generation().hash(&mut hasher);
            for row in &record.screen().rows {
                row.runs.hash(&mut hasher);
            }
            cache.record = Some((session, stamp, record, hasher.finish()));
        }
        let (_, _, record, generation) = cache
            .record
            .as_mut()
            .ok_or_else(|| failed(io::Error::other("missing search record")))?;
        let search = Search {
            session,
            generation: *generation,
            query,
            ignore_case,
            before,
            max_results,
            history_rows: record.total(),
            screen_rows: record.screen().rows.len(),
        };
        search.scan(|index| {
            if index < record.total() {
                record.row(index).map_err(failed)
            } else {
                Ok(record.screen().rows[index - record.total()].runs.clone())
            }
        })
    }

    fn load(&self, session: SessionId) -> io::Result<StoredRecord> {
        let record = match self.0.as_ref() {
            Backend::Memory { records, .. } => {
                let record = lock(records).get(&session).cloned().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "no saved terminal content")
                })?;
                record.validate()?;
                StoredRecord::Legacy(record)
            }
            Backend::Disk(disk) => {
                StoredRecord::open(&record_path(&disk.directory, session), disk.budget)?
            }
        };
        Ok(record)
    }

    pub(crate) fn save(
        &self,
        session: SessionId,
        snapshot: TerminalSnapshot,
        reason: Option<ExitReason>,
    ) -> io::Result<()> {
        let pending = Pending { snapshot, reason };
        // An ending session waits for its save; converting here keeps it from
        // queueing behind other sessions' conversions on the single writer.
        let operation = if reason.is_some() {
            Operation::Write(pending.record()?)
        } else {
            Operation::Save(pending)
        };
        self.submit(session, operation, reason.is_some())
    }

    pub(crate) fn discard(&self, session: SessionId) -> io::Result<()> {
        self.submit(session, Operation::Discard, true)
    }

    fn submit(&self, session: SessionId, operation: Operation, wait: bool) -> io::Result<()> {
        match self.0.as_ref() {
            Backend::Memory { records, revision } => {
                match operation {
                    Operation::Save(pending) => {
                        let record = pending.record()?;
                        record.validate()?;
                        lock(records).insert(session, record);
                    }
                    Operation::Write(record) => {
                        record.validate()?;
                        lock(records).insert(session, record);
                    }
                    Operation::Discard => {
                        lock(records).remove(&session);
                    }
                }
                revision.fetch_add(1, Ordering::Release);
                Ok(())
            }
            Backend::Disk(disk) => {
                let (done, completed) = mpsc::channel();
                {
                    let mut queue = lock(&disk.queue);
                    let job = queue.entry(session).or_insert_with(|| Job {
                        operation: Operation::Discard,
                        completions: Vec::new(),
                    });
                    job.operation = operation;
                    if wait {
                        job.completions.push(done);
                    }
                }
                if let Some(wake) = &disk.wake {
                    match wake.try_send(()) {
                        Ok(()) | Err(mpsc::TrySendError::Full(())) => {}
                        Err(mpsc::TrySendError::Disconnected(())) => {
                            return Err(io::Error::other("terminal save worker stopped"));
                        }
                    }
                }
                if wait {
                    completed
                        .recv()
                        .map_err(io::Error::other)?
                        .map_err(io::Error::other)?;
                }
                Ok(())
            }
        }
    }
}

pub(crate) fn history_range(
    session: SessionId,
    generation: u64,
    before: HistoryCursor,
    max_rows: u16,
    total: usize,
) -> Result<(Range<usize>, Option<HistoryCursor>), ServerError> {
    if !(1..=500).contains(&max_rows) {
        return Err(ServerError::new(
            ErrorCode::BadRequest,
            "history pages contain 1 through 500 rows",
        ));
    }
    let total = u32::try_from(total).map_err(|_| {
        ServerError::new(
            ErrorCode::HistoryUnavailable,
            "history exceeds the cursor range",
        )
    })?;
    let mut hasher = DefaultHasher::new();
    session.hash(&mut hasher);
    generation.hash(&mut hasher);
    let tag = hasher.finish() & !u64::from(u32::MAX);
    let end = if before.0 == 0 {
        total
    } else {
        let end = u32::try_from(before.0 & u64::from(u32::MAX)).unwrap_or(u32::MAX);
        if end == 0 || end > total || before.0 != tag | u64::from(end) {
            return Err(ServerError::new(
                ErrorCode::StaleHistoryCursor,
                "history changed; start from a fresh window",
            ));
        }
        end
    };
    let start = end.saturating_sub(u32::from(max_rows));
    let next = (start > 0).then_some(HistoryCursor(tag | u64::from(start)));
    Ok((start as usize..end as usize, next))
}

pub(crate) fn bound_history_page(
    session: SessionId,
    generation: u64,
    before: HistoryCursor,
    mut page: HistoryPage,
) -> Result<HistoryPage, ServerError> {
    let size_error = |error| {
        ServerError::new(
            ErrorCode::HistoryUnavailable,
            format!("history page: {error}"),
        )
    };
    let screen_size = postcard::experimental::serialized_size(&page.screen).map_err(size_error)?;
    let link_size = |row: &muxy_protocol::LinkRow| {
        row.spans.iter().fold(8_u64, |size, span| {
            size.saturating_add(span.uri.len() as u64)
                .saturating_add(16)
        })
    };
    let mut links = page.links.iter().flatten().rev().peekable();
    let mut used = u64::try_from(screen_size)
        .unwrap_or(u64::MAX)
        .saturating_add(128)
        .saturating_add(
            u64::try_from(page.prompts.len())
                .unwrap_or(u64::MAX)
                .saturating_mul(3),
        );
    while let Some(row) = links.next_if(|row| usize::from(row.row) >= page.rows.len()) {
        used = used.saturating_add(link_size(row));
    }
    let mut keep = 0_u16;
    for row in page.rows.iter().rev() {
        if let Some(row) = links.next_if(|links| links.row == row.index) {
            used = used.saturating_add(link_size(row));
        }
        let size = postcard::experimental::serialized_size(row).map_err(size_error)?;
        used = used.saturating_add(u64::try_from(size).unwrap_or(u64::MAX));
        if used > SCREEN_LIMIT {
            break;
        }
        keep += 1;
    }
    if usize::from(keep) == page.rows.len() {
        return Ok(page);
    }
    if keep == 0 && page.links.take().is_some() {
        return bound_history_page(session, generation, before, page);
    }
    if keep == 0 {
        return Err(ServerError::new(
            ErrorCode::HistoryUnavailable,
            "history row exceeds the page size limit",
        ));
    }
    let removed = page.rows.len() - usize::from(keep);
    page.prompts = page
        .prompts
        .into_iter()
        .filter_map(|row| {
            usize::from(row)
                .checked_sub(removed)
                .and_then(|row| u16::try_from(row).ok())
        })
        .collect();
    if let Some(links) = &mut page.links {
        links.retain_mut(|row| {
            let Some(index) = usize::from(row.row).checked_sub(removed) else {
                return false;
            };
            row.row = u16::try_from(index).unwrap_or(u16::MAX);
            true
        });
    }
    page.rows.drain(..removed);
    for (index, row) in page.rows.iter_mut().enumerate() {
        row.index = u16::try_from(index).unwrap_or(u16::MAX);
    }
    let total = usize::try_from(page.total_rows).unwrap_or(usize::MAX);
    page.next = history_range(session, generation, before, keep, total)?.1;
    Ok(page)
}

fn write_jobs(directory: &Path, queue: &Queue, wake: &Receiver<()>) {
    while wake.recv().is_ok() {
        loop {
            let job = lock(queue).pop_first();
            let Some((session, job)) = job else { break };
            let result = match job.operation {
                Operation::Save(pending) => pending
                    .record()
                    .and_then(|record| write_record(directory, session, &record)),
                Operation::Write(record) => write_record(directory, session, &record),
                Operation::Discard => discard_record(directory, session),
            }
            .map_err(|error| error.to_string());
            if let Err(error) = &result {
                log::error!("session {} saved content: {error}", session.get());
            }
            for completion in job.completions {
                let _ = completion.send(result.clone());
            }
        }
    }
}

fn record_path(directory: &Path, session: SessionId) -> PathBuf {
    directory.join(format!("{:016x}.postcard", session.get()))
}

fn read_record(path: &Path, budget: u64) -> io::Result<Record> {
    let limit = budget.saturating_add(SCREEN_LIMIT).saturating_add(1024);
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(io::Error::other(
            "saved terminal record exceeds the size limit",
        ));
    }
    let (version, _) = postcard::take_from_bytes::<u32>(&bytes).map_err(io::Error::other)?;
    if version != LEGACY_VERSION {
        return Err(io::Error::other(format!(
            "unsupported terminal record version {version}"
        )));
    }
    let old: legacy::Record = postcard::from_bytes(&bytes).map_err(io::Error::other)?;
    let record = Record {
        screen: old.screen.into(),
        history: old
            .history
            .into_iter()
            .map(|row| row.into_iter().map(Into::into).collect())
            .collect(),
    };
    record.validate()?;
    Ok(record)
}

fn write_record(directory: &Path, session: SessionId, record: &Record) -> io::Result<()> {
    Ok(muxy_core::fs::write_atomic_with(
        &record_path(directory, session),
        0o600,
        |file| write_buffered(record, file),
    )?)
}

fn write_buffered(record: &Record, writer: impl Write) -> io::Result<()> {
    let mut buffered = BufWriter::new(writer);
    storage::write(record, &mut buffered)?;
    buffered.flush()
}

fn discard_record(directory: &Path, session: SessionId) -> io::Result<()> {
    match fs::remove_file(record_path(directory, session)) {
        Ok(()) => File::open(directory)?.sync_all(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_protocol::{
        CellSize, Color, Cursor, CursorShape, GraphicImage, GraphicPlacement, Graphics, Style,
        Underline,
    };
    use muxy_terminal::{Size, Terminal};
    use std::error::Error;
    use std::io::Read;
    use std::sync::atomic::{AtomicU64, Ordering};

    type TestResult = Result<(), Box<dyn Error>>;
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "muxy-archive-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn capture(marker: &str) -> Result<TerminalSnapshot, Box<dyn Error>> {
        let mut terminal = Terminal::new(Size { cols: 30, rows: 3 }, 1024 * 1024)?;
        for row in 0..100 {
            terminal.feed(format!("\x1b[31mhistory-{row:03}\r\n").as_bytes());
        }
        terminal.feed(marker.as_bytes());
        Ok(terminal
            .snapshot()?
            .ok_or("output stops inside a sequence")?)
    }

    #[test]
    fn pre_render_upgrade_records_keep_screen_history_and_styles() -> TestResult {
        let directory = Directory::new()?;
        for (name, bytes) in [
            (
                "v1",
                include_bytes!("../tests/fixtures/render-v1.postcard").as_slice(),
            ),
            (
                "v2",
                include_bytes!("../tests/fixtures/render-v2.postcard").as_slice(),
            ),
        ] {
            let path = directory.0.join(name);
            fs::write(&path, bytes)?;
            let mut record = StoredRecord::open(&path, 65536)?;
            assert_eq!(record.screen().rows[0].runs[0].text, "preserved");
            assert_eq!(
                record.screen().rows[0].runs[0].style.underline,
                Underline::Single
            );
            assert_eq!(record.screen().cursor.shape, CursorShape::Block);
            assert_eq!(record.row(0)?[0].text, "preserved");
            assert_eq!(record.total(), 1);
        }
        Ok(())
    }

    fn sample() -> Record {
        let style = Style {
            fg: Color::Rgb(1, 2, 3),
            bg: Color::Indexed(4),
            bold: true,
            italic: true,
            underline: Underline::Curly,
            underline_color: Color::Indexed(9),
            invisible: false,
            overline: true,
            inverse: true,
            strikethrough: true,
            faint: true,
        };
        let run = |text: &str| Run {
            text: text.into(),
            width: u16::try_from(text.len()).unwrap_or(u16::MAX),
            style,
        };
        Record {
            screen: SavedScreen {
                graphics: Graphics {
                    cell: CellSize {
                        width: 9,
                        height: 18,
                    },
                    images: vec![GraphicImage {
                        id: 7,
                        generation: 2,
                        width: 1,
                        height: 2,
                        rgba: Arc::from([1, 2, 3, 4, 5, 6, 7, 8].as_slice()),
                    }],
                    placements: vec![GraphicPlacement {
                        image: 7,
                        id: 3,
                        column: -1,
                        row: 1,
                        offset: [1, 2],
                        size: [3, 4],
                        source: [0, 0, 1, 2],
                        z: -5,
                    }],
                },
                size: Size { cols: 20, rows: 2 },
                rows: vec![
                    Row {
                        index: 0,
                        runs: vec![run("screen")],
                    },
                    Row {
                        index: 1,
                        runs: Vec::new(),
                    },
                ],
                cursor: Cursor {
                    shape: CursorShape::Bar,
                    row: 1,
                    col: 4,
                    visible: false,
                },
                reason: Some(ExitReason::Signaled(15)),
            },
            history: vec![
                vec![run("first")],
                vec![
                    run("second"),
                    Run {
                        text: "plain".into(),
                        width: 5,
                        style: Style::default(),
                    },
                ],
            ],
        }
    }

    #[test]
    fn this_release_keeps_writing_the_checked_in_layout() -> TestResult {
        let mut written = Vec::new();
        storage::write(&sample(), &mut written)?;
        let fixture = include_bytes!("../tests/fixtures/terminal-v4.postcard");
        let std_hash_that_rust_releases_may_change = 24..32;
        assert_eq!(
            written[..std_hash_that_rust_releases_may_change.start],
            fixture[..std_hash_that_rust_releases_may_change.start]
        );
        assert_eq!(
            written[std_hash_that_rust_releases_may_change.end..],
            fixture[std_hash_that_rust_releases_may_change.end..]
        );
        Ok(())
    }

    #[test]
    fn indexed_layouts_load_with_screen_graphics_reason_and_history() -> TestResult {
        let directory = Directory::new()?;
        let expected = sample();
        for (name, bytes) in [
            (
                "v3",
                include_bytes!("../tests/fixtures/terminal-v3.postcard").as_slice(),
            ),
            (
                "v4",
                include_bytes!("../tests/fixtures/terminal-v4.postcard").as_slice(),
            ),
        ] {
            let path = directory.0.join(name);
            fs::write(&path, bytes)?;
            let mut record = StoredRecord::open(&path, 65536)?;
            assert!(matches!(record, StoredRecord::Indexed(_)));
            assert_eq!(*record.screen(), expected.screen, "{name}");
            assert_eq!(record.total(), expected.history.len());
            for (index, row) in expected.history.iter().enumerate() {
                assert_eq!(record.row(index)?, *row, "{name} row {index}");
            }
        }
        Ok(())
    }

    #[test]
    fn saved_search_reuses_the_open_record_and_detects_replacement() -> TestResult {
        let directory = Directory::new()?;
        let session = SessionId::new(1).ok_or("zero ID")?;
        let archive = Archive::open(&directory.0, 1024 * 1024)?;
        archive.save(session, capture("done")?, Some(ExitReason::Exited(0)))?;
        let mut cache = SearchCache::default();
        let first = archive.search(session, "history", false, HistoryCursor(0), 1, &mut cache)?;
        let cursor = first.next.ok_or("missing cursor")?;
        let cached = cache.record.as_ref().ok_or("missing cached record")?;
        assert!(matches!(cached.2, StoredRecord::Indexed(_)));
        let next = archive.search(session, "history", false, cursor, 1, &mut cache)?;
        assert!(next.matches[0].row < first.matches[0].row);
        archive.save(
            session,
            capture("replacement")?,
            Some(ExitReason::Exited(0)),
        )?;
        assert_eq!(
            archive
                .search(session, "history", false, cursor, 1, &mut cache)
                .err()
                .map(|error| error.code()),
            Some(ErrorCode::StaleHistoryCursor)
        );
        archive.discard(session)?;
        assert!(
            archive
                .search(session, "history", false, cursor, 1, &mut cache)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn large_pages_fit_the_wire_limit_without_skipping_rows() -> TestResult {
        let session = SessionId::new(1).ok_or("zero ID")?;
        let row = Row {
            index: 0,
            runs: vec![
                Run {
                    text: "wide".into(),
                    width: 1,
                    style: Style::default()
                };
                4096
            ],
        };
        let page = bound_history_page(
            session,
            7,
            HistoryCursor(0),
            HistoryPage {
                links: Some(
                    [0, 499]
                        .into_iter()
                        .map(|row| muxy_protocol::LinkRow {
                            row,
                            spans: vec![muxy_protocol::LinkSpan {
                                start: 0,
                                end: 4,
                                uri: format!("https://test/{row}"),
                            }],
                        })
                        .collect(),
                ),
                prompts: vec![0, 499],
                rows: (0..500)
                    .map(|index| Row {
                        index,
                        ..row.clone()
                    })
                    .collect(),
                next: None,
                total_rows: 500,
                screen: None,
            },
        )?;
        assert!(page.rows.len() < 500);
        assert!(!page.rows.is_empty());
        assert!(postcard::experimental::serialized_size(&page)? < usize::try_from(SCREEN_LIMIT)?);
        let kept = page.rows.len();
        assert_eq!(page.prompts, [u16::try_from(kept - 1)?]);
        let links = page.links.as_ref().ok_or("missing links")?;
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].row, u16::try_from(kept - 1)?);
        assert_eq!(links[0].spans[0].uri, "https://test/499");
        let (older, next) =
            history_range(session, 7, page.next.ok_or("missing cursor")?, 500, 500)?;
        assert_eq!(older, 0..500 - kept);
        assert_eq!(next, None);
        assert_eq!(page.rows.first().ok_or("empty page")?.index, 0);
        Ok(())
    }

    #[test]
    fn links_that_exhaust_the_page_budget_are_dropped_instead_of_failing() -> TestResult {
        let session = SessionId::new(1).ok_or("zero ID")?;
        let row = Row {
            index: 0,
            runs: vec![Run {
                text: "row".into(),
                width: 3,
                style: Style::default(),
            }],
        };
        let page = bound_history_page(
            session,
            7,
            HistoryCursor(0),
            HistoryPage {
                links: Some(vec![muxy_protocol::LinkRow {
                    row: 1,
                    spans: vec![muxy_protocol::LinkSpan {
                        start: 0,
                        end: 1,
                        uri: "x".repeat(usize::try_from(SCREEN_LIMIT)?),
                    }],
                }]),
                prompts: Vec::new(),
                rows: vec![row.clone()],
                next: None,
                total_rows: 1,
                screen: None,
            },
        )?;
        assert_eq!(page.rows, [row]);
        assert_eq!(page.links, None);
        Ok(())
    }

    #[test]
    fn records_round_trip_atomically_with_private_permissions_and_all_retained_rows() -> TestResult
    {
        let directory = Directory::new()?;
        let archive = Archive::open(&directory.0, 512)?;
        let session = SessionId::new(1).ok_or("zero ID")?;
        archive.save(session, capture("first")?, Some(ExitReason::Exited(3)))?;
        let path = record_path(&directory.0, session);
        let old_bytes = fs::read(&path)?;
        let mut old_file = File::open(&path)?;
        archive.save(session, capture("last")?, Some(ExitReason::Exited(7)))?;
        let mut still_old = Vec::new();
        old_file.read_to_end(&mut still_old)?;
        assert_eq!(old_bytes, still_old);
        assert_ne!(fs::read(&path)?, old_bytes);
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(&directory.0)?.permissions().mode() & 0o777,
            0o700
        );
        let record = StoredRecord::open(&path, 512)?;
        assert_eq!(record.total(), capture("last")?.archive()?.history.len());
        assert_eq!(*record.screen(), archive.read(session)?);
        assert_eq!(record.screen().reason, Some(ExitReason::Exited(7)));
        drop(archive);
        let reopened = Archive::open(&directory.0, 512)?;
        assert_eq!(reopened.read(session)?, *record.screen());
        Ok(())
    }

    #[test]
    fn corrupt_and_unsupported_records_do_not_hide_other_records() -> TestResult {
        let directory = Directory::new()?;
        let archive = Archive::open(&directory.0, 1024)?;
        let first = SessionId::new(1).ok_or("zero ID")?;
        let second = SessionId::new(2).ok_or("zero ID")?;
        archive.save(first, capture("good")?, Some(ExitReason::Ended))?;
        let path = record_path(&directory.0, second);
        fs::write(&path, b"broken")?;
        assert!(archive.read(second).is_err());
        let mut newer_version = include_bytes!("../tests/fixtures/render-v1.postcard").to_vec();
        newer_version[0] = u8::try_from(LEGACY_VERSION + 1)?;
        fs::write(&path, &newer_version)?;
        newer_version[0] = u8::try_from(LEGACY_VERSION)?;
        assert!(postcard::from_bytes::<legacy::Record>(&newer_version).is_ok());
        assert!(archive.read(second).is_err());
        assert!(archive.read(first).is_ok());
        Ok(())
    }

    #[test]
    fn failed_write_preserves_previous_complete_record_and_discard_drains_pending_saves()
    -> TestResult {
        let directory = Directory::new()?;
        let root = directory.0.join("records");
        let moved = directory.0.join("moved");
        let archive = Archive::open(&root, 1024)?;
        let session = SessionId::new(1).ok_or("zero ID")?;
        archive.save(session, capture("complete")?, Some(ExitReason::Ended))?;
        let before = archive.read(session)?;
        fs::rename(&root, &moved)?;
        fs::write(&root, "not a directory")?;
        assert!(
            archive
                .save(session, capture("failed")?, Some(ExitReason::Ended))
                .is_err()
        );
        fs::remove_file(&root)?;
        fs::rename(&moved, &root)?;
        assert_eq!(archive.read(session)?, before);
        for _ in 0..20 {
            archive.save(session, capture("queued")?, None)?;
        }
        archive.discard(session)?;
        archive.discard(session)?;
        drop(archive);
        assert!(!record_path(&root, session).exists());
        assert!(fs::read_dir(&root)?.next().is_none());
        Ok(())
    }

    #[test]
    fn styled_history_keeps_the_live_retained_window_on_disk_and_after_restart() -> TestResult {
        let directory = Directory::new()?;
        let budget = 64 * 1024;
        let mut terminal = Terminal::new(Size { cols: 80, rows: 24 }, budget)?;
        for _ in 0..10 {
            for index in 0..500 {
                use std::fmt::Write as _;
                let mut line = String::new();
                for col in 0..78 {
                    write!(
                        line,
                        "\x1b[38;2;{};{};{}mX",
                        col,
                        index % 255,
                        (col * 3) % 255
                    )?;
                }
                line.push_str("\r\n");
                terminal.feed(line.as_bytes());
            }
            terminal.compress_idle()?;
        }
        let live_rows = terminal.history_rows()?;
        let expected = Record::new(terminal.archive()?, Some(ExitReason::Ended));
        assert!(storage::serialized_size(&expected.history)? > budget as u64);
        let session = SessionId::from(std::num::NonZeroU64::MIN);
        let archive = Archive::open(&directory.0, budget as u64)?;
        let snapshot = terminal
            .snapshot()?
            .ok_or("output stops inside a sequence")?;
        archive.save(session, snapshot, Some(ExitReason::Ended))?;
        drop(archive);
        let archive = Archive::open(&directory.0, budget as u64)?;
        assert_eq!(archive.read(session)?, expected.screen);
        let mut before = HistoryCursor(0);
        let mut end = live_rows;
        loop {
            let page = archive.history_page(session, before, 123)?;
            assert_eq!(page.total_rows, live_rows as u64);
            let start = end - page.rows.len();
            let actual: Vec<_> = page.rows.into_iter().map(|row| row.runs).collect();
            assert_eq!(actual, expected.history[start..end]);
            end = start;
            let Some(next) = page.next else { break };
            before = next;
        }
        assert_eq!(end, 0);
        Ok(())
    }
}
