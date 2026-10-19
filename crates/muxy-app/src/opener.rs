use std::ffi::OsString;
use std::io;
#[cfg(target_os = "macos")]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;

mod process;
#[cfg(windows)]
#[allow(unsafe_code, reason = "Shell calls that open and reveal files")]
mod shell;

use muxy_app_core::opener::{FileLocation, OpenContext, OpenRequest, Registry, Target};
use muxy_core::worker::WorkerPool;
use muxy_ui::tr;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests;

const FINDER: &str = "com.apple.finder";

/// The label key of the file manager, among the openers in Settings.
pub(crate) const FILE_MANAGER: &str = if cfg!(target_os = "macos") {
    "Finder"
} else {
    muxy_ui::l10n::tr_key!("File manager")
};

/// The menu item that shows a path in the file manager.
pub(crate) fn reveal_label() -> gpui::SharedString {
    if cfg!(target_os = "macos") {
        tr!("Reveal in Finder")
    } else if cfg!(windows) {
        tr!("Show in File Explorer")
    } else {
        tr!("Show in File Manager")
    }
}

/// The action that picks a folder in the system's own dialog.
pub(crate) fn browse_label() -> gpui::SharedString {
    if cfg!(target_os = "macos") {
        tr!("Finder…")
    } else {
        tr!("Browse…")
    }
}

pub(crate) fn submit<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> io::Result<async_channel::Receiver<T>> {
    static WORKERS: OnceLock<Result<WorkerPool, String>> = OnceLock::new();
    let pool = WORKERS
        .get_or_init(|| WorkerPool::new("resource-openers", 2, 32).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|error| io::Error::other(error.clone()))?;
    let (send, receive) = async_channel::bounded(1);
    pool.try_spawn(move || {
        if !send.is_closed() {
            let _ = send.try_send(work());
        }
    })?;
    Ok(receive)
}

pub(crate) fn resolve(text: &str, context: &OpenContext) -> Option<Target> {
    Target::web_url(text).or_else(|| {
        if context.server != muxy_app_core::ServerId::local() {
            return None;
        }
        let home = muxy_core::paths::home_dir().ok();
        Target::file(text, &context.directory, home.as_deref(), Path::exists)
    })
}

#[derive(Clone, Copy, Debug)]
enum Handler {
    Browser,
    Finder,
    System,
    Editor,
}

fn registry() -> &'static Registry<Handler> {
    static REGISTRY: OnceLock<Registry<Handler>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = Registry::default();
        registry.register(
            "system.browser",
            "Default Browser",
            |r| matches!(&r.target, Target::Url(url) if Target::web_url(url).is_some()),
            Handler::Browser,
        );
        for (id, title, handler) in [
            (
                "system.finder",
                if cfg!(target_os = "macos") {
                    "Finder"
                } else {
                    "File Manager"
                },
                Handler::Finder,
            ),
            ("system.application", "Default Application", Handler::System),
            ("system.editor", "Project Editor", Handler::Editor),
        ] {
            registry.register(
                id,
                title,
                |r| {
                    r.context.server == muxy_app_core::ServerId::local()
                        && matches!(r.target, Target::File(_))
                },
                handler,
            );
        }
        registry
    })
}

pub(crate) fn open(
    request: &OpenRequest,
    settings: &muxy_app_core::settings::OpenerSettings,
) -> io::Result<()> {
    open_with(request, settings, editors, launch)
}

