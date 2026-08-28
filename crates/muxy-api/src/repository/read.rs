use super::model::{
    BoundedText, BranchEntry, BranchKind, ChangedFile, ChangedFiles, RepositoryError,
    RepositoryHead, RepositoryIdentity, RepositorySummary, UntrackedLineCount,
};
use super::parse;
use crate::execution_environment::ExecutionEnvironment;
use crate::git::GitOptions;
use crate::git::command::{RepositoryCommandRequest, repository_command, run_output};
use crate::subprocess::{
    CancellationSignal, Deadline, StdinMode, SubprocessOutput, bounded_error_text,
};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const READ_STDOUT_LIMIT: usize = 16 * 1_024 * 1_024;
const STDERR_LIMIT: usize = 1_024 * 1_024;
const SUBJECT_STDOUT_LIMIT: usize = 256 * 1_024;
const DIFF_STDOUT_LIMIT: usize = 1_024 * 1_024;
const UNTRACKED_BYTE_LIMIT: u64 = 1_024 * 1_024;
const DIFF_LINE_LIMIT: usize = 800;
const UNTRACKED_LINE_LIMIT: u64 = 800;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct RepositoryOptions {
    pub git: GitOptions,
    pub environment: ExecutionEnvironment,
}

#[derive(Clone, Debug)]
pub struct RepositoryService {
    pub(super) options: RepositoryOptions,
    cancellation: Option<CancellationSignal>,
}

impl RepositoryService {
    pub fn new(options: RepositoryOptions) -> Self {
        Self {
            options,
            cancellation: None,
        }
    }

    pub fn with_cancellation(mut self, cancellation: CancellationSignal) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    pub fn summary(&self, repository: &Path) -> Result<RepositorySummary, RepositoryError> {
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let output = self.complete(
            repository,
            "summary",
            os_args(&[
                "status",
                "--porcelain=v2",
                "--branch",
                "--untracked-files=all",
            ]),
            false,
            READ_STDOUT_LIMIT,
            &deadline,
        )?;
        parse::parse_summary(&output.stdout).map_err(|source| RepositoryError::Parse {
            operation: "summary",
            source,
        })
    }

    pub fn changed_files(&self, repository: &Path) -> Result<ChangedFiles, RepositoryError> {
        let head = match self.summary(repository)?.head {
            RepositoryHead::Unborn => self.empty_tree(repository)?,
            RepositoryHead::Commit(_) => OsString::from("HEAD"),
        };
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let status_args = os_args(&[
            "-c",
            "core.quotepath=false",
            "status",
            "--porcelain=1",
            "-z",
            "--untracked-files=all",
        ]);
        let mut combined_args = os_args(&["-c", "core.quotepath=false", "diff"]);
        combined_args.push(head);
        combined_args.extend(os_args(&["--numstat", "-z", "--no-color", "--no-ext-diff"]));
        let staged_args = os_args(&[
            "-c",
            "core.quotepath=false",
            "diff",
            "--cached",
            "--numstat",
            "-z",
            "--no-color",
            "--no-ext-diff",
        ]);
        let unstaged_args = os_args(&[
            "-c",
            "core.quotepath=false",
            "diff",
            "--numstat",
            "-z",
            "--no-color",
            "--no-ext-diff",
        ]);
        let outputs: Result<_, RepositoryError> = std::thread::scope(|scope| {
            let status = scope.spawn(|| {
                self.complete(
                    repository,
                    "status",
                    status_args,
                    false,
                    READ_STDOUT_LIMIT,
                    &deadline,
                )
            });
            let combined = scope.spawn(|| {
                self.complete(
                    repository,
                    "combined line statistics",
                    combined_args,
                    false,
                    READ_STDOUT_LIMIT,
                    &deadline,
                )
            });
            let staged = scope.spawn(|| {
                self.complete(
                    repository,
                    "staged line statistics",
                    staged_args,
                    false,
                    READ_STDOUT_LIMIT,
                    &deadline,
                )
            });
            let unstaged = scope.spawn(|| {
                self.complete(
                    repository,
                    "unstaged line statistics",
                    unstaged_args,
                    false,
                    READ_STDOUT_LIMIT,
                    &deadline,
                )
            });
            let status = status.join().map_err(|_| RepositoryError::Worker {
                operation: "status",
            })??;
            let combined = combined.join().map_err(|_| RepositoryError::Worker {
                operation: "combined line statistics",
            })??;
            let staged = staged.join().map_err(|_| RepositoryError::Worker {
                operation: "staged line statistics",
            })??;
            let unstaged = unstaged.join().map_err(|_| RepositoryError::Worker {
                operation: "unstaged line statistics",
            })??;
            Ok((status, combined, staged, unstaged))
        });
        let (status, combined, staged, unstaged) = outputs?;
        let status =
            parse::parse_status(&status.stdout).map_err(|source| RepositoryError::Parse {
                operation: "status",
                source,
            })?;
        let combined =
            parse::parse_numstat(&combined.stdout).map_err(|source| RepositoryError::Parse {
                operation: "combined line statistics",
                source,
            })?;
        let staged =
            parse::parse_numstat(&staged.stdout).map_err(|source| RepositoryError::Parse {
                operation: "staged line statistics",
                source,
            })?;
        let unstaged =
            parse::parse_numstat(&unstaged.stdout).map_err(|source| RepositoryError::Parse {
                operation: "unstaged line statistics",
                source,
            })?;
        parse::aggregate(status, combined, staged, unstaged).map_err(|source| {
            RepositoryError::Parse {
                operation: "changed files",
                source,
            }
        })
    }

