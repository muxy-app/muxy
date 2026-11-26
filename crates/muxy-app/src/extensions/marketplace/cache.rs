use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use muxy_app_core::extensions::Marketplace;

pub(super) const FRESH: Duration = Duration::from_secs(300);
pub(super) const DEFAULT_RETRY: Duration = Duration::from_secs(60);
pub(super) const MAX_RETRY: Duration = Duration::from_secs(3600);
const PICTURE_BUDGET: u64 = 64 * 1024 * 1024;

pub(super) enum Fetched {
    Modified { body: Vec<u8>, etag: Option<String> },
    NotModified,
}

pub(super) struct Failure {
    pub(super) message: String,
    pub(super) retry_after: Option<Duration>,
}

impl Failure {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retry_after: None,
        }
    }
}

struct Entry<T> {
    value: T,
    etag: Option<String>,
    checked: Option<Instant>,
    retry_at: Option<Instant>,
}

pub(super) struct Indexes<T> {
    entries: BTreeMap<Marketplace, Entry<T>>,
}

impl<T: Clone> Indexes<T> {
    pub(super) const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub(super) fn expire(&mut self) {
        for entry in self.entries.values_mut() {
            entry.checked = None;
        }
    }

    pub(super) fn load(
        &mut self,
        source: &Marketplace,
        now: Instant,
        directory: Option<&Path>,
        parse: impl Fn(&[u8]) -> Result<T, String>,
        fetch: impl FnOnce(Option<&str>) -> Result<Fetched, Failure>,
    ) -> Result<T, String> {
        if !self.entries.contains_key(source)
            && let Some((body, etag)) =
                directory.and_then(|directory| read_index(directory, source))
            && let Ok(value) = parse(&body)
        {
            self.entries.insert(
                source.clone(),
                Entry {
                    value,
                    etag,
                    checked: None,
                    retry_at: None,
                },
            );
        }
        if let Some(entry) = self.entries.get(source)
            && (entry
                .checked
                .is_some_and(|checked| now.saturating_duration_since(checked) < FRESH)
                || entry.retry_at.is_some_and(|retry_at| now < retry_at))
        {
            return Ok(entry.value.clone());
        }
        let etag = self
            .entries
            .get(source)
            .and_then(|entry| entry.etag.clone());
        match fetch(etag.as_deref()) {
            Ok(Fetched::NotModified) => {
                let Some(entry) = self.entries.get_mut(source) else {
                    return Err("The marketplace answered without an index.".into());
                };
                entry.checked = Some(now);
                entry.retry_at = None;
                Ok(entry.value.clone())
            }
            Ok(Fetched::Modified { body, etag }) => {
                let value = parse(&body)?;
                if let Some(directory) = directory {
                    write_index(directory, source, &body, etag.as_deref());
                }
                self.entries.insert(
                    source.clone(),
                    Entry {
                        value: value.clone(),
                        etag,
                        checked: Some(now),
                        retry_at: None,
                    },
                );
                Ok(value)
            }
            Err(failure) => {
                let Some(entry) = self.entries.get_mut(source) else {
                    return Err(failure.message);
                };
                entry.retry_at = Some(now + retry_delay(failure.retry_after));
                Ok(entry.value.clone())
            }
        }
    }
}

pub(super) fn retry_delay(retry_after: Option<Duration>) -> Duration {
    retry_after.unwrap_or(DEFAULT_RETRY).min(MAX_RETRY)
}

pub(super) fn directory() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    muxy_core::dirs::muxy_dir()
        .ok()
        .map(|directory| directory.join("cache").join("marketplace"))
}

fn index_path(directory: &Path, source: &Marketplace) -> PathBuf {
    directory
        .join("indexes")
        .join(format!("{}.json", source.as_str().replace('/', "@")))
}

fn read_index(directory: &Path, source: &Marketplace) -> Option<(Vec<u8>, Option<String>)> {
    let path = index_path(directory, source);
    let body = std::fs::read(&path).ok()?;
    let etag = std::fs::read_to_string(path.with_extension("etag"))
        .ok()
        .map(|etag| etag.trim().to_owned())
        .filter(|etag| !etag.is_empty());
    Some((body, etag))
}

fn write_index(directory: &Path, source: &Marketplace, body: &[u8], etag: Option<&str>) {
    let path = index_path(directory, source);
    let etag_path = path.with_extension("etag");
    let Some(parent) = path.parent() else {
        return;
    };
    let _ = std::fs::remove_file(&etag_path);
    if std::fs::create_dir_all(parent).is_ok()
        && muxy_core::fs::write_atomic(&path, body, 0o600).is_ok()
        && let Some(etag) = etag
    {
        let _ = muxy_core::fs::write_atomic(&etag_path, etag.as_bytes(), 0o600);
    }
}

fn picture_path(directory: &Path, url: &str) -> PathBuf {
    directory
        .join("pictures")
        .join(muxy_core::hex::encode(&muxy_core::sha256::digest(
            url.as_bytes(),
        )))
}

pub(super) fn read_picture(directory: &Path, url: &str, limit: u64) -> Option<Vec<u8>> {
    let path = picture_path(directory, url);
    let bytes = std::fs::read(&path)
        .ok()
        .filter(|bytes| !bytes.is_empty() && bytes.len() as u64 <= limit)?;
    let _ = std::fs::File::options()
        .write(true)
        .open(&path)
        .and_then(|file| file.set_modified(SystemTime::now()));
    Some(bytes)
}

pub(super) fn write_picture(directory: &Path, url: &str, bytes: &[u8]) {
    write_picture_within(directory, url, bytes, PICTURE_BUDGET);
}

