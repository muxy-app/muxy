//! `OpenCode`'s shared service (`opencode serve --service`) and the tabs its
//! terminal UIs keep, both under its state directory.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Weak;
use std::time::{Duration, SystemTime};

use serde_json::Value;

use super::{Pane, Report, Tab, http};
use crate::activity::Activity;

const INTERVAL: Duration = Duration::from_millis(1500);
/// The least time between polls, so a burst of terminal changes costs one.
const SETTLE: Duration = Duration::from_millis(200);
const MAX_FILE: u64 = 4 * 1024 * 1024;
/// Parents looked up at most once per session, and forgotten past this many.
const MAX_PARENTS: usize = 4096;

fn state_directory() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/state")))?;
    Some(base.join("opencode"))
}

fn read_json(path: &Path) -> Option<Value> {
    use std::io::Read;
    let mut text = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_FILE)
        .read_to_end(&mut text)
        .ok()?;
    serde_json::from_slice(&text).ok()
}

struct Service {
    address: SocketAddr,
    authorization: String,
}

impl Service {
    fn read(directory: &Path) -> Option<Self> {
        let service = read_json(&directory.join("service.json"))?;
        let address = http::loopback(service["url"].as_str()?)?;
        let password = service["password"].as_str()?;
        Some(Self {
            address,
            authorization: muxy_core::base64::encode(format!("opencode:{password}").as_bytes()),
        })
    }

    fn get(&self, path: &str) -> Option<Value> {
        let body = http::get(self.address, path, &self.authorization)?;
        let mut reply: Value = serde_json::from_slice(&body).ok()?;
        Some(reply["data"].take())
    }
}

/// The `tabs.json` written last, as `OpenCode` keeps one per release channel
/// such as `latest/tui/tabs.json`.
fn tabs_file(directory: &Path) -> Option<PathBuf> {
    std::fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("tui/tabs.json"))
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _): &(SystemTime, PathBuf)| *modified)
        .map(|(_, path)| path)
}

pub(super) fn parse_tabs(tabs: &Value, directories: &HashSet<String>) -> HashMap<String, Vec<Tab>> {
    let Some(scopes) = tabs["cwd"].as_object() else {
        return HashMap::new();
    };
    scopes
        .iter()
        .filter(|(directory, _)| {
            directories
                .iter()
                .any(|known| super::same_directory(known, directory))
        })
        .map(|(directory, scope)| {
            let tabs = scope["tabs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|tab| {
                    Some(Tab {
                        id: tab["sessionID"].as_str()?.to_owned(),
                        title: tab["title"].as_str().unwrap_or_default().to_owned(),
                    })
                })
                .collect();
            (directory.clone(), tabs)
        })
        .collect()
}

/// The session `id` belongs to at the top, through its parents.
fn root<'a>(id: &'a str, parents: &'a HashMap<String, Option<String>>) -> &'a str {
    let mut current = id;
    for _ in 0..32 {
        match parents.get(current) {
            Some(Some(parent)) => current = parent,
            _ => break,
        }
    }
    current
}

struct Poller {
    parents: HashMap<String, Option<String>>,
}

impl Poller {
    fn report(&mut self, panes: &BTreeMap<muxy_protocol::SessionId, Pane>) -> Option<Report> {
        let directory = state_directory()?;
        let service = Service::read(&directory)?;
        let directories: HashSet<String> =
            panes.values().map(|pane| pane.directory.clone()).collect();
        let mut tabs = parse_tabs(&read_json(&tabs_file(&directory)?)?, &directories);
        for (directory, tabs) in &mut tabs {
            let listed = service.get(&format!(
                "/api/session?directory={}",
                http::query(directory)
            ));
            for session in listed
                .as_ref()
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(id) = session["id"].as_str() else {
                    continue;
                };
                let parent = session["parentID"].as_str().map(str::to_owned);
                self.parents.insert(id.to_owned(), parent);
                if let Some(tab) = tabs.iter_mut().find(|tab| tab.id == id)
                    && let Some(title) = session["title"].as_str()
                {
                    title.clone_into(&mut tab.title);
                }
            }
        }
        let active = service.get("/api/session/active")?;
        let active: Vec<&str> = active
            .as_object()
            .into_iter()
            .flat_map(|sessions| sessions.keys().map(String::as_str))
            .collect();
        for &id in &active {
            let mut current = id.to_owned();
            for _ in 0..32 {
                if !self.parents.contains_key(&current) {
                    let parent = service
                        .get(&format!("/api/session/{}", http::query(&current)))
                        .and_then(|session| session["parentID"].as_str().map(str::to_owned));
                    if self.parents.len() >= MAX_PARENTS {
                        self.parents.clear();
                    }
                    self.parents.insert(current.clone(), parent);
                }
                match self.parents.get(&current) {
                    Some(Some(parent)) => current.clone_from(parent),
                    _ => break,
                }
            }
        }
        let busy = active
            .iter()
            .map(|id| root(id, &self.parents).to_owned())
            .collect();
        Some(Report { tabs, busy })
    }
}

/// Polls `OpenCode` while any terminal runs it, until `activity` is dropped:
/// every `INTERVAL`, or soon after a terminal starts, stops or switches tabs.
/// A failed poll keeps the last report, so tabs don't blink out.
pub(crate) fn poll(activity: &Weak<Activity>) {
    let mut poller = Poller {
        parents: HashMap::new(),
    };
    loop {
        let Some(shared) = activity.upgrade() else {
            return;
        };
        let panes = shared.wait_for_harness(INTERVAL);
        if panes.is_empty() {
            shared.set_harness_report(Report::default());
        } else if let Some(report) = poller.report(&panes) {
            shared.set_harness_report(report);
        }
        drop(shared);
        std::thread::sleep(SETTLE);
    }
}