    pub fn repository_identity(
        &self,
        repository: &Path,
    ) -> Result<RepositoryIdentity, RepositoryError> {
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let root = self.complete(
            repository,
            "worktree root",
            os_args(&["rev-parse", "--show-toplevel"]),
            false,
            READ_STDOUT_LIMIT,
            &deadline,
        )?;
        let git_dir = self.complete(
            repository,
            "Git directory",
            os_args(&["rev-parse", "--absolute-git-dir"]),
            false,
            READ_STDOUT_LIMIT,
            &deadline,
        )?;
        let worktree_root = path_from_output(&root.stdout)?;
        let git_dir = path_from_output(&git_dir.stdout)?;
        Ok(RepositoryIdentity {
            worktree_root: std::fs::canonicalize(worktree_root).map_err(|source| {
                RepositoryError::Io {
                    operation: "worktree root",
                    source,
                }
            })?,
            git_dir: std::fs::canonicalize(git_dir).map_err(|source| RepositoryError::Io {
                operation: "Git directory",
                source,
            })?,
        })
    }

    pub fn local_branches(&self, repository: &Path) -> Result<Vec<Vec<u8>>, RepositoryError> {
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let output = self.complete(
            repository,
            "local branches",
            os_args(&["branch", "--list", "--format=%(refname:short)"]),
            false,
            READ_STDOUT_LIMIT,
            &deadline,
        )?;
        parse_branch_lines(&output.stdout, "local branches")
    }

    pub fn branch_entries(&self, repository: &Path) -> Result<Vec<BranchEntry>, RepositoryError> {
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let output = self.complete(
            repository,
            "branch metadata",
            os_args(&[
                "for-each-ref",
                "--format=%(refname)%00%(refname:short)%00%(objectname)%00%(authorname)%00%(committerdate:unix)%00%(subject)%00%(upstream:short)%00%(HEAD)%1e",
                "refs/heads",
                "refs/remotes",
            ]),
            false,
            READ_STDOUT_LIMIT,
            &deadline,
        )?;
        parse_branch_entries(&output.stdout)
    }

    pub fn remote_branches(&self, repository: &Path) -> Result<Vec<Vec<u8>>, RepositoryError> {
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let output = self.complete(
            repository,
            "remote branches",
            os_args(&["ls-remote", "--heads", "origin"]),
            true,
            READ_STDOUT_LIMIT,
            &deadline,
        )?;
        parse::parse_remote_heads(&output.stdout).map_err(|source| RepositoryError::Parse {
            operation: "remote branches",
            source,
        })
    }

