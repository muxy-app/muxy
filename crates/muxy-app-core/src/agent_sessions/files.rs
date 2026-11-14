use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use muxy_protocol::AgentProvider;

use super::{Session, normalized, parse};

const MAX_ENTRIES: usize = 20_000;
const PROVIDER_READ_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Roots {
    sources: Vec<(AgentProvider, PathBuf, usize)>,
}

impl Roots {
    pub fn from_home(home: &Path) -> Self {
        Self {
            sources: vec![
                (AgentProvider::Claude, home.join(".claude/projects"), 1),
                (AgentProvider::Codex, home.join(".codex/sessions"), 3),
                (
                    AgentProvider::Copilot,
                    home.join(".copilot/session-state"),
                    1,
                ),
                (AgentProvider::Droid, home.join(".factory/sessions"), 1),
                (AgentProvider::Pi, home.join(".pi/agent/sessions"), 1),
                (AgentProvider::OhMyPi, home.join(".omp/agent/sessions"), 1),
                (AgentProvider::Xal, home.join(".xal/sessions"), 1),
            ],
        }
    }

    pub fn from_environment() -> Option<Self> {
        let mut roots = Self::from_home(&std::env::home_dir()?);
        for (provider, root, _) in &mut roots.sources {
            let override_path = match provider {
                AgentProvider::Claude => std::env::var_os("CLAUDE_CONFIG_DIR")
                    .map(|path| PathBuf::from(path).join("projects")),
                AgentProvider::Codex => {
                    std::env::var_os("CODEX_HOME").map(|path| PathBuf::from(path).join("sessions"))
                }
                _ => None,
            };
            if let Some(path) = override_path.filter(|path| path.is_absolute()) {
                *root = path;
            }
        }
        Some(roots)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Listing {
    pub sessions: Vec<Session>,
    pub warnings: Vec<(AgentProvider, String)>,
    pub limited: bool,
    pub pending: bool,
}

#[derive(Debug)]
struct Cached {
    provider: AgentProvider,
    modified: SystemTime,
    length: u64,
    session: Option<Session>,
}

#[derive(Debug, Default)]
pub struct Catalog {
    files: HashMap<PathBuf, Cached>,
}

impl Catalog {
    pub fn scan(&mut self, roots: &Roots, project: &Path) -> Listing {
        let project = normalized(project);
        let mut listing = Listing::default();
        let mut seen = HashSet::new();
        for (provider, root, depth) in &roots.sources {
            let mut read_budget = PROVIDER_READ_BYTES;
            let mut paths = Vec::new();
            let mut budget = MAX_ENTRIES;
            if let Err(error) = discover(root, *depth, &mut budget, &mut paths) {
                listing.warnings.push((*provider, error.to_string()));
            }
            listing.limited |= budget == 0;
            for path in paths {
                let Ok(metadata) = fs::symlink_metadata(&path) else {
                    continue;
                };
                if !metadata.is_file() {
                    continue;
                }
                let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let length = metadata.len();
                seen.insert(path.clone());
                let cached = self.files.get(&path).filter(|cached| {
                    cached.provider == *provider
                        && cached.modified == modified
                        && cached.length == length
                });
                if cached.is_none() && read_budget < parse::read_limit(length) {
                    listing.pending = true;
                } else if cached.is_none() {
                    match parse::read(&path, *provider, modified, &mut read_budget) {
                        Ok(session) => {
                            self.files.insert(
                                path.clone(),
                                Cached {
                                    provider: *provider,
                                    modified,
                                    length,
                                    session,
                                },
                            );
                        }
                        Err(error) => {
                            self.files.remove(&path);
                            if !listing.warnings.iter().any(|(id, _)| id == provider) {
                                listing.warnings.push((*provider, error.to_string()));
                            }
                        }
                    }
                }
                if let Some(session) = self
                    .files
                    .get(&path)
                    .filter(|cached| cached.provider == *provider)
                    .and_then(|cached| cached.session.as_ref())
                    && session.directory == project
                {
                    listing.sessions.push(session.clone());
                }
            }
        }
        self.files.retain(|path, _| seen.contains(path));
        listing.sessions.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| a.path.cmp(&b.path))
        });
        let mut identities = HashSet::new();
        listing
            .sessions
            .retain(|session| identities.insert((session.provider.name(), session.id.clone())));
        listing
    }
}

fn discover(
    path: &Path,
    depth: usize,
    budget: &mut usize,
    paths: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut directories = Vec::new();
    for entry in entries {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() && depth > 0 {
            directories.push(entry.path());
        } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "jsonl") {
            paths.push(entry.path());
        }
    }
    directories.sort();
    for directory in directories {
        if *budget == 0 {
            break;
        }
        discover(&directory, depth - 1, budget, paths)?;
    }
    Ok(())
}