fn write_picture_within(directory: &Path, url: &str, bytes: &[u8], budget: u64) {
    let path = picture_path(directory, url);
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_ok()
        && muxy_core::fs::write_atomic(&path, bytes, 0o600).is_ok()
    {
        prune(parent, budget);
    }
}

fn prune(directory: &Path, budget: u64) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then(|| {
                (
                    metadata.modified().unwrap_or(UNIX_EPOCH),
                    metadata.len(),
                    entry.path(),
                )
            })
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    files.sort();
    for (_, size, path) in files {
        if total <= budget {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "Tests fail immediately on fixture errors"
    )]
    use std::cell::Cell;

    use super::*;

    fn source() -> Marketplace {
        Marketplace::parse("someone/extensions").unwrap()
    }

    fn parse(body: &[u8]) -> Result<String, String> {
        String::from_utf8(body.to_vec()).map_err(|error| error.to_string())
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "Stands in for a fetch, which can fail"
    )]
    fn modified(body: &str, etag: &str) -> Result<Fetched, Failure> {
        Ok(Fetched::Modified {
            body: body.as_bytes().to_vec(),
            etag: Some(etag.into()),
        })
    }

    #[test]
    fn a_fresh_index_is_reused_and_a_stale_one_is_revalidated_with_its_etag() {
        let mut indexes = Indexes::new();
        let start = Instant::now();
        let load = |indexes: &mut Indexes<String>, at, answer: Result<Fetched, Failure>| {
            let sent = Cell::new(None);
            let fetched = Cell::new(false);
            let value = indexes.load(&source(), at, None, parse, |etag| {
                fetched.set(true);
                sent.set(etag.map(str::to_owned));
                answer
            });
            (value.unwrap(), fetched.get(), sent.take())
        };

        assert_eq!(
            load(&mut indexes, start, modified("one", "\"a\"")),
            ("one".into(), true, None)
        );
        assert_eq!(
            load(&mut indexes, start + FRESH / 2, modified("two", "\"b\"")),
            ("one".into(), false, None)
        );
        assert_eq!(
            load(&mut indexes, start + FRESH, Ok(Fetched::NotModified)),
            ("one".into(), true, Some("\"a\"".into()))
        );
        indexes.expire();
        assert_eq!(
            load(&mut indexes, start + FRESH, modified("two", "\"b\"")),
            ("two".into(), true, Some("\"a\"".into()))
        );
    }

    #[test]
    fn a_limited_or_failing_marketplace_serves_the_cached_index_until_retry_after() {
        let mut indexes = Indexes::new();
        let start = Instant::now();
        let first = indexes.load(&source(), start, None, parse, |_| {
            Err(Failure::new("offline"))
        });
        assert_eq!(first.unwrap_err(), "offline");

        indexes
            .load(&source(), start, None, parse, |_| modified("one", "\"a\""))
            .unwrap();
        let limited = start + FRESH;
        let served = indexes.load(&source(), limited, None, parse, |_| {
            Err(Failure {
                message: "429".into(),
                retry_after: Some(Duration::from_secs(120)),
            })
        });
        assert_eq!(served.unwrap(), "one");
        indexes.expire();
        let waiting = indexes.load(
            &source(),
            limited + Duration::from_secs(119),
            None,
            parse,
            |_| panic!("asked GitHub again before Retry-After"),
        );
        assert_eq!(waiting.unwrap(), "one");
        let after = indexes.load(
            &source(),
            limited + Duration::from_secs(120),
            None,
            parse,
            |_| modified("two", "\"b\""),
        );
        assert_eq!(after.unwrap(), "two");
        assert_eq!(retry_delay(Some(Duration::from_secs(86_400))), MAX_RETRY);
    }

    #[test]
    fn a_restart_reuses_the_saved_index_and_revalidates_it() {
        let directory = tempfile::tempdir().unwrap();
        let start = Instant::now();
        Indexes::new()
            .load(&source(), start, Some(directory.path()), parse, |_| {
                modified("one", "\"a\"")
            })
            .unwrap();

        let mut restarted = Indexes::new();
        let sent = Cell::new(None);
        let value = restarted.load(&source(), start, Some(directory.path()), parse, |etag| {
            sent.set(etag.map(str::to_owned));
            Ok(Fetched::NotModified)
        });
        assert_eq!(value.unwrap(), "one");
        assert_eq!(sent.take().as_deref(), Some("\"a\""));

        let offline = Indexes::new().load(&source(), start, Some(directory.path()), parse, |_| {
            Err(Failure::new("offline"))
        });
        assert_eq!(offline.unwrap(), "one");
    }

    #[test]
    fn pictures_are_kept_by_url_and_the_least_recently_used_go_first() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let url = |n: u8| {
            format!("https://github.com/a/b/releases/download/x%401.0.0/screenshot-{n}.png")
        };
        assert_eq!(read_picture(root, &url(1), 1024), None);
        write_picture_within(root, &url(1), &[1; 40], 100);
        write_picture_within(root, &url(2), &[2; 40], 100);
        let old = SystemTime::now() - Duration::from_secs(60);
        for n in [1, 2] {
            std::fs::File::options()
                .write(true)
                .open(picture_path(root, &url(n)))
                .unwrap()
                .set_modified(old + Duration::from_secs(u64::from(n)))
                .unwrap();
        }
        assert_eq!(read_picture(root, &url(1), 1024), Some(vec![1; 40]));
        assert_eq!(read_picture(root, &url(1), 39), None);
        write_picture_within(root, &url(3), &[3; 40], 100);
        assert_eq!(read_picture(root, &url(2), 1024), None);
        assert!(read_picture(root, &url(1), 1024).is_some());
        assert!(read_picture(root, &url(3), 1024).is_some());
    }
}
