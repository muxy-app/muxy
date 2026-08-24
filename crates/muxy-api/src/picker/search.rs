use muxy_core::fold::fold;
use notify::{RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_INDEXED_DIRECTORIES: usize = 50_000;
const MAX_VISITED_ENTRIES: usize = 250_000;
const MAX_SCAN_DURATION: Duration = Duration::from_secs(3);
const MAX_RESULT_LIMIT: usize = 100;
const MAX_CACHED_ROOTS: usize = 2;

const SKIPPED_NAMES: [&str; 12] = [
    ".build",
    ".cache",
    ".gradle",
    ".next",
    "build",
    "carthage",
    "deriveddata",
    "dist",
    "node_modules",
    "pods",
    "target",
    "vendor",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub name: String,
    pub path: String,
    pub display_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub results: Vec<SearchResult>,
    pub read_failed: bool,
    pub is_truncated: bool,
    pub has_more_results: bool,
}

#[derive(Debug, Clone)]
struct Entry {
    name: String,
    path: String,
    folded_name: String,
    folded_search_path: String,
}

#[derive(Debug, Default)]
struct Index {
    entries: Vec<Entry>,
    is_truncated: bool,
}

#[derive(Clone)]
pub struct SearchService {
    state: Arc<Mutex<ServiceState>>,
}

struct ServiceState {
    indexes: HashMap<String, Arc<Index>>,
    cache_order: Vec<String>,
    watchers: HashMap<String, notify::RecommendedWatcher>,
    home_directory: String,
}

impl Default for SearchService {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ServiceState {
                indexes: HashMap::new(),
                cache_order: Vec::new(),
                watchers: HashMap::new(),
                home_directory: standardized_absolute_path(
                    &muxy_core::prefs::home_dir().to_string_lossy(),
                    "",
                )
                .unwrap_or_default(),
            })),
        }
    }

    pub fn prepare(&self, root_path: &str) {
        let mut guard = self.state.lock().expect("folder search service");
        let Some(root) = standardized_absolute_path(root_path, &guard.home_directory) else {
            return;
        };
        let _ = guard.prepared_index(&self.state, &root);
    }

    pub fn search(
        &self,
        query: &str,
        root_path: &str,
        existing_project_paths: &[String],
        limit: usize,
    ) -> Snapshot {
        let mut guard = self.state.lock().expect("folder search service");
        let Some(root) = standardized_absolute_path(root_path, &guard.home_directory) else {
            return Snapshot {
                read_failed: true,
                ..Default::default()
            };
        };

        let home = guard.home_directory.clone();
        let index = match guard.prepared_index(&self.state, &root) {
            Ok(index) => index,
            Err(()) => {
                return Snapshot {
                    read_failed: true,
                    ..Default::default()
                };
            }
        };
        drop(guard);

        run_search(query, &root, existing_project_paths, &index, &home, limit)
    }
}

impl ServiceState {
    fn prepared_index(
        &mut self,
        shared: &Arc<Mutex<ServiceState>>,
        root: &str,
    ) -> Result<Arc<Index>, ()> {
        match root_state(root) {
            RootState::Invalid | RootState::Unreadable => {
                self.indexes.remove(root);
                self.watchers.remove(root);
                self.cache_order.retain(|cached| cached != root);
                return Err(());
            }
            RootState::Ready => {}
        }

        if let Some(index) = self.indexes.get(root).cloned() {
            self.touch(root);
            return Ok(index);
        }

        let index = Arc::new(normalized_index(scan(root, &self.home_directory), root));
        self.indexes.insert(root.to_owned(), index.clone());
        self.touch(root);
        self.ensure_watcher(shared, root);

        while self.cache_order.len() > MAX_CACHED_ROOTS {
            let evicted = self.cache_order.remove(0);
            self.indexes.remove(&evicted);
            self.watchers.remove(&evicted);
        }

        Ok(index)
    }

    fn touch(&mut self, root: &str) {
        self.cache_order.retain(|cached| cached != root);
        self.cache_order.push(root.to_owned());
    }

