use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
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

pub(crate) const FILE_MANAGER: &str = if cfg!(target_os = "macos") {
    "Finder"
} else {
    muxy_ui::l10n::tr_key!("File manager")
};

pub(crate) fn reveal_label() -> gpui::SharedString {
    muxy_ui::l10n::translate(reveal_key())
}

pub(crate) fn reveal_key() -> &'static str {
    if cfg!(target_os = "macos") {
        muxy_ui::l10n::tr_key!("Reveal in Finder")
    } else if cfg!(windows) {
        muxy_ui::l10n::tr_key!("Show in File Explorer")
    } else {
        muxy_ui::l10n::tr_key!("Show in File Manager")
    }
}

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

pub(crate) fn project_file(path: &Path, project: &Path) -> Option<String> {
    let path = path.canonicalize().ok()?;
    if !path.is_file() {
        return None;
    }
    let project = project.canonicalize().ok()?;
    path.strip_prefix(project).ok()?.to_str().map(str::to_owned)
}

#[derive(Clone, Copy, Debug)]
enum Handler {
    Browser,
    Finder,
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
        registry.register(
            "system.finder",
            FILE_MANAGER,
            |r| r.context.server.is_local() && matches!(r.target, Target::File(_)),
            Handler::Finder,
        );
        registry
    })
}

pub(crate) fn open(
    request: &OpenRequest,
    settings: &muxy_app_core::settings::OpenerSettings,
) -> io::Result<()> {
    open_with(request, settings, launch)
}

