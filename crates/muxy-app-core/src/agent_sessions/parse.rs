use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use muxy_protocol::AgentProvider;
use serde_json::Value;

use super::{Session, normalized, valid_id};

const PREFIX_BYTES: u64 = 1024 * 1024;
const TAIL_BYTES: u64 = 64 * 1024;

pub(super) fn read_limit(length: u64) -> u64 {
    length.min(PREFIX_BYTES + TAIL_BYTES)
}

#[derive(Default)]
struct Metadata {
    id: Option<String>,
    directory: Option<PathBuf>,
    title: Option<String>,
    prompt: Option<String>,
    child: bool,
}

pub(super) fn read(
    path: &Path,
    provider: AgentProvider,
    modified: SystemTime,
    budget: &mut u64,
) -> io::Result<Option<Session>> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Ok(None);
    }
    let length = file.metadata()?.len();
    let mut reader = BufReader::new(file.take(PREFIX_BYTES.min(*budget)));
    let mut metadata = Metadata::default();
    let mut line = Vec::new();
    let mut consumed = 0;
    while reader.read_until(b'\n', &mut line)? > 0 {
        consumed += line.len() as u64;
        *budget = budget.saturating_sub(line.len() as u64);
        if line.last() == Some(&b'\n')
            && let Ok(value) = serde_json::from_slice(&line)
        {
            metadata.visit(provider, &value, false);
        }
        line.clear();
    }
    if length > consumed {
        let mut file = reader.into_inner().into_inner();
        let start = consumed.max(length.saturating_sub(TAIL_BYTES));
        file.seek(SeekFrom::Start(start))?;
        let mut tail = BufReader::new(file.take(TAIL_BYTES.min(*budget)));
        if start > consumed {
            let read = tail.read_until(b'\n', &mut line)?;
            *budget = budget.saturating_sub(read as u64);
            line.clear();
        }
        while tail.read_until(b'\n', &mut line)? > 0 {
            *budget = budget.saturating_sub(line.len() as u64);
            if line.last() == Some(&b'\n')
                && let Ok(value) = serde_json::from_slice(&line)
            {
                metadata.visit(provider, &value, true);
            }
            line.clear();
        }
    }
    let Some(id) = metadata.id.filter(|id| valid_id(id)) else {
        return Ok(None);
    };
    let Some(directory) = metadata.directory.filter(|path| path.is_absolute()) else {
        return Ok(None);
    };
    if metadata.child {
        return Ok(None);
    }
    Ok(Some(Session {
        provider,
        title: metadata
            .title
            .or(metadata.prompt)
            .unwrap_or_else(|| id.clone()),
        id,
        directory: normalized(&directory),
        path: path.to_path_buf(),
        modified,
    }))
}

impl Metadata {
    fn identity(&mut self, value: &Value, id: &str, cwd: &str) {
        if self.id.is_none() {
            self.id = value[id].as_str().map(str::to_owned);
        }
        if self.directory.is_none() {
            self.directory = value[cwd].as_str().map(PathBuf::from);
        }
    }

    fn prompt(&mut self, value: &Value) {
        if self.prompt.is_none() {
            self.prompt = text(value);
        }
    }

    fn visit(&mut self, provider: AgentProvider, value: &Value, tail: bool) {
        let kind = value["type"].as_str().unwrap_or_default();
        match provider {
            AgentProvider::Claude => {
                if !tail && matches!(kind, "user" | "assistant") {
                    self.identity(value, "sessionId", "cwd");
                    self.child |= value["isSidechain"] == true;
                }
                if kind == "custom-title" {
                    self.title = text(&value["customTitle"]);
                } else if kind == "summary" && self.title.is_none() {
                    self.title = text(&value["summary"]);
                } else if !tail && kind == "user" && value["isMeta"] != true {
                    self.prompt(&value["message"]["content"]);
                }
            }
            AgentProvider::Codex => {
                let payload = &value["payload"];
                if !tail && kind == "session_meta" {
                    self.identity(payload, "id", "cwd");
                    self.child |= payload["source"].get("subagent").is_some();
                } else if !tail && kind == "event_msg" && payload["type"] == "user_message" {
                    self.prompt(&payload["message"]);
                }
            }
            AgentProvider::Pi | AgentProvider::OhMyPi | AgentProvider::Droid => {
                if !tail && matches!(kind, "session" | "session_start") {
                    self.identity(value, "id", "cwd");
                    if self.title.is_none() {
                        self.title = text(&value["title"]);
                    }
                } else if matches!(kind, "session_info" | "title" | "title_change") {
                    self.title = text(&value["title"]).or_else(|| text(&value["name"]));
                } else if !tail && kind == "message" && value["message"]["role"] == "user" {
                    self.prompt(&value["message"]["content"]);
                }
            }
            AgentProvider::Copilot => {
                let data = &value["data"];
                if !tail && kind == "session.start" {
                    self.id = data["sessionId"].as_str().map(str::to_owned);
                    self.directory = data["context"]["cwd"].as_str().map(PathBuf::from);
                } else if kind == "session.title_changed" {
                    self.title = text(&data["title"]);
                } else if !tail && kind == "user.message" && value.get("agentId").is_none() {
                    self.prompt(&data["content"]);
                }
            }
            AgentProvider::Xal => {
                if !tail && kind == "meta" {
                    self.identity(&value["meta"], "id", "cwd");
                } else if kind == "event" && value["event"]["type"] == "session_title_changed" {
                    self.title = text(&value["event"]["title"]);
                } else if !tail {
                    for key in ["item", "event"] {
                        if value[key]["type"] == "user_message" {
                            self.prompt(&value[key]["text"]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn text(value: &Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        let title: String = value
            .chars()
            .take(240)
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        return (!title.trim().is_empty()).then(|| title.trim().to_owned());
    }
    value.as_array()?.iter().find_map(|part| {
        matches!(part["type"].as_str(), Some("text" | "input_text"))
            .then(|| text(&part["text"]))
            .flatten()
    })
}