    pub fn default_branch(&self, repository: &Path) -> Result<Option<Vec<u8>>, RepositoryError> {
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let remote = self.invoke(
            repository,
            "default branch",
            os_args(&["ls-remote", "--symref", "origin", "HEAD"]),
            true,
            READ_STDOUT_LIMIT,
            &deadline,
        )?;
        ensure_complete_capture(&remote, "default branch", false)?;
        if remote.status.success()
            && let Some(branch) =
                parse::parse_remote_default_branch(&remote.stdout).map_err(|source| {
                    RepositoryError::Parse {
                        operation: "default branch",
                        source,
                    }
                })?
        {
            return Ok(Some(branch));
        }
        let local = self.invoke(
            repository,
            "default branch fallback",
            os_args(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]),
            false,
            READ_STDOUT_LIMIT,
            &deadline,
        )?;
        ensure_complete_capture(&local, "default branch fallback", false)?;
        if !local.status.success() {
            return Ok(None);
        }
        let value = trim_output(&local.stdout).ok_or(RepositoryError::InvalidPath)?;
        let value = value.strip_prefix(b"origin/").unwrap_or(value);
        if value.is_empty() || value.contains(&0) {
            return Err(RepositoryError::InvalidPath);
        }
        Ok(Some(value.to_vec()))
    }

    pub fn recent_commit_subjects(
        &self,
        repository: &Path,
    ) -> Result<Vec<String>, RepositoryError> {
        if self.summary(repository)?.head == RepositoryHead::Unborn {
            return Ok(Vec::new());
        }
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let output = self.complete(
            repository,
            "commit subjects",
            os_args(&["log", "-z", "--format=%s", "--max-count=12"]),
            false,
            SUBJECT_STDOUT_LIMIT,
            &deadline,
        )?;
        parse::parse_subjects(&output.stdout).map_err(|source| RepositoryError::Parse {
            operation: "commit subjects",
            source,
        })
    }

    pub fn staged_diff(&self, repository: &Path) -> Result<BoundedText, RepositoryError> {
        self.diff(
            repository,
            "staged diff",
            os_args(&["diff", "--cached", "--no-color", "--no-ext-diff"]),
        )
    }

    pub fn branch_diff(
        &self,
        repository: &Path,
        branch: &[u8],
    ) -> Result<BoundedText, RepositoryError> {
        if branch.is_empty() || branch.starts_with(b"-") || branch.contains(&0) {
            return Err(RepositoryError::InvalidPath);
        }
        let mut reference = b"origin/".to_vec();
        reference.extend_from_slice(branch);
        reference.extend_from_slice(b"...HEAD");
        self.diff(
            repository,
            "branch diff",
            vec![
                OsString::from("diff"),
                os_string_from_bytes(reference)?,
                OsString::from("--no-color"),
                OsString::from("--no-ext-diff"),
            ],
        )
    }

    pub fn untracked_line_count(
        &self,
        repository: &Path,
        file: &ChangedFile,
    ) -> UntrackedLineCount {
        if !file.is_untracked {
            return UntrackedLineCount::Unknown;
        }
        let Ok(relative) = path_from_relative_bytes(&file.path) else {
            return UntrackedLineCount::Unknown;
        };
        let Ok(root) = std::fs::canonicalize(repository) else {
            return UntrackedLineCount::Unknown;
        };
        let candidate = root.join(relative);
        let Ok(metadata) = std::fs::symlink_metadata(&candidate) else {
            return UntrackedLineCount::Unknown;
        };
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return UntrackedLineCount::Unknown;
        }
        let Some(parent) = candidate.parent() else {
            return UntrackedLineCount::Unknown;
        };
        let Ok(parent) = std::fs::canonicalize(parent) else {
            return UntrackedLineCount::Unknown;
        };
        if !parent.starts_with(&root) {
            return UntrackedLineCount::Unknown;
        }
        let Ok(file) = std::fs::File::open(candidate) else {
            return UntrackedLineCount::Unknown;
        };
        let mut bytes = Vec::new();
        if file
            .take(UNTRACKED_BYTE_LIMIT + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() as u64 > UNTRACKED_BYTE_LIMIT
            || bytes.contains(&0)
            || std::str::from_utf8(&bytes).is_err()
        {
            return UntrackedLineCount::Unknown;
        }
        let lines = bytes.iter().filter(|byte| **byte == b'\n').count() as u64
            + u64::from(!bytes.is_empty() && bytes.last() != Some(&b'\n'));
        if lines > UNTRACKED_LINE_LIMIT {
            UntrackedLineCount::Unknown
        } else {
            UntrackedLineCount::Known(lines)
        }
    }

    fn empty_tree(&self, repository: &Path) -> Result<OsString, RepositoryError> {
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let output = self.complete_write(
            repository,
            "empty tree",
            os_args(&["mktree"]),
            false,
            256,
            &deadline,
        )?;
        let oid = trim_output(&output.stdout).ok_or(RepositoryError::Parse {
            operation: "empty tree",
            source: super::model::RepositoryParseError::ObjectId,
        })?;
        if !matches!(oid.len(), 40 | 64) || !oid.iter().all(u8::is_ascii_hexdigit) {
            return Err(RepositoryError::Parse {
                operation: "empty tree",
                source: super::model::RepositoryParseError::ObjectId,
            });
        }
        let mut tree = oid.to_vec();
        tree.extend_from_slice(b"^{tree}");
        self.complete(
            repository,
            "empty tree verification",
            vec![
                OsString::from("cat-file"),
                OsString::from("-e"),
                os_string_from_bytes(tree)?,
            ],
            false,
            256,
            &deadline,
        )?;
        os_string_from_bytes(oid.to_vec())
    }

    pub(super) fn diff(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
    ) -> Result<BoundedText, RepositoryError> {
        let deadline = Deadline::new(COMMAND_TIMEOUT);
        let output = self.invoke(
            repository,
            operation,
            args,
            false,
            DIFF_STDOUT_LIMIT,
            &deadline,
        )?;
        ensure_complete_capture(&output, operation, true)?;
        ensure_success(&output, operation)?;
        Ok(bound_text(&output.stdout, output.stdout_truncated))
    }

    pub(super) fn complete(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        network: bool,
        stdout_limit: usize,
        deadline: &Deadline,
    ) -> Result<SubprocessOutput, RepositoryError> {
        let output = self.invoke(repository, operation, args, network, stdout_limit, deadline)?;
        ensure_complete_capture(&output, operation, false)?;
        ensure_success(&output, operation)?;
        Ok(output)
    }

    fn complete_write(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        network: bool,
        stdout_limit: usize,
        deadline: &Deadline,
    ) -> Result<SubprocessOutput, RepositoryError> {
        let command = repository_command(
            &self.options.environment,
            RepositoryCommandRequest {
                args,
                read_only: false,
                network,
                stdin: StdinMode::Closed,
                stdout_limit,
                stderr_limit: STDERR_LIMIT,
                cancellation: self.cancellation.clone(),
            },
        );
        let output = run_output(&self.options.git, repository, command, deadline)
            .map_err(|source| RepositoryError::Process { operation, source })?;
        ensure_complete_capture(&output, operation, false)?;
        ensure_success(&output, operation)?;
        Ok(output)
    }

    fn invoke(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        network: bool,
        stdout_limit: usize,
        deadline: &Deadline,
    ) -> Result<SubprocessOutput, RepositoryError> {
        let command = repository_command(
            &self.options.environment,
            RepositoryCommandRequest {
                args,
                read_only: true,
                network,
                stdin: StdinMode::Closed,
                stdout_limit,
                stderr_limit: STDERR_LIMIT,
                cancellation: self.cancellation.clone(),
            },
        );
        run_output(&self.options.git, repository, command, deadline)
            .map_err(|source| RepositoryError::Process { operation, source })
    }
}

