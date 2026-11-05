//! Publishing an unpacked extension: checking what the store requires, then
//! opening a pull request against the store's repository from the user's fork.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use muxy_app_core::extensions::Extension;
use muxy_ui::tr;
use serde_json::{Value, json};

use super::authoring::{STORE_REPOSITORY, Tools, copy_tree};
use super::marketplace;

/// The store on github.com, whatever host `GH_HOST` points `gh` at.
const STORE_ON_GITHUB: &str = "github.com/muxy-app/extensions";
const GITHUB_HOST: &str = "github.com";
const ICON_MIN_SIZE: u32 = 256;
const ICON_MAX_SVG_BYTES: u64 = 512 * 1024;
const ICON_MAX_PNG_BYTES: u64 = 1024 * 1024;
const SCREENSHOT_MAX_BYTES: u64 = 3 * 1024 * 1024;
const SCREENSHOT_MAX_COUNT: usize = 6;
const MAX_PUBLISHED_BYTES: u64 = 64 * 1024 * 1024;
const PUSH_ATTEMPTS: usize = 6;
const BRANCH_ATTEMPTS: usize = 9;
const FILES_IN_CONFIRMATION: usize = 12;
/// Never sent from the package's top level: build output and its logs.
const LEFT_OUT_AT_ROOT: &[&str] = &["dist", "logs", "release"];
/// Never sent from anywhere, compared in lowercase: dependencies, each agent's
/// folder and instructions, and files that hold credentials.
const LEFT_OUT_NAMES: &[&str] = &[
    "node_modules",
    ".claude",
    ".agents",
    ".muxy",
    ".codex",
    ".cursor",
    ".opencode",
    ".factory",
    ".kiro",
    ".copilot",
    ".grok",
    ".pi",
    ".omp",
    ".xal",
    ".antigravity",
    ".gemini",
    "agents.md",
    "claude.md",
    ".ds_store",
    ".env",
    ".envrc",
    ".npmrc",
    ".netrc",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
];
const LEFT_OUT_ENDINGS: &[&str] = &[
    ".log",
    ".env",
    ".key",
    ".pem",
    ".p12",
    ".pfx",
    ".ppk",
    ".jks",
    ".keystore",
];

/// The package as it stands, and what stops it being published.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Readiness {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) permissions: Vec<String>,
    pub(crate) problems: Vec<String>,
    pub(crate) files: Vec<PathBuf>,
}

/// Who `gh` is signed in as.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitHub {
    Missing,
    SignedOut,
    /// It couldn't tell, such as when offline.
    Unreachable(String),
    User(String),
}

/// The store's published copy of an extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Published {
    pub(crate) version: String,
    pub(crate) author: Option<String>,
}

/// The pull request a publish opened or updated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequest {
    pub(crate) url: String,
    pub(crate) branch: String,
    pub(crate) updated: bool,
}

/// Why a publish stopped.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PublishFailure {
    NotReady(Vec<String>),
    /// `gh` is missing or signed out, so an agent can guide the user.
    NeedsGitHub(String),
    Declined,
    Failed(String),
}

impl PublishFailure {
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::NotReady(problems) => not_ready_message(problems),
            Self::NeedsGitHub(reason) | Self::Failed(reason) => reason.clone(),
            Self::Declined => tr!("The user declined publishing in Muxy.").to_string(),
        }
    }
}

fn not_ready_message(problems: &[String]) -> String {
    let mut lines = vec![tr!("Fix these before publishing:").to_string()];
    lines.extend(problems.iter().map(|problem| format!("• {problem}")));
    lines.join("\n")
}

/// A publish that only needs the user's approval.
#[derive(Debug)]
pub(crate) struct PublishPlan {
    pub(crate) readiness: Readiness,
    pub(crate) login: String,
    pub(crate) update: bool,
    /// Who published this name already, when that isn't the user.
    pub(crate) other_author: Option<String>,
    /// A private copy of the files, made before the user is asked, so what they
    /// approve is exactly what gets sent.
    pub(crate) staged: tempfile::TempDir,
}