fn open_with(
    request: &OpenRequest,
    settings: &muxy_app_core::settings::OpenerSettings,
    mut launch: impl FnMut(&Launch) -> io::Result<()>,
) -> io::Result<()> {
    let (preferred, fallback) = match request.target {
        Target::Url(_) => (settings.url.as_str(), "system.browser"),
        Target::File(_) => ("system.finder", "system.finder"),
    };
    let handler = registry()
        .resolve(request, preferred, fallback)
        .ok_or_else(|| io::Error::other(tr!("No opener supports this resource").to_string()))?
        .handler;
    match (&request.target, handler) {
        (Target::Url(url), Handler::Browser) => launch(&Launch::system([OsString::from(url)])),
        (Target::File(file), Handler::Finder) => launch(&finder(&file.path)),
        _ => Err(io::Error::other(
            tr!("Opener does not support this resource").to_string(),
        )),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Launch {
    program: PathBuf,
    args: Vec<OsString>,
    #[cfg(not(target_os = "macos"))]
    select: Option<PathBuf>,
}

impl Launch {
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

    #[cfg(not(windows))]
    fn text_editor(path: &Path) -> Self {
        let mut args = Vec::new();
        if cfg!(target_os = "macos") {
            args.push(OsString::from("-t"));
        }
        args.push(path.as_os_str().to_owned());
        Self::system(args)
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
    process::launch(
        Command::new(&launch.program).args(&launch.args),
        Duration::from_secs(10),
    )
    .map_err(|error| missing(&launch.program, error))
}

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

#[cfg(target_os = "macos")]
pub(crate) fn open_url(url: &str, app: Option<&Path>) -> io::Result<()> {
    let mut args = Vec::new();
    if let Some(app) = app {
        args.extend([OsString::from("-a"), app.as_os_str().to_owned()]);
    }
    args.push(OsString::from(url));
    launch(&Launch::system(args))
}

#[cfg(windows)]
pub(crate) fn open_url(url: &str, app: Option<&Path>) -> io::Result<()> {
    match app {
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

pub(crate) fn web_browsers() -> &'static [WebBrowser] {
    static BROWSERS: OnceLock<Vec<WebBrowser>> = OnceLock::new();
    BROWSERS.get_or_init(|| {
        muxy_ui::browser::installed_browsers()
            .into_iter()
            .map(|(name, path)| WebBrowser { name, path })
            .collect()
    })
}

pub(crate) fn open_with_default_app(path: &Path) -> io::Result<()> {
    launch(&Launch::system([path.as_os_str().to_owned()]))
}

#[cfg(not(windows))]
pub(crate) fn open_in_text_editor(path: &Path) -> io::Result<()> {
    launch(&Launch::text_editor(path))
}

#[cfg(windows)]
pub(crate) fn open_in_text_editor(path: &Path) -> io::Result<()> {
    Command::new("notepad.exe")
        .arg(path)
        .spawn()
        .map(drop)
        .map_err(|error| missing(Path::new("notepad.exe"), error))
}

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
    name: String,
    path: PathBuf,
    rank: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectApp {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

impl AsRef<ProjectApp> for ProjectApp {
    fn as_ref(&self) -> &ProjectApp {
        self
    }
}

impl ProjectApp {
    pub(crate) fn is_file_manager(&self) -> bool {
        self.id == FINDER
    }
}

#[cfg(not(test))]
pub(crate) fn project_apps() -> Vec<ProjectApp> {
    project_apps_from(editors())
}

fn project_apps_from(editors: &[Editor]) -> Vec<ProjectApp> {
    let file_manager = ProjectApp {
        id: FINDER.into(),
        name: if cfg!(target_os = "macos") {
            "Finder".into()
        } else {
            tr!("File Manager").to_string()
        },
        path: PathBuf::from("/System/Library/CoreServices/Finder.app"),
    };
    std::iter::once(file_manager)
        .chain(editors.iter().map(|editor| ProjectApp {
            id: editor.bundle.clone(),
            name: editor.name.clone(),
            path: editor.path.clone(),
        }))
        .collect()
}

pub(crate) fn default_project_app<'a, T: AsRef<ProjectApp>>(
    apps: &'a [T],
    chosen: Option<&str>,
) -> Option<&'a T> {
    chosen
        .and_then(|id| apps.iter().find(|app| app.as_ref().id == id))
        .or_else(|| apps.iter().find(|app| !app.as_ref().is_file_manager()))
        .or_else(|| apps.first())
}

pub(crate) fn open_project(directory: &Path, app: &str) -> io::Result<()> {
    open_project_with(directory, app, editors(), launch)
}

fn open_project_with(
    directory: &Path,
    app: &str,
    editors: &[Editor],
    launch: impl FnOnce(&Launch) -> io::Result<()>,
) -> io::Result<()> {
    if app == FINDER {
        return launch(&finder(directory));
    }
    let editor = editors
        .iter()
        .find(|editor| editor.bundle == app)
        .ok_or_else(|| io::Error::other(tr!("This app is no longer installed").to_string()))?;
    let folder = FileLocation {
        path: directory.to_owned(),
        line: None,
        column: None,
    };
    launch(&editor.command(&folder, directory))
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
                let name = path.file_stem().map_or_else(
                    || bundle.clone(),
                    |stem| stem.to_string_lossy().into_owned(),
                );
                found.push(Editor {
                    bundle,
                    name,
                    path,
                    rank,
                });
            }
        } else if depth < 3 && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            discover(&path, depth + 1, remaining, deadline, found);
        }
    }
}

#[cfg(target_os = "macos")]
fn bundle_identifier(path: &Path, remaining: Duration) -> Option<String> {
    let output = muxy_core::process::run(
        Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"])
            .arg(path.join("Contents/Info.plist")),
        muxy_core::process::Limits::new(remaining.min(Duration::from_millis(500)), 1024),
    )
    .ok()?;
    if !output.status.success() || output.timed_out || output.stdout.truncated {
        return None;
    }
    String::from_utf8(output.stdout.bytes)
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
        "com.google.android.studio",
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
            ("com.microsoft.VSCode", "code", "Visual Studio Code"),
            (
                "com.microsoft.VSCodeInsiders",
                "code-insiders",
                "Visual Studio Code - Insiders",
            ),
            ("com.vscodium", "codium", "VSCodium"),
            ("com.todesktop.230313mzl4w4u92", "cursor", "Cursor"),
            ("dev.zed.Zed", "zed", "Zed"),
            ("com.exafunction.windsurf", "windsurf", "Windsurf"),
        ]
        .into_iter()
        .enumerate()
        .filter_map(|(rank, (bundle, command, name))| {
            let path = muxy_core::executable::find_program(&directories, command)?;
            Some(Editor {
                bundle: bundle.into(),
                name: name.into(),
                path,
                rank,
            })
        })
        .collect();
        editors.sort_by_key(|editor| editor.rank);
        editors
    })
}