fn ensure_complete_capture(
    output: &SubprocessOutput,
    operation: &'static str,
    allow_stdout_truncation: bool,
) -> Result<(), RepositoryError> {
    if output.stderr_truncated || (output.stdout_truncated && !allow_stdout_truncation) {
        return Err(RepositoryError::Truncated { operation });
    }
    Ok(())
}

fn ensure_success(
    output: &SubprocessOutput,
    operation: &'static str,
) -> Result<(), RepositoryError> {
    if output.status.success() {
        return Ok(());
    }
    Err(RepositoryError::Status {
        operation,
        status: output.status.code(),
        message: bounded_error_text(&output.stderr),
    })
}

fn os_args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn trim_output(bytes: &[u8]) -> Option<&[u8]> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    (!bytes.is_empty()).then_some(bytes)
}

fn parse_branch_lines(
    input: &[u8],
    operation: &'static str,
) -> Result<Vec<Vec<u8>>, RepositoryError> {
    let mut branches = Vec::new();
    for line in input.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        if line.contains(&0) {
            return Err(RepositoryError::InvalidPath);
        }
        branches.push(line.to_vec());
    }
    branches.sort_by(|left, right| parse::natural_cmp(left, right));
    branches.dedup();
    if input.len() > READ_STDOUT_LIMIT {
        return Err(RepositoryError::Truncated { operation });
    }
    Ok(branches)
}

fn parse_branch_entries(input: &[u8]) -> Result<Vec<BranchEntry>, RepositoryError> {
    let mut entries = Vec::new();
    for record in input.split(|byte| *byte == 0x1e) {
        let record = trim_ascii_whitespace(record);
        if record.is_empty() {
            continue;
        }
        let fields = record.split(|byte| *byte == 0).collect::<Vec<_>>();
        if fields.len() != 8 {
            return Err(RepositoryError::InvalidPath);
        }
        let kind = if fields[0].starts_with(b"refs/heads/") {
            BranchKind::Local
        } else if fields[0].starts_with(b"refs/remotes/") {
            BranchKind::Remote
        } else {
            return Err(RepositoryError::InvalidPath);
        };
        if kind == BranchKind::Remote && fields[0].ends_with(b"/HEAD") {
            continue;
        }
        if fields[1].is_empty()
            || fields[1].contains(&0)
            || !matches!(fields[2].len(), 40 | 64)
            || !fields[2].iter().all(u8::is_ascii_hexdigit)
        {
            return Err(RepositoryError::InvalidPath);
        }
        let author = std::str::from_utf8(fields[3])
            .map_err(|_| RepositoryError::InvalidPath)?
            .to_owned();
        let timestamp = std::str::from_utf8(fields[4])
            .map_err(|_| RepositoryError::InvalidPath)?
            .parse()
            .map_err(|_| RepositoryError::InvalidPath)?;
        let subject = std::str::from_utf8(fields[5])
            .map_err(|_| RepositoryError::InvalidPath)?
            .to_owned();
        let upstream = (!fields[6].is_empty()).then(|| fields[6].to_vec());
        entries.push(BranchEntry {
            name: fields[1].to_vec(),
            oid: fields[2].to_vec(),
            kind,
            current: fields[7] == b"*",
            upstream,
            author,
            subject,
            timestamp,
        });
    }
    entries.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| right.current.cmp(&left.current))
            .then_with(|| right.timestamp.cmp(&left.timestamp))
            .then_with(|| parse::natural_cmp(&left.name, &right.name))
    });
    Ok(entries)
}