/// Checks everything that can be checked without changing anything. The store
/// is only asked about a package that is otherwise ready, so a name it might
/// reject never reaches a request.
pub(crate) fn plan(tools: &dyn Tools, source: &Path) -> Result<PublishPlan, PublishFailure> {
    let mut readiness = check(source);
    if !readiness.problems.is_empty() {
        return Err(PublishFailure::NotReady(readiness.problems));
    }
    let login = match github_account(tools) {
        GitHub::User(login) => login,
        GitHub::Missing => {
            return Err(PublishFailure::NeedsGitHub(
                tr!("The GitHub CLI (gh) isn't installed.").to_string(),
            ));
        }
        GitHub::SignedOut => {
            return Err(PublishFailure::NeedsGitHub(
                tr!("The GitHub CLI (gh) isn't signed in. Run gh auth login.").to_string(),
            ));
        }
        GitHub::Unreachable(reason) => return Err(PublishFailure::Failed(reason)),
    };
    let published = published_version(tools, &readiness.name).map_err(PublishFailure::Failed)?;
    check_against_store(&mut readiness, published.as_ref());
    if !readiness.problems.is_empty() {
        return Err(PublishFailure::NotReady(readiness.problems));
    }
    let staged = stage(source, &readiness.files).map_err(PublishFailure::Failed)?;
    Ok(PublishPlan {
        staged,
        readiness,
        update: published.is_some(),
        other_author: published
            .and_then(|published| published.author)
            .filter(|author| !author.eq_ignore_ascii_case(&login)),
        login,
    })
}

impl PublishPlan {
    /// What the user approves: where it goes, under whose name, and what is sent.
    pub(crate) fn confirmation(&self) -> String {
        let readiness = &self.readiness;
        let shown = readiness.files.len().min(FILES_IN_CONFIRMATION);
        let mut files: Vec<String> = readiness.files[..shown]
            .iter()
            .map(|file| file.display().to_string())
            .collect();
        if readiness.files.len() > shown {
            files.push(tr!("and %lld more", readiness.files.len() - shown).to_string());
        }
        let mut lines = vec![
            tr!(
                "This opens a public pull request against %@ from your GitHub account, %@. Maintainers review it, and once merged it is listed for everyone.",
                STORE_REPOSITORY,
                &self.login
            )
            .to_string(),
            String::new(),
            tr!("Fork: your fork of %@, created if needed", STORE_REPOSITORY).to_string(),
            tr!(
                "Branch: %@, or a numbered one if your fork already has it",
                branch_name(&readiness.name, &readiness.version)
            )
            .to_string(),
            tr!(
                "Pull request: %@",
                pull_request_title(&readiness.name, &readiness.version)
            )
            .to_string(),
            tr!("An open pull request for this version is updated instead, replacing its earlier commits.").to_string(),
        ];
        if let Some(author) = &self.other_author {
            lines.push(String::new());
            lines.push(
                tr!(
                    "%@ is already published by %@. Your pull request would change their extension.",
                    &readiness.name,
                    author
                )
                .to_string(),
            );
        }
        lines.push(String::new());
        lines.push(tr!("Files:").to_string());
        lines.extend(files);
        lines.join("\n")
    }
}

pub(crate) fn branch_name(name: &str, version: &str) -> String {
    format!("{name}-{version}")
}

pub(crate) fn pull_request_title(name: &str, version: &str) -> String {
    format!("{name} {version}")
}