fn open_with<'a>(
    request: &OpenRequest,
    settings: &muxy_app_core::settings::OpenerSettings,
    editors: impl FnOnce() -> &'a [Editor],
    mut launch: impl FnMut(&Launch) -> io::Result<()>,
) -> io::Result<()> {
    let (preferred, fallback) = match request.target {
        Target::Url(_) => (&settings.url, "system.browser"),
        Target::File(_) => (&settings.file, "system.editor"),
    };
    let handler = registry()
        .resolve(request, preferred, fallback)
        .ok_or_else(|| io::Error::other(tr!("No opener supports this resource").to_string()))?
        .handler;
    match (&request.target, handler) {
        (Target::Url(url), Handler::Browser) => launch(&Launch::system([OsString::from(url)])),
        (Target::File(file), Handler::Finder) => launch(&finder(&file.path)),
        (Target::File(file), Handler::System) => {
            launch(&Launch::system([file.path.clone().into_os_string()]))
        }
        (Target::File(file), Handler::Editor) => {
            let selected = settings.project_target.as_deref();
            if selected == Some(FINDER) {
                return launch(&finder(&file.path));
            }
            let editors = editors();
            let editor = selected
                .and_then(|id| editors.iter().find(|editor| editor.bundle == id))
                .or_else(|| editors.first());
            let command = editor.map_or_else(
                || finder(&file.path),
                |editor| editor.command(file, &request.context.project_directory),
            );
            launch(&command).or_else(|_| launch(&finder(&file.path)))
        }
        _ => Err(io::Error::other(
            tr!("Opener does not support this resource").to_string(),
        )),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Launch {
    program: PathBuf,
    args: Vec<OsString>,
    /// A file to select in its folder, where a file manager offers that. The
    /// program runs when none does.
    #[cfg(not(target_os = "macos"))]
    select: Option<PathBuf>,
}

impl Launch {
    /// Hands `args` to the system's handler for them. On Windows that is
    /// the shell itself, so there is no program.
    fn system(args: impl IntoIterator<Item = OsString>) -> Self {
        Self::program(
            if cfg!(target_os = "macos") {
                "/usr/bin/open"
            } else if cfg!(windows) {
                ""
            } else {
                "xdg-open"
            },
            args,
        )
    }

    fn program(program: impl Into<PathBuf>, args: impl IntoIterator<Item = OsString>) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().collect(),
            #[cfg(not(target_os = "macos"))]
            select: None,
        }
    }
}

fn launch(launch: &Launch) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    if let Some(file) = &launch.select
        && muxy_ui::file_manager::select(file).is_ok()
    {
        return Ok(());
    }
    #[cfg(windows)]
    if let Some(file) = &launch.select {
        return shell::select(file);
    }
    #[cfg(windows)]
    if launch.program.as_os_str().is_empty() {
        return launch
            .args
            .iter()
            .try_for_each(|target| shell::open(target));
    }
    process::run(
        Command::new(&launch.program)
            .args(&launch.args)
            .stdout(Stdio::null()),
        Duration::from_secs(10),
    )
    .map(drop)
    .map_err(|error| missing(&launch.program, error))
}

/// Names the program when it is what could not be found.
fn missing(program: &Path, error: io::Error) -> io::Error {
    if error.kind() != io::ErrorKind::NotFound {
        return error;
    }
    io::Error::new(
        io::ErrorKind::NotFound,
        tr!(
            "%@ is not installed, so there is nothing to open this with.",
            program.display().to_string()
        )
        .to_string(),
    )
}

/// Opens `url` with the system's handler for it, or with the app at `app`.
#[cfg(target_os = "macos")]
pub(crate) fn open_url(url: &str, app: Option<&Path>) -> io::Result<()> {
    let mut args = Vec::new();
    if let Some(app) = app {
        args.extend([OsString::from("-a"), app.as_os_str().to_owned()]);
    }
    args.push(OsString::from(url));
    launch(&Launch::system(args))
}