fn trim_ascii_whitespace(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn bound_text(bytes: &[u8], mut truncated: bool) -> BoundedText {
    let text = String::from_utf8_lossy(bytes);
    let mut end = text.len();
    let mut lines = 0;
    for (index, character) in text.char_indices() {
        if character == '\n' {
            lines += 1;
            if lines == DIFF_LINE_LIMIT {
                end = index + 1;
                if end < text.len() {
                    truncated = true;
                }
                break;
            }
        }
    }
    BoundedText {
        text: text[..end].to_owned(),
        truncated,
    }
}

fn path_from_output(bytes: &[u8]) -> Result<PathBuf, RepositoryError> {
    let bytes = trim_output(bytes).ok_or(RepositoryError::InvalidPath)?;
    if bytes.contains(&0) {
        return Err(RepositoryError::InvalidPath);
    }
    os_string_from_bytes(bytes.to_vec()).map(PathBuf::from)
}

fn path_from_relative_bytes(bytes: &[u8]) -> Result<PathBuf, RepositoryError> {
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(RepositoryError::InvalidPath);
    }
    let path = PathBuf::from(os_string_from_bytes(bytes.to_vec())?);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
    {
        return Err(RepositoryError::InvalidPath);
    }
    Ok(path)
}

#[cfg(unix)]
fn os_string_from_bytes(bytes: Vec<u8>) -> Result<OsString, RepositoryError> {
    use std::os::unix::ffi::OsStringExt;

    Ok(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn os_string_from_bytes(bytes: Vec<u8>) -> Result<OsString, RepositoryError> {
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(|_| RepositoryError::InvalidPath)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_environment::ExecutionEnvironment;
    use crate::git::GitOptions;
    use crate::repository::{RepositoryHead, UntrackedLineCount};
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};
    use std::path::Path;
    use std::process::Command;

    fn environment(home: &Path) -> ExecutionEnvironment {
        ExecutionEnvironment::fallback([
            (
                OsString::from("PATH"),
                std::env::var_os("PATH").unwrap_or_default(),
            ),
            (OsString::from("HOME"), home.as_os_str().to_owned()),
            (
                OsString::from("XDG_CONFIG_HOME"),
                home.join("config").into_os_string(),
            ),
        ])
    }

    fn service(home: &Path) -> RepositoryService {
        let environment = environment(home);
        let executable = environment
            .resolve_executable(OsStr::new("git"))
            .expect("git executable");
        RepositoryService::new(RepositoryOptions {
            git: GitOptions {
                executable,
                environment: HashMap::new(),
            },
            environment,
        })
    }

    fn command(repo: Option<&Path>, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        if let Some(repo) = repo {
            command.arg("-C").arg(repo);
        }
        command
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null");
        command
    }

    fn git(repo: Option<&Path>, args: &[&str]) {
        let output = command(repo, args).output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init(repo: &Path, format: Option<&str>) {
        let mut args = vec!["init", "-q", "-b", "main"];
        if let Some(format) = format {
            args.push(format);
        }
        args.push(repo.to_str().unwrap());
        git(None, &args);
    }

    fn commit(repo: &Path, message: &str) {
        git(Some(repo), &["add", "-A"]);
        git(
            Some(repo),
            &[
                "-c",
                "user.name=Muxy Tests",
                "-c",
                "user.email=muxy@example.invalid",
                "commit",
                "-qm",
                message,
            ],
        );
    }

    fn write(repo: &Path, path: &str, contents: impl AsRef<[u8]>) {
        let path = repo.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn repository_read_summary_covers_unborn_clean_dirty_and_detached_states() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        init(&repo, None);
        let service = service(&temp.path().join("home"));

        let unborn = service.summary(&repo).unwrap();
        assert_eq!(unborn.head, RepositoryHead::Unborn);
        assert_eq!(unborn.branch, "main");
        assert!(!unborn.is_dirty());

        write(&repo, "file.txt", "initial\n");
        let untracked = service.summary(&repo).unwrap();
        assert_eq!(untracked.changed_count, 1);
        assert_eq!(untracked.untracked_count, 1);

        commit(&repo, "initial");
        assert!(!service.summary(&repo).unwrap().is_dirty());
        write(&repo, "file.txt", "changed\n");
        git(Some(&repo), &["add", "file.txt"]);
        write(&repo, "file.txt", "changed twice\n");
        let dirty = service.summary(&repo).unwrap();
        assert_eq!(dirty.staged_count, 1);
        assert_eq!(dirty.unstaged_count, 1);

        git(Some(&repo), &["checkout", "--detach", "-q"]);
        let detached = service.summary(&repo).unwrap();
        assert!(detached.is_detached);
        assert!(detached.display_branch().starts_with("Detached "));
    }

    #[test]
    fn repository_read_unborn_changes_use_git_derived_empty_trees_for_sha1_and_sha256() {
        for format in [None, Some("--object-format=sha256")] {
            let temp = tempfile::tempdir().unwrap();
            let repo = temp.path().join("repo");
            init(&repo, format);
            write(&repo, "staged.txt", "one\ntwo\n");
            git(Some(&repo), &["add", "staged.txt"]);
            let changes = service(&temp.path().join("home"))
                .changed_files(&repo)
                .unwrap();

            assert_eq!(changes.files.len(), 1);
            let file = &changes.files[0];
            assert_eq!(file.path, b"staged.txt");
            assert_eq!(file.combined_stat.unwrap().additions, Some(2));
            assert_eq!(file.staged_stat.unwrap().additions, Some(2));
            assert!(file.unstaged_stat.is_none());
        }
    }

    #[cfg(unix)]
    #[test]
    fn repository_read_changes_preserve_non_utf8_rename_copy_binary_and_dual_sides() {
        #[cfg(not(target_os = "macos"))]
        use std::os::unix::ffi::OsStringExt;

        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        init(&repo, None);
        write(&repo, "original.txt", "same contents for rename and copy\n");
        write(&repo, "both.txt", "base\n");
        commit(&repo, "initial");
        git(Some(&repo), &["config", "status.renames", "copies"]);
        git(Some(&repo), &["config", "diff.renames", "copies"]);

        std::fs::rename(repo.join("original.txt"), repo.join("renamed.txt")).unwrap();
        std::fs::copy(repo.join("renamed.txt"), repo.join("copied.txt")).unwrap();
        #[cfg(not(target_os = "macos"))]
        let raw_name = {
            let raw_name = OsString::from_vec(b"raw\tline\n\xff".to_vec());
            std::fs::write(repo.join(&raw_name), b"raw\n").unwrap();
            raw_name
        };
        std::fs::write(repo.join("binary.bin"), [0, 1, 2, 3]).unwrap();
        write(&repo, "both.txt", "staged\n");
        git(Some(&repo), &["add", "-A"]);
        write(&repo, "both.txt", "unstaged\n");

        let changes = service(&temp.path().join("home"))
            .changed_files(&repo)
            .unwrap();

        #[cfg(not(target_os = "macos"))]
        assert!(
            changes
                .files
                .iter()
                .any(|file| file.path == raw_name.as_encoded_bytes())
        );
        assert!(changes.files.iter().any(|file| file.is_binary));
        let both = changes
            .files
            .iter()
            .find(|file| file.path == b"both.txt")
            .unwrap();
        assert!(both.is_staged && both.is_unstaged);
        assert!(both.staged_stat.is_some());
        assert!(both.unstaged_stat.is_some());
        let renamed = changes
            .files
            .iter()
            .find(|file| file.path == b"renamed.txt")
            .unwrap();
        assert_eq!(
            renamed.old_path.as_deref(),
            Some(b"original.txt".as_slice())
        );
        let copied = changes
            .files
            .iter()
            .find(|file| file.path == b"copied.txt")
            .unwrap();
        assert_eq!(copied.old_path.as_deref(), Some(b"original.txt".as_slice()));
    }

    #[test]
    fn repository_read_changes_model_conflicts_separately() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        init(&repo, None);
        write(&repo, "conflict.txt", "base\n");
        commit(&repo, "base");
        git(Some(&repo), &["switch", "-qc", "other"]);
        write(&repo, "conflict.txt", "other\n");
        commit(&repo, "other");
        git(Some(&repo), &["switch", "-q", "main"]);
        write(&repo, "conflict.txt", "main\n");
        commit(&repo, "main");
        let output = command(
            Some(&repo),
            &[
                "-c",
                "user.name=Muxy Tests",
                "-c",
                "user.email=muxy@example.invalid",
                "merge",
                "other",
            ],
        )
        .output()
        .unwrap();
        assert!(!output.status.success());

        let service = service(&temp.path().join("home"));
        let summary = service.summary(&repo).unwrap();
        assert_eq!(summary.conflicted_count, 1);
        let changes = service.changed_files(&repo).unwrap();
        assert_eq!(changes.conflicts().len(), 1);
        assert!(changes.staged().is_empty());
        assert!(changes.unstaged().is_empty());
    }

    #[test]
    fn repository_read_summary_tracks_ahead_behind_and_diverged_upstream() {
        let temp = tempfile::tempdir().unwrap();
        let origin = temp.path().join("origin.git");
        git(
            None,
            &[
                "init",
                "--bare",
                "-q",
                "-b",
                "main",
                origin.to_str().unwrap(),
            ],
        );
        let repo = temp.path().join("repo");
        init(&repo, None);
        write(&repo, "file.txt", "initial\n");
        commit(&repo, "initial");
        git(
            Some(&repo),
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(Some(&repo), &["push", "-qu", "origin", "main"]);
        let service = service(&temp.path().join("home"));

        write(&repo, "local.txt", "ahead\n");
        commit(&repo, "ahead");
        let ahead = service.summary(&repo).unwrap();
        assert_eq!((ahead.ahead, ahead.behind), (1, 0));
        git(Some(&repo), &["push", "-q"]);

        let other = temp.path().join("other");
        git(
            None,
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        );
        write(&other, "remote.txt", "behind\n");
        commit(&other, "remote");
        git(Some(&other), &["push", "-q"]);
        git(Some(&repo), &["fetch", "-q", "origin"]);
        let behind = service.summary(&repo).unwrap();
        assert_eq!((behind.ahead, behind.behind), (0, 1));

        write(&repo, "diverged.txt", "diverged\n");
        commit(&repo, "diverged");
        let diverged = service.summary(&repo).unwrap();
        assert_eq!((diverged.ahead, diverged.behind), (1, 1));
    }

    #[test]
    fn repository_read_resolves_linked_worktree_identity_and_natural_branch_lists() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let linked = temp.path().join("linked");
        init(&repo, None);
        write(&repo, "file.txt", "initial\n");
        commit(&repo, "initial");
        git(Some(&repo), &["branch", "feature10"]);
        git(Some(&repo), &["branch", "feature2"]);
        git(
            Some(&repo),
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "linked",
                linked.to_str().unwrap(),
            ],
        );
        let service = service(&temp.path().join("home"));

        let identity = service.repository_identity(&linked).unwrap();
        assert_eq!(
            identity.worktree_root,
            std::fs::canonicalize(&linked).unwrap()
        );
        assert!(identity.git_dir.is_absolute());
        assert!(!identity.git_dir.starts_with(&linked));
        let branches = service.local_branches(&repo).unwrap();
        assert_eq!(
            branches,
            [
                b"feature2".to_vec(),
                b"feature10".to_vec(),
                b"linked".to_vec(),
                b"main".to_vec(),
            ]
        );
    }

    #[test]
    fn repository_read_remote_branches_default_branch_subjects_and_diffs_are_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let origin = temp.path().join("origin.git");
        git(
            None,
            &[
                "init",
                "--bare",
                "-q",
                "-b",
                "main",
                origin.to_str().unwrap(),
            ],
        );
        let repo = temp.path().join("repo");
        init(&repo, None);
        for index in 0..14 {
            write(&repo, &format!("commit-{index}"), format!("{index}\n"));
            commit(&repo, &format!("subject {index}"));
        }
        git(Some(&repo), &["branch", "feature10"]);
        git(Some(&repo), &["branch", "feature2"]);
        git(
            Some(&repo),
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(
            Some(&repo),
            &["push", "-q", "origin", "main", "feature2", "feature10"],
        );
        git(
            None,
            &[
                "--git-dir",
                origin.to_str().unwrap(),
                "symbolic-ref",
                "HEAD",
                "refs/heads/main",
            ],
        );
        let service = service(&temp.path().join("home"));

        assert_eq!(
            service.default_branch(&repo).unwrap(),
            Some(b"main".to_vec())
        );
        assert_eq!(
            service.remote_branches(&repo).unwrap(),
            [
                b"feature2".to_vec(),
                b"feature10".to_vec(),
                b"main".to_vec()
            ]
        );
        let subjects = service.recent_commit_subjects(&repo).unwrap();
        assert_eq!(subjects.len(), 12);
        assert_eq!(subjects[0], "subject 13");

        write(&repo, "branch-only.txt", "branch change\n");
        commit(&repo, "branch change");

        let lines: String = (0..900).map(|index| format!("line {index}\n")).collect();
        write(&repo, "large.txt", lines);
        git(Some(&repo), &["add", "large.txt"]);
        let staged = service.staged_diff(&repo).unwrap();
        assert!(staged.truncated);
        assert!(staged.text.lines().count() <= 800);
        let branch = service.branch_diff(&repo, b"main").unwrap();
        assert!(branch.text.contains("branch-only.txt"));

        write(&repo, "huge.txt", vec![b'x'; DIFF_STDOUT_LIMIT + 100_000]);
        git(Some(&repo), &["add", "huge.txt"]);
        let byte_bounded = service.staged_diff(&repo).unwrap();
        assert!(byte_bounded.truncated);
        assert!(byte_bounded.text.len() <= DIFF_STDOUT_LIMIT);

        git(
            Some(&repo),
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );
        git(
            Some(&repo),
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        git(
            Some(&repo),
            &[
                "remote",
                "set-url",
                "origin",
                temp.path().join("missing.git").to_str().unwrap(),
            ],
        );
        assert_eq!(
            service.default_branch(&repo).unwrap(),
            Some(b"main".to_vec())
        );
    }

    #[cfg(unix)]
    #[test]
    fn repository_read_untracked_line_count_is_lazy_bounded_and_contained() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        init(&repo, None);
        write(&repo, "known.txt", "one\ntwo");
        write(&repo, "binary.bin", [0, 1]);
        let large: String = (0..801).map(|_| "line\n").collect();
        write(&repo, "too-many.txt", large);
        write(
            &repo,
            "too-large.txt",
            vec![b'x'; UNTRACKED_BYTE_LIMIT as usize + 1],
        );
        std::fs::create_dir(repo.join("directory")).unwrap();
        write(temp.path(), "outside.txt", "outside\n");
        symlink(temp.path().join("outside.txt"), repo.join("link.txt")).unwrap();
        let service = service(&temp.path().join("home"));
        let changes = service.changed_files(&repo).unwrap();
        let known = changes
            .files
            .iter()
            .find(|file| file.path == b"known.txt")
            .unwrap();
        assert_eq!(
            service.untracked_line_count(&repo, known),
            UntrackedLineCount::Known(2)
        );
        for path in [
            b"binary.bin".as_slice(),
            b"too-many.txt",
            b"too-large.txt",
            b"directory",
            b"link.txt",
            b"../outside.txt",
            b"/absolute",
        ] {
            let mut file = known.clone();
            file.path = path.to_vec();
            assert_eq!(
                service.untracked_line_count(&repo, &file),
                UntrackedLineCount::Unknown
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn repository_read_sanitizes_redirects_and_rejects_truncated_summary() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let selected = temp.path().join("selected");
        let redirected = temp.path().join("redirected");
        init(&selected, None);
        init(&redirected, None);
        write(&selected, "selected.txt", "selected\n");
        let mut variables = environment(&temp.path().join("home")).variables();
        variables.push((
            OsString::from("GIT_DIR"),
            redirected.join(".git").into_os_string(),
        ));
        variables.push((
            OsString::from("GIT_WORK_TREE"),
            redirected.clone().into_os_string(),
        ));
        let redirected_environment = ExecutionEnvironment::fallback(variables);
        let executable = redirected_environment
            .resolve_executable(OsStr::new("git"))
            .unwrap();
        let service = RepositoryService::new(RepositoryOptions {
            git: GitOptions {
                executable,
                environment: HashMap::new(),
            },
            environment: redirected_environment,
        });
        assert_eq!(service.summary(&selected).unwrap().untracked_count, 1);

        let fake = temp.path().join("git");
        std::fs::write(&fake, b"#!/bin/sh\nyes x | head -c 17000000\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let environment = environment(&temp.path().join("fake-home"));
        let service = RepositoryService::new(RepositoryOptions {
            git: GitOptions {
                executable: fake,
                environment: HashMap::new(),
            },
            environment,
        });
        assert!(matches!(
            service.summary(&selected),
            Err(RepositoryError::Truncated {
                operation: "summary"
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn repository_read_cancellation_terminates_the_active_git_child() {
        use crate::git::GitError;
        use crate::subprocess::{CancellationSignal, SubprocessError};
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let repository = temp.path().join("repository");
        let marker = temp.path().join("marker");
        let fake = temp.path().join("git");
        std::fs::create_dir_all(&repository).unwrap();
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\nprintf 'start\\n' >> '{}'\nsleep 5\nprintf 'complete\\n' >> '{}'\nprintf 'main\\n'\n",
                marker.display(),
                marker.display(),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let environment = environment(&temp.path().join("home"));
        let cancellation = CancellationSignal::new();
        let service = RepositoryService::new(RepositoryOptions {
            git: GitOptions {
                executable: fake,
                environment: HashMap::new(),
            },
            environment,
        })
        .with_cancellation(cancellation.clone());
        let read = std::thread::spawn(move || service.local_branches(&repository));
        for _ in 0..100 {
            if std::fs::read_to_string(&marker).is_ok_and(|value| value.contains("start")) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        cancellation.cancel();
        assert!(matches!(
            read.join().unwrap(),
            Err(RepositoryError::Process {
                source: GitError::Process(SubprocessError::Cancelled { .. }),
                ..
            })
        ));
        assert!(
            !std::fs::read_to_string(marker)
                .unwrap_or_default()
                .contains("complete")
        );
    }
}
