use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use muxy_protocol::AgentProvider;
use serde_json::Value;

use super::{Report, Tab, TabSource, http};

const MAX_FILE: u64 = 4 * 1024 * 1024;
const MAX_PARENTS: usize = 4096;
const LEADER: u8 = 0x18;
const REPORT_BUDGET: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(super) struct OpenCode {
    parents: HashMap<String, Option<String>>,
}

impl TabSource for OpenCode {
    fn provider(&self) -> AgentProvider {
        AgentProvider::OpenCode
    }

    fn report(&mut self, directories: &HashSet<String>) -> Option<Report> {
        let directory = state_directory()?;
        let service = Service::read(&directory, Instant::now() + REPORT_BUDGET)?;
        let mut tabs = read_tabs(&directory, directories);
        let active = service.get("/api/session/active")?;
        let busy = active
            .as_object()
            .into_iter()
            .flat_map(|sessions| sessions.keys())
            .map(|id| self.root(&service, id))
            .collect();
        let mut viewed = HashMap::new();
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
                self.remember(id, session["parentID"].as_str());
                if let Some(time) = session["time"]["viewed"].as_u64() {
                    viewed.insert(id.to_owned(), time);
                }
                if let Some(tab) = tabs.iter_mut().find(|tab| tab.id == id)
                    && let Some(title) = session["title"].as_str()
                {
                    title.clone_into(&mut tab.title);
                }
            }
        }
        Some(Report { tabs, busy, viewed })
    }

    fn shown_title<'a>(&self, title: &'a str) -> Option<(&'a str, bool)> {
        let title = title.trim().strip_prefix("OC | ")?;
        Some(match title.strip_suffix('…') {
            Some(prefix) => (prefix, true),
            None => (title, false),
        })
    }

    fn switch_input(&self, index: usize) -> Option<Vec<u8>> {
        let digit = u8::try_from((index + 1) % 10).ok().filter(|_| index < 10)?;
        Some(vec![LEADER, b'0' + digit])
    }

    fn resume_command(&self, id: &str) -> Option<String> {
        safe_id(id).then(|| format!("opencode -s {id}"))
    }
}

impl OpenCode {
    fn remember(&mut self, id: &str, parent: Option<&str>) {
        if !self.parents.contains_key(id) && self.parents.len() >= MAX_PARENTS {
            self.parents.clear();
        }
        self.parents
            .insert(id.to_owned(), parent.map(str::to_owned));
    }

    fn root(&mut self, service: &Service, id: &str) -> String {
        let mut current = id.to_owned();
        for _ in 0..32 {
            if !self.parents.contains_key(&current)
                && let Some(session) =
                    service.get(&format!("/api/session/{}", http::query(&current)))
            {
                self.remember(&current, session["parentID"].as_str());
            }
            match self.parents.get(&current) {
                Some(Some(parent)) => current.clone_from(parent),
                _ => break,
            }
        }
        current
    }
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

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
    deadline: Instant,
}

impl Service {
    fn read(directory: &Path, deadline: Instant) -> Option<Self> {
        let service = read_json(&directory.join("service.json"))?;
        let address = http::loopback(service["url"].as_str()?)?;
        let password = service["password"].as_str()?;
        Some(Self {
            address,
            authorization: muxy_core::base64::encode(format!("opencode:{password}").as_bytes()),
            deadline,
        })
    }

    fn get(&self, path: &str) -> Option<Value> {
        let body = http::get(self.address, path, &self.authorization, self.deadline)?;
        let mut reply: Value = serde_json::from_slice(&body).ok()?;
        Some(reply["data"].take())
    }
}

pub(super) fn read_tabs(
    directory: &Path,
    directories: &HashSet<String>,
) -> HashMap<String, Vec<Tab>> {
    let mut files: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("tui/tabs.json"))
        .filter_map(|path| Some((std::fs::metadata(&path).ok()?.modified().ok()?, path)))
        .collect();
    files.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let mut tabs: HashMap<String, Vec<Tab>> = HashMap::new();
    for (_, path) in files {
        let Some(file) = read_json(&path) else {
            continue;
        };
        for (directory, found) in parse_tabs(&file, directories) {
            if !tabs
                .keys()
                .any(|known| super::same_directory(known, &directory))
            {
                tabs.insert(directory, found);
            }
        }
    }
    tabs
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