/// Opens `url` with the system's handler for it, or with the browser at `app`.
#[cfg(windows)]
pub(crate) fn open_url(url: &str, app: Option<&Path>) -> io::Result<()> {
    match app {
        // A browser keeps running, so it isn't waited for.
        Some(app) => Command::new(app)
            .arg(url)
            .spawn()
            .map(drop)
            .map_err(|error| missing(app, error)),
        None => launch(&Launch::system([url.into()])),
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn open_url(url: &str, app: Option<&Path>) -> io::Result<()> {
    match app {
        Some(app) => launch(&Launch::program(
            "gio",
            ["launch".into(), app.as_os_str().to_owned(), url.into()],
        )),
        None => launch(&Launch::system([url.into()])),
    }
}

#[derive(Debug)]
pub(crate) struct WebBrowser {
    pub name: String,
    pub path: PathBuf,
}

/// The browsers installed on this computer, by name.
pub(crate) fn web_browsers() -> &'static [WebBrowser] {
    static BROWSERS: OnceLock<Vec<WebBrowser>> = OnceLock::new();
    BROWSERS.get_or_init(|| {
        muxy_ui::browser::installed_browsers()
            .into_iter()
            .map(|(name, path)| WebBrowser { name, path })
            .collect()
    })
}

/// Opens a file with this computer's default app for its type.
pub(crate) fn open_with_default_app(path: &Path) -> io::Result<()> {
    launch(&Launch::system([path.as_os_str().to_owned()]))
}

/// Opens a text file in the text editor every Windows has, as macOS opens
/// one in its text editor. `.toml` files usually have no app of their own.
#[cfg(windows)]
pub(crate) fn edit_text(path: &Path) -> io::Result<()> {
    Command::new("notepad.exe")
        .arg(path)
        .spawn()
        .map(drop)
        .map_err(|error| missing(Path::new("notepad.exe"), error))
}

/// Shows a path in the file manager: a file selected in its folder, or a
/// folder opened. A path that doesn't exist yet shows its nearest existing
/// folder.
pub(crate) fn reveal(path: &Path) -> io::Result<()> {
    let existing = path.ancestors().find(|path| path.exists()).unwrap_or(path);
    launch(&finder(existing))
}

#[cfg(target_os = "macos")]
fn finder(path: &Path) -> Launch {
    if path.is_dir() {
        Launch::system([
            OsString::from("-a"),
            "Finder".into(),
            path.as_os_str().into(),
        ])
    } else {
        Launch::system([OsString::from("-R"), path.as_os_str().into()])
    }
}

#[derive(Debug)]
struct Editor {
    bundle: String,
    path: PathBuf,
    rank: usize,
}

impl Editor {
    #[cfg(target_os = "macos")]
    fn command(&self, file: &FileLocation, directory: &Path) -> Launch {
        let cli = match self.bundle.as_str() {
            "com.microsoft.VSCode" => Some(("Contents/Resources/app/bin/code", true)),
            "com.microsoft.VSCodeInsiders" => {
                Some(("Contents/Resources/app/bin/code-insiders", true))
            }
            "com.vscodium" => Some(("Contents/Resources/app/bin/codium", true)),
            "com.todesktop.230313mzl4w4u92" => Some(("Contents/Resources/app/bin/cursor", true)),
            "com.exafunction.windsurf" => Some(("Contents/Resources/app/bin/windsurf", true)),
            "dev.zed.Zed" => Some(("Contents/MacOS/cli", false)),
            _ => None,
        };
        if let Some((relative, goto)) = cli {
            let program = self.path.join(relative);
            if executable(&program) {
                let mut args = vec![directory.as_os_str().to_owned()];
                if file.path.is_dir() {
                    if file.path != directory {
                        args.push(file.path.as_os_str().to_owned());
                    }
                } else {
                    if goto {
                        args.push("--goto".into());
                    }
                    args.push(location_argument(file));
                }
                return Launch { program, args };
            }
        }
        let mut args = vec![
            OsString::from("-a"),
            self.path.clone().into_os_string(),
            directory.as_os_str().into(),
        ];
        if file.path != directory {
            args.push(file.path.clone().into_os_string());
        }
        Launch::system(args)
    }
}

fn location_argument(file: &FileLocation) -> OsString {
    let mut target = file.path.as_os_str().to_owned();
    target.push(format!(
        ":{}:{}",
        file.line.unwrap_or(1),
        file.column.unwrap_or(1)
    ));
    target
}

#[cfg(target_os = "macos")]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(target_os = "macos")]
fn editors() -> &'static [Editor] {
    static EDITORS: OnceLock<Vec<Editor>> = OnceLock::new();
    EDITORS.get_or_init(|| {
        let mut roots = vec![PathBuf::from("/Applications")];
        if let Ok(home) = muxy_core::paths::home_dir() {
            roots.push(home.join("Applications"));
        }
        let mut found = Vec::new();
        let mut remaining = 4096;
        let deadline = Instant::now() + Duration::from_secs(5);
        for root in roots {
            discover(&root, 0, &mut remaining, deadline, &mut found);
        }
        found.sort_by(|a, b| (a.rank, &a.bundle, &a.path).cmp(&(b.rank, &b.bundle, &b.path)));
        found.dedup_by(|a, b| a.bundle == b.bundle);
        found
    })
}

