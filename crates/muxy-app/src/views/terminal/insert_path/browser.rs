//! Which folder Insert Path browses, what it lists there, and the path a
//! chosen entry is typed as. Paths are those of the terminal's computer,
//! which may be of another kind than this one.

/// The most rows the list shows; a filter brings the others within reach.
const ROW_LIMIT: usize = 500;

/// A file or folder directly inside the folder being browsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) is_directory: bool,
}

/// An entry the list shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Row {
    pub(super) name: String,
    pub(super) is_directory: bool,
    /// It, or something inside it, was chosen recently.
    pub(super) recent: bool,
}

/// A chosen entry's path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Chosen {
    /// Where the entry is on the terminal's computer.
    pub(super) absolute: String,
    /// What is typed at the cursor: relative inside the terminal's folder,
    /// and ending in a separator for a folder.
    pub(super) typed: String,
}

#[derive(Clone, Debug)]
pub(super) struct Browser {
    /// The terminal's folder, which typed paths are relative to.
    base: Option<String>,
    /// The Home folder, which `~` stands for.
    home: Option<String>,
    /// The folder being browsed.
    directory: String,
    /// Paths chosen before, most recent first.
    recents: Vec<String>,
    /// Whether paths are Windows paths, which also take `\` between names.
    windows: bool,
}

impl Browser {
    /// Starts in `base`, the terminal's folder, or in `home` when the
    /// terminal's folder isn't known. `None` when neither is.
    pub(super) fn new(
        base: Option<String>,
        home: Option<String>,
        recents: Vec<String>,
    ) -> Option<Self> {
        let start = base.clone().or_else(|| home.clone())?;
        let windows = is_windows(&start);
        let clean = |path: String| trimmed(&path, windows).to_owned();
        Some(Self {
            directory: clean(start),
            base: base.map(clean),
            home: home.map(clean),
            recents,
            windows,
        })
    }

    /// Takes `base` as the terminal's folder from now on, which may have
    /// changed since browsing started.
    pub(super) fn rebase(&mut self, base: Option<String>) {
        self.base = base.map(|path| trimmed(&path, self.windows).to_owned());
    }

    pub(super) fn directory(&self) -> &str {
        &self.directory
    }

    /// The browsed folder as the filter field names it: with `~` for Home,
    /// and ending in a separator.
    pub(super) fn label(&self) -> String {
        let separator = self.separator();
        let mut label = match &self.home {
            Some(home) if *home == self.directory => "~".to_owned(),
            Some(home) => match inside(home, &self.directory, self.windows) {
                Some(rest) => format!("~{separator}{rest}"),
                None => self.directory.clone(),
            },
            None => self.directory.clone(),
        };
        if !label.ends_with(self.separators()) {
            label.push(separator);
        }
        label
    }

    /// Browses the folder above. Returns whether there is one.
    pub(super) fn leave(&mut self) -> bool {
        match parent(&self.directory, self.windows) {
            Some(parent) => {
                self.directory = parent;
                true
            }
            None => false,
        }
    }

    /// Browses the folder `name` inside the browsed one.
    pub(super) fn enter(&mut self, name: &str) {
        self.directory = join(&self.directory, name, self.windows);
    }

    /// Follows the folders `query` names, such as `docs/`, `../`, `~/` or
    /// `/usr/`, and returns what is left of it to filter by. `None` when it
    /// names no folder, so all of it is a filter. `entries` are the browsed
    /// folder's, whose spelling a name typed in another case takes.
    pub(super) fn follow(&mut self, query: &str, entries: &[Entry]) -> Option<String> {
        let separators = self.separators();
        let mut directory = self.directory.clone();
        let mut listed = Some(entries);
        let mut rest = query;
        if let (Some(home), Some(after)) = (&self.home, query.strip_prefix('~'))
            && after.starts_with(separators)
        {
            directory.clone_from(home);
            rest = &after[1..];
            listed = None;
        } else {
            match self.start(query) {
                Start::Root(root, named) => {
                    directory = root;
                    rest = &query[named..];
                    listed = None;
                }
                Start::Here if query.contains(separators) => {}
                Start::Here | Start::Unfinished => return None,
            }
        }
        while let Some(index) = rest.find(separators) {
            let name = &rest[..index];
            rest = &rest[index + 1..];
            match name {
                "" | "." => continue,
                ".." => {
                    if let Some(parent) = parent(&directory, self.windows) {
                        directory = parent;
                    }
                }
                name => {
                    let name = listed
                        .and_then(|entries| spelling(entries, name))
                        .unwrap_or(name);
                    directory = join(&directory, name, self.windows);
                }
            }
            listed = None;
        }
        self.directory = directory;
        Some(rest.to_owned())
    }

