const ROW_LIMIT: usize = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) is_directory: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Row {
    pub(super) name: String,
    pub(super) is_directory: bool,
    pub(super) recent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Chosen {
    pub(super) absolute: String,
    pub(super) typed: String,
}

#[derive(Clone, Debug)]
pub(super) struct Browser {
    base: Option<String>,
    home: Option<String>,
    directory: String,
    recents: Vec<String>,
    windows: bool,
}

impl Browser {
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

    pub(super) fn rebase(&mut self, base: Option<String>) {
        self.base = base.map(|path| trimmed(&path, self.windows).to_owned());
    }

    pub(super) fn directory(&self) -> &str {
        &self.directory
    }

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

    pub(super) fn leave(&mut self) -> bool {
        match parent(&self.directory, self.windows) {
            Some(parent) => {
                self.directory = parent;
                true
            }
            None => false,
        }
    }

    pub(super) fn enter(&mut self, name: &str) {
        self.directory = join(&self.directory, name, self.windows);
    }

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

    pub(super) fn hides(entries: &[Entry]) -> bool {
        entries.iter().any(|entry| entry.name.starts_with('.'))
    }

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

enum Start {
    Here,
    Root(String, usize),
    Unfinished,
}

fn separators(windows: bool) -> &'static [char] {
    if windows { &['\\', '/'] } else { &['/'] }
}

fn is_windows(path: &str) -> bool {
    match path.as_bytes() {
        [drive, b':', b'\\' | b'/', ..] => drive.is_ascii_alphabetic(),
        [b'\\', b'\\', ..] => true,
        _ => false,
    }
}

fn drive_root(path: &str) -> Option<&str> {
    match path.as_bytes() {
        [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic() => path.get(..3),
        _ => None,
    }
}

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

fn inside<'a>(directory: &str, path: &'a str, windows: bool) -> Option<&'a str> {
    let rest = path.strip_prefix(directory)?;
    let rest = if directory.ends_with(separators(windows)) {
        rest
    } else {
        rest.strip_prefix(separators(windows))?
    };
    (!rest.is_empty()).then_some(rest)
}

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