#[cfg(target_os = "macos")]
fn discover(
    root: &Path,
    depth: usize,
    remaining: &mut usize,
    deadline: Instant,
    found: &mut Vec<Editor>,
) {
    let Ok(entries) = root.read_dir() else { return };
    for entry in entries.flatten() {
        if *remaining == 0 || Instant::now() >= deadline {
            return;
        }
        *remaining -= 1;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "app") {
            let Some(bundle) =
                bundle_identifier(&path, deadline.saturating_duration_since(Instant::now()))
            else {
                continue;
            };
            if let Some(rank) = editor_rank(&bundle) {
                found.push(Editor { bundle, path, rank });
            }
        } else if depth < 3 && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            discover(&path, depth + 1, remaining, deadline, found);
        }
    }
}

#[cfg(target_os = "macos")]
fn bundle_identifier(path: &Path, remaining: Duration) -> Option<String> {
    let mut child = process::run(
        Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"])
            .arg(path.join("Contents/Info.plist"))
            .stdout(Stdio::piped()),
        remaining.min(Duration::from_millis(500)),
    )
    .ok()?;
    let mut bytes = Vec::new();
    child
        .stdout
        .take()?
        .take(1025)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1024 {
        return None;
    }
    String::from_utf8(bytes)
        .ok()
        .map(|bundle| bundle.trim().to_owned())
}

#[cfg(target_os = "macos")]
fn editor_rank(bundle: &str) -> Option<usize> {
    const EDITORS: &[&str] = &[
        "com.microsoft.VSCode",
        "com.microsoft.VSCodeInsiders",
        "com.vscodium",
        "com.todesktop.230313mzl4w4u92",
        "dev.zed.Zed",
        "com.exafunction.windsurf",
        "com.qoder.ide",
        "com.apple.dt.Xcode",
        "com.jetbrains.PhpStorm",
        "com.jetbrains.WebStorm",
        "com.jetbrains.PyCharm",
        "com.jetbrains.IntelliJ-IDEA",
        "com.jetbrains.CLion",
        "com.jetbrains.GoLand",
        "com.jetbrains.RubyMine",
        "com.jetbrains.DataGrip",
        "com.jetbrains.Rider",
        "com.jetbrainsFleet",
        "com.panic.Nova",
        "com.sublimetext.4",
        "com.barebones.bbedit",
        "com.macromates.TextMate",
        "org.gnu.Emacs",
        "org.aquamacs.Aquamacs",
        "com.code.athas",
    ];
    EDITORS.iter().position(|id| *id == bundle).or_else(|| {
        (bundle.starts_with("com.jetbrains.") && !bundle.to_lowercase().contains("toolbox"))
            .then_some(100)
    })
}

#[cfg(not(target_os = "macos"))]
fn finder(path: &Path) -> Launch {
    if path.is_dir() {
        return Launch::system([path.as_os_str().to_owned()]);
    }
    let folder = path.parent().unwrap_or(path);
    Launch {
        select: Some(path.to_owned()),
        ..Launch::system([folder.as_os_str().to_owned()])
    }
}

#[cfg(not(target_os = "macos"))]
impl Editor {
    fn command(&self, file: &FileLocation, directory: &Path) -> Launch {
        let mut args = vec![directory.as_os_str().to_owned()];
        if file.path.is_dir() {
            if file.path != directory {
                args.push(file.path.as_os_str().to_owned());
            }
        } else {
            if self.bundle != "dev.zed.Zed" {
                args.push("--goto".into());
            }
            args.push(location_argument(file));
        }
        Launch::program(self.path.clone(), args)
    }
}

#[cfg(not(target_os = "macos"))]
fn editors() -> &'static [Editor] {
    static EDITORS: OnceLock<Vec<Editor>> = OnceLock::new();
    EDITORS.get_or_init(|| {
        let paths = std::env::var_os("PATH").unwrap_or_default();
        let directories: Vec<_> = std::env::split_paths(&paths)
            .filter(|path| path.is_absolute())
            .collect();
        let mut editors: Vec<_> = [
            ("com.microsoft.VSCode", "code"),
            ("com.microsoft.VSCodeInsiders", "code-insiders"),
            ("com.vscodium", "codium"),
            ("com.todesktop.230313mzl4w4u92", "cursor"),
            ("dev.zed.Zed", "zed"),
            ("com.exafunction.windsurf", "windsurf"),
        ]
        .into_iter()
        .enumerate()
        .filter_map(|(rank, (bundle, command))| {
            let path = muxy_core::executable::find_program(&directories, command)?;
            Some(Editor {
                bundle: bundle.into(),
                path,
                rank,
            })
        })
        .collect();
        editors.sort_by_key(|editor| editor.rank);
        editors
    })
}