/// Checks a package's source folder against the store's rules. CI repeats
/// these, so this only saves the user a failed pull request.
pub(crate) fn check(source: &Path) -> Readiness {
    let mut readiness = Readiness::default();
    let manifest = source.join("package.json");
    let value = match fs::read_to_string(&manifest)
        .map_err(|error| error.to_string())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|error| error.to_string()))
    {
        Ok(value) => value,
        Err(error) => {
            readiness
                .problems
                .push(tr!("package.json can't be read: %@", error).to_string());
            return readiness;
        }
    };
    value["name"]
        .as_str()
        .unwrap_or_default()
        .clone_into(&mut readiness.name);
    value["version"]
        .as_str()
        .unwrap_or_default()
        .clone_into(&mut readiness.version);
    if !publishable_name(&readiness.name) {
        readiness.problems.push(
            tr!("The package name must start with a-z or 0-9, and may only use a-z, 0-9, dots and dashes.")
                .to_string(),
        );
    }
    if !marketplace::is_valid_version(&readiness.version) {
        readiness
            .problems
            .push(tr!("package.json needs a semantic version, such as 1.0.0.").to_string());
    }
    if value["scripts"]["build"].as_str().is_none_or(str::is_empty) {
        readiness.problems.push(
            tr!("package.json needs a scripts.build that produces dist/ with package.json inside.")
                .to_string(),
        );
    }
    if let Err(error) = Extension::load(source) {
        readiness.problems.push(error);
    }
    value["muxy"]["description"]
        .as_str()
        .unwrap_or_default()
        .clone_into(&mut readiness.description);
    readiness.permissions = value["muxy"]["permissions"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    readiness.permissions.sort();
    if !regular_file(&source.join("README.md")) {
        readiness.problems.push(
            tr!("Add a README.md that says what it does and why it needs each permission.")
                .to_string(),
        );
    }
    if !regular_file(&source.join("package-lock.json")) {
        readiness
            .problems
            .push(tr!("Add a package-lock.json. Run npm install to make one.").to_string());
    }
    check_listing(
        source,
        &value["muxy"]["marketplace"],
        &mut readiness.problems,
    );
    match files_to_publish(source) {
        Ok((files, bytes)) => {
            readiness.files = files;
            if bytes > MAX_PUBLISHED_BYTES {
                readiness.problems.push(
                    tr!("The source is over 64 MB. Move generated files out of the folder.")
                        .to_string(),
                );
            }
        }
        Err(error) => readiness.problems.push(error.to_string()),
    }
    readiness
}

/// Whether `path` is a file itself. A link to one is left out of what is sent,
/// so it doesn't count.
fn regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
}

fn publishable_name(name: &str) -> bool {
    name.len() <= 100
        && name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte))
}