    fn ensure_watcher(&mut self, shared: &Arc<Mutex<ServiceState>>, root: &str) {
        if self.watchers.contains_key(root) {
            return;
        }
        let watched = root.to_owned();
        let state = Arc::downgrade(shared);
        let Ok(mut watcher) =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else { return };
                let touches_directory = event
                    .paths
                    .iter()
                    .any(|path| path.to_str() == Some(watched.as_str()) || !path.is_file());
                if !touches_directory {
                    return;
                }
                let Some(state) = state.upgrade() else { return };
                let Ok(mut guard) = state.lock() else { return };
                guard.indexes.remove(&watched);
            })
        else {
            return;
        };
        if watcher
            .watch(Path::new(root), RecursiveMode::Recursive)
            .is_ok()
        {
            self.watchers.insert(root.to_owned(), watcher);
        }
    }
}

enum RootState {
    Ready,
    Unreadable,
    Invalid,
}

fn root_state(path: &str) -> RootState {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            if std::fs::read_dir(path).is_ok() {
                RootState::Ready
            } else {
                RootState::Unreadable
            }
        }
        Ok(_) => RootState::Invalid,
        Err(_) => RootState::Invalid,
    }
}

fn scan(root: &str, home: &str) -> Index {
    let started = Instant::now();
    let home_library = format!("{home}/Library");

    let mut entries = Vec::new();
    let mut is_truncated = false;
    let mut visited = 0usize;
    let mut queue = std::collections::VecDeque::from([root.to_owned()]);
    entries.push(make_entry(root, root));

    while let Some(directory) = queue.pop_front() {
        let Ok(read) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in read.flatten() {
            visited += 1;
            if visited > MAX_VISITED_ENTRIES || started.elapsed() > MAX_SCAN_DURATION {
                is_truncated = true;
                return Index {
                    entries,
                    is_truncated,
                };
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let is_directory = if file_type.is_symlink() {
                std::fs::metadata(entry.path())
                    .map(|metadata| metadata.is_dir())
                    .unwrap_or(false)
            } else {
                file_type.is_dir()
            };
            if !is_directory {
                continue;
            }
            if entries.len() >= MAX_INDEXED_DIRECTORIES {
                is_truncated = true;
                return Index {
                    entries,
                    is_truncated,
                };
            }

            let path = entry.path();
            let path_string = path.to_string_lossy().into_owned();
            let name = entry.file_name().to_string_lossy().into_owned();
            entries.push(make_entry(&path_string, root));

            let skip = name.starts_with('.')
                || file_type.is_symlink()
                || SKIPPED_NAMES.contains(&name.to_lowercase().as_str())
                || is_package(&name)
                || (root == home && path_string == home_library);
            if !skip {
                queue.push_back(path_string);
            }
        }
    }

    Index {
        entries,
        is_truncated,
    }
}

fn is_package(name: &str) -> bool {
    matches!(
        name.rsplit_once('.').map(|(_, extension)| extension),
        Some("app" | "bundle" | "framework" | "xcodeproj" | "xcworkspace" | "photoslibrary")
    )
}

fn make_entry(path: &str, root: &str) -> Entry {
    let name = name_for(path);
    Entry {
        folded_name: fold(&name),
        folded_search_path: folded_search_path(path, root),
        name,
        path: path.to_owned(),
    }
}

fn normalized_index(index: Index, root: &str) -> Index {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    let mut is_truncated = index.is_truncated;

    for entry in index.entries {
        let Some(path) = standardized_absolute_path(&entry.path, "") else {
            continue;
        };
        if !is_inside(&path, root) || !seen.insert(path.clone()) {
            continue;
        }
        if entries.len() >= MAX_INDEXED_DIRECTORIES {
            is_truncated = true;
            break;
        }
        entries.push(make_entry(&path, root));
    }

    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Index {
        entries,
        is_truncated,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MatchKind {
    Exact,
    Prefix,
    Substring,
    Context,
}

impl MatchKind {
    fn rank(self) -> usize {
        match self {
            Self::Exact => 0,
            Self::Prefix => 1,
            Self::Substring => 2,
            Self::Context => 3,
        }
    }

    fn resolve(folded_name: &str, term: &str) -> Option<Self> {
        if folded_name == term {
            Some(Self::Exact)
        } else if folded_name.starts_with(term) {
            Some(Self::Prefix)
        } else if folded_name.contains(term) {
            Some(Self::Substring)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Match {
    kind: MatchKind,
    path_score: usize,
}

impl PartialOrd for Match {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Match {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.kind
            .cmp(&other.kind)
            .then(self.path_score.cmp(&other.path_score))
    }
}

struct Term {
    value: String,
    exact_component: String,
    component_prefix: String,
}

impl Term {
    fn new(value: String) -> Self {
        Self {
            exact_component: format!("/{value}/"),
            component_prefix: format!("/{value}"),
            value,
        }
    }
}

fn parse_query(raw: &str) -> Option<Vec<Term>> {
    let mut seen = HashSet::new();
    let terms: Vec<Term> = raw
        .split(|character: char| character.is_whitespace() || character == '/')
        .filter_map(|value| {
            let folded = fold(value);
            if folded.is_empty() || !seen.insert(folded.clone()) {
                return None;
            }
            Some(Term::new(folded))
        })
        .collect();
    (!terms.is_empty()).then_some(terms)
}

fn resolve_match(folded_name: &str, folded_search_path: &str, terms: &[Term]) -> Option<Match> {
    if terms.len() == 1 {
        let kind = MatchKind::resolve(folded_name, &terms[0].value)?;
        return Some(Match {
            kind,
            path_score: kind.rank(),
        });
    }

    let mut path_score = 0;
    for term in terms {
        if folded_search_path.contains(&term.exact_component) {
            continue;
        }
        if folded_search_path.contains(&term.component_prefix) {
            path_score += MatchKind::Prefix.rank();
            continue;
        }
        if !folded_search_path.contains(&term.value) {
            return None;
        }
        path_score += MatchKind::Substring.rank();
    }

    let kind = terms
        .iter()
        .filter_map(|term| MatchKind::resolve(folded_name, &term.value))
        .min()
        .unwrap_or(MatchKind::Context);
    Some(Match { kind, path_score })
}

struct Candidate {
    name: String,
    path: String,
    matched: Match,
    is_existing_project: bool,
    depth: usize,
}

fn precedes(left: &Candidate, right: &Candidate) -> std::cmp::Ordering {
    left.matched
        .cmp(&right.matched)
        .then_with(|| right.is_existing_project.cmp(&left.is_existing_project))
        .then_with(|| left.depth.cmp(&right.depth))
        .then_with(|| super::path_service::natural_compare(&left.name, &right.name))
        .then_with(|| super::path_service::natural_compare(&left.path, &right.path))
}

fn run_search(
    query: &str,
    root: &str,
    existing_project_paths: &[String],
    index: &Index,
    home: &str,
    limit: usize,
) -> Snapshot {
    let limit = limit.min(MAX_RESULT_LIMIT);
    let Some(terms) = parse_query(query) else {
        return Snapshot {
            is_truncated: index.is_truncated,
            ..Default::default()
        };
    };

    let existing: HashSet<String> = existing_project_paths
        .iter()
        .take(MAX_INDEXED_DIRECTORIES)
        .filter_map(|path| standardized_absolute_path(path, home))
        .filter(|path| is_inside(path, root))
        .collect();

    let mut candidates: HashMap<String, Candidate> = HashMap::new();

    for path in &existing {
        let name = name_for(path);
        let Some(matched) = resolve_match(&fold(&name), &folded_search_path(path, root), &terms)
        else {
            continue;
        };
        candidates.insert(
            path.clone(),
            Candidate {
                name,
                path: path.clone(),
                matched,
                is_existing_project: true,
                depth: depth_below(path, root),
            },
        );
    }

    for entry in &index.entries {
        let Some(matched) = resolve_match(&entry.folded_name, &entry.folded_search_path, &terms)
        else {
            continue;
        };
        match candidates.get_mut(&entry.path) {
            Some(existing_candidate) => {
                existing_candidate.matched = existing_candidate.matched.min(matched);
            }
            None => {
                if candidates.len() >= MAX_INDEXED_DIRECTORIES {
                    break;
                }
                candidates.insert(
                    entry.path.clone(),
                    Candidate {
                        name: entry.name.clone(),
                        path: entry.path.clone(),
                        matched,
                        is_existing_project: existing.contains(&entry.path),
                        depth: depth_below(&entry.path, root),
                    },
                );
            }
        }
    }

    let mut ranked: Vec<Candidate> = candidates.into_values().collect();
    ranked.sort_by(precedes);
    let has_more_results = ranked.len() > limit;

    Snapshot {
        results: ranked
            .into_iter()
            .take(limit)
            .map(|candidate| SearchResult {
                display_path: display_path(&candidate.path, home),
                name: candidate.name,
                path: candidate.path,
            })
            .collect(),
        read_failed: false,
        is_truncated: index.is_truncated,
        has_more_results,
    }
}

pub fn standardized_absolute_path(path: &str, home_directory: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let expanded = if trimmed == "~" {
        home_directory.to_owned()
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        format!("{home_directory}/{rest}")
    } else {
        trimmed.to_owned()
    };
    if !expanded.starts_with('/') {
        return None;
    }
    Some(super::path_service::standardize(&expanded))
}

pub fn name_for(path: &str) -> String {
    if path == "/" {
        return "/".to_owned();
    }
    super::path_service::last_component(path)
}

fn is_inside(path: &str, root: &str) -> bool {
    if root == "/" {
        return path.starts_with('/');
    }
    path == root || path.starts_with(&format!("{root}/"))
}

fn depth_below(path: &str, root: &str) -> usize {
    let components = |value: &str| value.split('/').filter(|part| !part.is_empty()).count();
    components(path).saturating_sub(components(root))
}

fn folded_search_path(path: &str, root: &str) -> String {
    let relative = if path == root {
        name_for(path)
    } else if root == "/" {
        path.trim_start_matches('/').to_owned()
    } else {
        path.get(root.len() + 1..).unwrap_or_default().to_owned()
    };
    format!("/{}/", fold(&relative))
}

pub fn display_path(path: &str, home: &str) -> String {
    let display = if path == home {
        "~".to_owned()
    } else if let Some(rest) = path.strip_prefix(&format!("{home}/")) {
        format!("~/{rest}")
    } else {
        path.to_owned()
    };
    if display.ends_with('/') {
        display
    } else {
        format!("{display}/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(paths: &[&str], root: &str) -> Index {
        Index {
            entries: paths.iter().map(|path| make_entry(path, root)).collect(),
            is_truncated: false,
        }
    }

    #[test]
    fn ranks_exact_then_prefix_then_substring() {
        let root = "/Users/alice";
        let index = index(
            &[
                "/Users/alice/work/premuxy",
                "/Users/alice/code/muxy-native",
                "/Users/alice/code/muxy",
            ],
            root,
        );

        let snapshot = run_search("muxy", root, &[], &index, root, MAX_RESULT_LIMIT);
        let names: Vec<&str> = snapshot
            .results
            .iter()
            .map(|result| result.name.as_str())
            .collect();
        assert_eq!(names, ["muxy", "muxy-native", "premuxy"]);
    }

    #[test]
    fn multi_term_queries_match_across_the_path() {
        let root = "/Users/alice";
        let index = index(
            &["/Users/alice/code/muxy", "/Users/alice/archive/muxy"],
            root,
        );

        let snapshot = run_search("code muxy", root, &[], &index, root, MAX_RESULT_LIMIT);
        assert_eq!(snapshot.results.len(), 1);
        assert_eq!(snapshot.results[0].path, "/Users/alice/code/muxy");
    }

    #[test]
    fn existing_projects_win_ties_and_report_display_paths() {
        let root = "/Users/alice";
        let index = index(&["/Users/alice/a/muxy", "/Users/alice/b/muxy"], root);

        let snapshot = run_search(
            "muxy",
            root,
            &["/Users/alice/b/muxy".to_owned()],
            &index,
            root,
            MAX_RESULT_LIMIT,
        );
        assert_eq!(snapshot.results[0].path, "/Users/alice/b/muxy");
        assert_eq!(snapshot.results[0].display_path, "~/b/muxy/");
    }

    #[test]
    fn blank_queries_return_nothing() {
        let root = "/Users/alice";
        let snapshot = run_search(
            "   ",
            root,
            &[],
            &index(&["/Users/alice/muxy"], root),
            root,
            MAX_RESULT_LIMIT,
        );
        assert!(snapshot.results.is_empty());
    }
}