    /// The root `query` starts from, when it names one.
    fn start(&self, query: &str) -> Start {
        if !self.windows {
            return if query.starts_with('/') {
                Start::Root("/".to_owned(), 1)
            } else {
                Start::Here
            };
        }
        let separators = self.separators();
        if query.starts_with(r"\\") || query.starts_with("//") {
            let mut names = query[2..].splitn(3, separators);
            return match (names.next(), names.next(), names.next()) {
                (Some(host), Some(share), Some(_)) if !host.is_empty() && !share.is_empty() => {
                    Start::Root(format!(r"\\{host}\{share}"), host.len() + share.len() + 4)
                }
                _ => Start::Unfinished,
            };
        }
        if let Some(drive) = drive_root(query) {
            return Start::Root(format!(r"{}\", &drive[..2]), drive.len());
        }
        if query.starts_with(separators) {
            let root = root_length(&self.directory, true);
            return Start::Root(self.directory[..root].to_owned(), 1);
        }
        Start::Here
    }

    /// The entries the list shows for `filter`: those whose name holds it,
    /// ignoring case, with recent ones first, then names that start with it.
    /// Names that start with a dot only show for a filter that does too.
    pub(super) fn rows(&self, entries: &[Entry], filter: &str) -> Vec<Row> {
        let needle = filter.to_lowercase();
        let hidden = filter.starts_with('.');
        let mut rows: Vec<_> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| hidden || !entry.name.starts_with('.'))
            .filter_map(|(index, entry)| {
                let name = entry.name.to_lowercase();
                if !name.contains(&needle) {
                    return None;
                }
                let recency = self.recency(&join(&self.directory, &entry.name, self.windows));
                Some((
                    (
                        recency.unwrap_or(usize::MAX),
                        !name.starts_with(&needle),
                        index,
                    ),
                    Row {
                        name: entry.name.clone(),
                        is_directory: entry.is_directory,
                        recent: recency.is_some(),
                    },
                ))
            })
            .collect();
        rows.sort_by_key(|(order, _)| *order);
        rows.truncate(ROW_LIMIT);
        rows.into_iter().map(|(_, row)| row).collect()
    }

    /// Whether the browsed folder has entries that only a filter starting
    /// with a dot shows.
    pub(super) fn hides(entries: &[Entry]) -> bool {
        entries.iter().any(|entry| entry.name.starts_with('.'))
    }

    /// The path of the entry `name` in the browsed folder.
    pub(super) fn chosen(&self, name: &str, is_directory: bool) -> Chosen {
        let absolute = join(&self.directory, name, self.windows);
        let mut typed = self
            .base
            .as_deref()
            .and_then(|base| inside(base, &absolute, self.windows))
            .unwrap_or(&absolute)
            .to_owned();
        if is_directory {
            typed.push(self.separator());
        }
        Chosen { absolute, typed }
    }

    /// How recently `path`, or something inside it, was chosen: 0 for the
    /// latest choice.
    fn recency(&self, path: &str) -> Option<usize> {
        self.recents
            .iter()
            .position(|recent| recent == path || inside(path, recent, self.windows).is_some())
    }

    fn separator(&self) -> char {
        if self.windows { '\\' } else { '/' }
    }

    fn separators(&self) -> &'static [char] {
        separators(self.windows)
    }
}

/// Where a typed path starts.
enum Start {
    /// In the browsed folder.
    Here,
    /// At a root, named by this many of the path's bytes.
    Root(String, usize),
    /// At a share whose name isn't typed out yet.
    Unfinished,
}

fn separators(windows: bool) -> &'static [char] {
    if windows { &['\\', '/'] } else { &['/'] }
}

/// A drive path such as `C:\a` or `C:/a`, or a UNC path such as `\\host\a`.
fn is_windows(path: &str) -> bool {
    match path.as_bytes() {
        [drive, b':', b'\\' | b'/', ..] => drive.is_ascii_alphabetic(),
        [b'\\', b'\\', ..] => true,
        _ => false,
    }
}

/// The start of `path` that names a drive's root, such as `C:\` or `C:/`.
fn drive_root(path: &str) -> Option<&str> {
    match path.as_bytes() {
        [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic() => path.get(..3),
        _ => None,
    }
}

/// How many bytes of `path` name its root, which has no folder above it:
/// `/`, a drive such as `C:\`, or a share such as `\\host\share`.
fn root_length(path: &str, windows: bool) -> usize {
    if !windows {
        return usize::from(path.starts_with('/'));
    }
    if let Some(share) = path.strip_prefix(r"\\") {
        let mut names = share.splitn(3, separators(true));
        let host = names.next().map_or(0, str::len);
        let name = names.next().map_or(0, |name| name.len() + 1);
        return 2 + host + name;
    }
    drive_root(path).map_or(0, str::len)
}

/// `path` without separators at its end, which a root keeps.
fn trimmed(path: &str, windows: bool) -> &str {
    let root = root_length(path, windows);
    let end = path.trim_end_matches(separators(windows)).len();
    &path[..end.max(root)]
}

fn join(directory: &str, name: &str, windows: bool) -> String {
    if directory.ends_with(separators(windows)) {
        format!("{directory}{name}")
    } else if windows {
        format!("{directory}\\{name}")
    } else {
        format!("{directory}/{name}")
    }
}

/// The folder `path` is in; `None` for a root.
fn parent(path: &str, windows: bool) -> Option<String> {
    let root = root_length(path, windows);
    if path.len() <= root {
        return None;
    }
    let cut = path[root..]
        .rfind(separators(windows))
        .map_or(0, |index| root + index);
    Some(trimmed(&path[..cut.max(root)], windows).to_owned())
}

/// What `path` is called from `directory`, when it is inside it.
fn inside<'a>(directory: &str, path: &'a str, windows: bool) -> Option<&'a str> {
    let rest = path.strip_prefix(directory)?;
    let rest = if directory.ends_with(separators(windows)) {
        rest
    } else {
        rest.strip_prefix(separators(windows))?
    };
    (!rest.is_empty()).then_some(rest)
}

/// How the folder among `entries` that `name` means is spelled: `name`
/// itself, or the folder that differs from it only in case.
fn spelling<'a>(entries: &'a [Entry], name: &str) -> Option<&'a str> {
    let folders = || entries.iter().filter(|entry| entry.is_directory);
    let lower = name.to_lowercase();
    folders()
        .find(|entry| entry.name == name)
        .or_else(|| folders().find(|entry| entry.name.to_lowercase() == lower))
        .map(|entry| entry.name.as_str())
}

#[cfg(test)]
mod tests;