fn check_listing(source: &Path, listing: &Value, problems: &mut Vec<String>) {
    if !listing.is_object() {
        problems.push(
            tr!("Add a marketplace block to muxy in package.json, with an icon and at least one screenshot.")
                .to_string(),
        );
        return;
    }
    match listing["icon"].as_str() {
        Some(icon) => check_icon(source, icon, problems),
        None => problems.push(
            tr!("marketplace.icon is required (SVG, or square PNG of at least 256×256).")
                .to_string(),
        ),
    }
    let screenshots = listing["screenshots"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    if screenshots.is_empty() {
        problems.push(
            tr!("Add at least one marketplace.screenshots entry (PNG, 16:10 or 16:9).").to_string(),
        );
    }
    if screenshots.len() > SCREENSHOT_MAX_COUNT {
        problems.push(tr!("The store takes at most 6 screenshots.").to_string());
    }
    for screenshot in screenshots {
        match screenshot.as_str() {
            Some(screenshot) => check_screenshot(source, screenshot, problems),
            None => problems
                .push(tr!("Each marketplace.screenshots entry must be a file path.").to_string()),
        }
    }
}

/// Listing files are named relative to the build output, which may not have
/// been built yet, so a file that a build would copy there counts too.
fn listing_asset(source: &Path, relative: &str) -> Option<PathBuf> {
    if Path::new(relative)
        .components()
        .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    ["dist", "", "public", "src"]
        .iter()
        .map(|folder| source.join(folder).join(relative))
        .find(|path| path.is_file())
}

fn png_size(path: &Path) -> Option<(u32, u32)> {
    let bytes = fs::read(path).ok()?;
    let width = bytes.get(16..20)?;
    let height = bytes.get(20..24)?;
    if bytes.get(..8)? != b"\x89PNG\r\n\x1a\n" || bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    Some((
        u32::from_be_bytes(width.try_into().ok()?),
        u32::from_be_bytes(height.try_into().ok()?),
    ))
}

fn size_of(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}

fn has_extension(file: &str, extension: &str) -> bool {
    Path::new(file)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

fn check_icon(source: &Path, icon: &str, problems: &mut Vec<String>) {
    let Some(path) = listing_asset(source, icon) else {
        problems.push(tr!("The marketplace icon %@ wasn't found.", icon).to_string());
        return;
    };
    if has_extension(icon, "svg") {
        if size_of(&path) > ICON_MAX_SVG_BYTES {
            problems.push(tr!("The SVG icon must be at most 512 KB.").to_string());
        }
        return;
    }
    let square = has_extension(icon, "png")
        && png_size(&path).is_some_and(|(width, height)| width == height && width >= ICON_MIN_SIZE);
    if !square {
        problems.push(
            tr!("The marketplace icon must be an SVG, or a square PNG of at least 256×256.")
                .to_string(),
        );
    }
    if size_of(&path) > ICON_MAX_PNG_BYTES {
        problems.push(tr!("The PNG icon must be at most 1 MB.").to_string());
    }
}

fn check_screenshot(source: &Path, screenshot: &str, problems: &mut Vec<String>) {
    let Some(path) = listing_asset(source, screenshot) else {
        problems.push(tr!("The screenshot %@ wasn't found.", screenshot).to_string());
        return;
    };
    let shaped = png_size(&path).is_some_and(|(width, height)| {
        let ratio = f64::from(width) / f64::from(height.max(1));
        [16.0 / 10.0, 16.0 / 9.0]
            .iter()
            .any(|allowed: &f64| (ratio - allowed).abs() < 0.01)
    });
    if !shaped {
        problems
            .push(tr!("The screenshot %@ must be a 16:10 or 16:9 PNG.", screenshot).to_string());
    }
    if size_of(&path) > SCREENSHOT_MAX_BYTES {
        problems.push(tr!("The screenshot %@ must be at most 3 MB.", screenshot).to_string());
    }
}

fn left_out(relative: &Path) -> bool {
    let at_root = relative.components().next().is_some_and(|part| {
        let name = part.as_os_str().to_string_lossy().to_lowercase();
        LEFT_OUT_AT_ROOT.contains(&name.as_str())
    });
    at_root
        || relative.components().any(|part| {
            let name = part.as_os_str().to_string_lossy().to_lowercase();
            LEFT_OUT_NAMES.contains(&name.as_str())
                || name.starts_with(".env.")
                || LEFT_OUT_ENDINGS.iter().any(|ending| name.ends_with(ending))
        })
}

/// The source files a pull request would carry, and their total size.
fn files_to_publish(source: &Path) -> io::Result<(Vec<PathBuf>, u64)> {
    fn walk(
        root: &Path,
        folder: &Path,
        files: &mut Vec<PathBuf>,
        bytes: &mut u64,
    ) -> io::Result<()> {
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(io::Error::other)?;
            let kind = entry.file_type()?;
            if kind.is_symlink() || entry.file_name() == ".git" || left_out(relative) {
                continue;
            }
            if kind.is_dir() {
                walk(root, &path, files, bytes)?;
            } else if kind.is_file() {
                *bytes += entry.metadata()?.len();
                files.push(relative.to_owned());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    let mut bytes = 0;
    walk(source, source, &mut files, &mut bytes)?;
    files.sort();
    Ok((files, bytes))
}

pub(crate) fn github_account(tools: &dyn Tools) -> GitHub {
    if !tools.installed("gh") {
        return GitHub::Missing;
    }
    match tools.run(
        "gh",
        &["api", "user", "--hostname", GITHUB_HOST, "--jq", ".login"],
        None,
    ) {
        Ok(login) if !login.is_empty() => GitHub::User(login),
        Ok(_) => GitHub::SignedOut,
        Err(error) if signed_out(&error) => GitHub::SignedOut,
        Err(error) => GitHub::Unreachable(error),
    }
}

fn signed_out(error: &str) -> bool {
    let error = error.to_lowercase();
    ["auth login", "not logged", "401", "bad credentials"]
        .iter()
        .any(|sign| error.contains(sign))
}

/// Copies the files to a private folder. One that went away or became a link
/// since it was listed, or a folder that is itself a link, stops the publish.
fn stage(source: &Path, files: &[PathBuf]) -> Result<tempfile::TempDir, String> {
    let changed = || tr!("The extension's files changed while checking them. Publish again.");
    let linked = fs::symlink_metadata(source).map_err(|_| changed().to_string())?;
    if linked.is_symlink() {
        return Err(changed().to_string());
    }
    let staged = tempfile::tempdir().map_err(|error| error.to_string())?;
    for file in files {
        let mut path = source.to_owned();
        for part in file.components() {
            path.push(part);
            let metadata = fs::symlink_metadata(&path).map_err(|_| changed().to_string())?;
            if metadata.is_symlink() {
                return Err(changed().to_string());
            }
        }
        let target = staged.path().join(file);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(&path, &target).map_err(|error| error.to_string())?;
    }
    Ok(staged)
}

/// The store's copy of `name`, if it has one.
pub(crate) fn published_version(
    tools: &dyn Tools,
    name: &str,
) -> Result<Option<Published>, String> {
    let path = format!("repos/{STORE_REPOSITORY}/contents/extensions/{name}/package.json");
    let text = match tools.run(
        "gh",
        &[
            "api",
            "-H",
            "Accept: application/vnd.github.raw",
            &path,
            "--hostname",
            GITHUB_HOST,
        ],
        None,
    ) {
        Ok(text) => text,
        Err(error) if error.contains("404") => return Ok(None),
        Err(error) => return Err(error),
    };
    let value: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    Ok(Some(Published {
        version: value["version"].as_str().unwrap_or_default().to_owned(),
        author: value["muxy"]["marketplace"]["github"]
            .as_str()
            .map(str::to_owned),
    }))
}

/// Adds what only the store knows: a published version can't change.
pub(crate) fn check_against_store(readiness: &mut Readiness, published: Option<&Published>) {
    if let Some(published) = published
        && !marketplace::is_update(&published.version, &readiness.version)
    {
        readiness.problems.push(
            tr!(
                "Version %@ isn't newer than the published %@. Bump the version in package.json.",
                &readiness.version,
                &published.version
            )
            .to_string(),
        );
    }
}

fn pull_request_body(readiness: &Readiness, update: bool) -> String {
    let permissions = if readiness.permissions.is_empty() {
        "None.".to_owned()
    } else {
        readiness
            .permissions
            .iter()
            .map(|permission| format!("- `{permission}`"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "## Extension\n\n\
         - **Name:** {}\n\
         - **Version:** {}\n\
         - **New or update:** {}\n\n\
         ## What it does\n\n{}\n\n\
         ## Permissions\n\n{permissions}\n\n\
         ## Checklist\n\n\
         - [x] Directory name equals `manifest.name`.\n\
         - [ ] `node scripts/validate.mjs <name>` passes locally.\n\
         - [x] `README.md` is present with a short description.\n\
         - [x] `marketplace.icon` is set (SVG, or square PNG ≥256×256).\n\
         - [x] At least one `marketplace.screenshots` entry.\n\
         - [ ] Only permissions actually used are declared.\n\
         - [ ] Source is readable (not minified/obfuscated).\n\
         - [{}] Bumped `version` if this updates an existing extension.\n\n\
         Opened from Muxy's Publish action.\n",
        readiness.name,
        readiness.version,
        if update { "update" } else { "new" },
        readiness.description,
        if update { "x" } else { " " },
    )
}

/// The open pull request from `owner`'s `branch`, if there is one.
fn open_pull_request(
    tools: &dyn Tools,
    owner: &str,
    branch: &str,
) -> Result<Option<String>, String> {
    let listed = tools.run(
        "gh",
        &[
            "pr",
            "list",
            "--repo",
            STORE_ON_GITHUB,
            "--head",
            branch,
            "--state",
            "open",
            "--json",
            "url,headRepositoryOwner",
        ],
        None,
    )?;
    let requests: Vec<Value> = serde_json::from_str(&listed).map_err(|error| error.to_string())?;
    Ok(requests
        .iter()
        .find(|request| request["headRepositoryOwner"]["login"] == json!(owner))
        .and_then(|request| request["url"].as_str())
        .map(str::to_owned))
}

/// A branch to push to: the open pull request's, whose commits get replaced, or
/// one the fork doesn't have, so that no other branch of the user's is overwritten.
fn choose_branch(
    tools: &dyn Tools,
    owner: &str,
    fork_url: &str,
    base: &str,
) -> Result<(String, Option<String>), String> {
    for attempt in 1..=BRANCH_ATTEMPTS {
        let branch = if attempt == 1 {
            base.to_owned()
        } else {
            format!("{base}-{attempt}")
        };
        if let Some(url) = open_pull_request(tools, owner, &branch)? {
            return Ok((branch, Some(url)));
        }
        let taken = tools
            .run(
                "git",
                &[
                    "ls-remote",
                    "--heads",
                    fork_url,
                    &format!("refs/heads/{branch}"),
                ],
                None,
            )
            .is_ok_and(|listed| !listed.is_empty());
        if !taken {
            return Ok((branch, None));
        }
    }
    Err(tr!("Your fork already has too many branches named %@.", base).to_string())
}

/// A fork GitHub only just created can take a moment to accept a push.
fn push(
    tools: &dyn Tools,
    repository: &Path,
    fork_url: &str,
    branch: &str,
    force: bool,
) -> Result<(), String> {
    let refspec = format!("HEAD:refs/heads/{branch}");
    let mut arguments = vec![
        "-c",
        "credential.helper=",
        "-c",
        "credential.helper=!gh auth git-credential",
        "push",
    ];
    if force {
        arguments.push("--force");
    }
    arguments.extend([fork_url, &refspec]);
    let mut result = Err(String::new());
    for attempt in 0..PUSH_ATTEMPTS {
        if attempt > 0 {
            tools.wait();
        }
        result = tools.run("git", &arguments, Some(repository)).map(|_| ());
        if result.is_ok() {
            break;
        }
    }
    result
}

/// Forks the store if needed, commits the staged files to a new branch, pushes
/// it, and opens a pull request or updates the open one. The caller has already
/// had this approved by the user.
pub(crate) fn publish(
    tools: &dyn Tools,
    staged: &Path,
    readiness: &Readiness,
    login: &str,
    update: bool,
) -> Result<PullRequest, String> {
    let fork = tools.run(
        "gh",
        &[
            "api",
            "-X",
            "POST",
            &format!("repos/{STORE_REPOSITORY}/forks"),
            "--hostname",
            GITHUB_HOST,
            "--jq",
            ".full_name",
        ],
        None,
    )?;
    let owner = fork.split('/').next().unwrap_or(login);
    let fork_url = format!("https://github.com/{fork}.git");
    let (branch, open) = choose_branch(
        tools,
        owner,
        &fork_url,
        &branch_name(&readiness.name, &readiness.version),
    )?;

    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let repository = scratch.path().join("repository");
    let store_url = format!("https://github.com/{STORE_REPOSITORY}");
    tools.run(
        "git",
        &[
            "clone",
            "--filter=blob:none",
            "--sparse",
            &store_url,
            &repository.to_string_lossy(),
        ],
        None,
    )?;
    let folder = format!("extensions/{}", readiness.name);
    tools.run(
        "git",
        &["sparse-checkout", "set", &folder],
        Some(&repository),
    )?;
    tools.run("git", &["checkout", "-b", &branch], Some(&repository))?;
    let destination = repository.join(&folder);
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|error| error.to_string())?;
    }
    copy_tree(staged, &destination).map_err(|error| error.to_string())?;
    tools.run("git", &["add", "--all", "--", &folder], Some(&repository))?;
    let changed = tools.run("git", &["status", "--porcelain"], Some(&repository))?;
    if changed.is_empty() && open.is_none() {
        return Err(
            tr!("This version is already what the store has. Nothing to publish.").to_string(),
        );
    }
    let title = pull_request_title(&readiness.name, &readiness.version);
    let name_option = format!("user.name={login}");
    let email_option = format!("user.email={login}@users.noreply.github.com");
    let mut commit = vec!["-c", "commit.gpgsign=false"];
    if tools
        .run("git", &["config", "user.email"], Some(&repository))
        .is_err()
    {
        commit.extend(["-c", &name_option, "-c", &email_option]);
    }
    commit.extend(["commit", "--allow-empty", "-m", &title]);
    tools.run("git", &commit, Some(&repository))?;
    push(tools, &repository, &fork_url, &branch, open.is_some())?;

    if let Some(url) = open {
        return Ok(PullRequest {
            url,
            branch,
            updated: true,
        });
    }
    let head = format!("{owner}:{branch}");
    let body = pull_request_body(readiness, update);
    let url = tools.run(
        "gh",
        &[
            "pr",
            "create",
            "--repo",
            STORE_ON_GITHUB,
            "--base",
            "main",
            "--head",
            &head,
            "--title",
            &title,
            "--body",
            &body,
        ],
        None,
    )?;
    Ok(PullRequest {
        url: url.lines().last().unwrap_or_default().to_owned(),
        branch,
        updated: false,
    })
}

#[cfg(test)]
mod tests;
