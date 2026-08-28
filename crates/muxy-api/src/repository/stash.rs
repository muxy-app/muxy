use super::{
    BoundedText, MutationControl, MutationEffect, MutationOutcome, RepositoryError,
    RepositoryMutationError, RepositoryService,
};
use crate::subprocess::Deadline;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

const STASH_LIST_LIMIT: usize = 4 * 1_024 * 1_024;
const STASH_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashEntry {
    pub index: usize,
    pub reference: String,
    pub oid: Vec<u8>,
    pub branch: Option<String>,
    pub message: String,
    pub timestamp: i64,
}

impl StashEntry {
    pub fn stable_id(&self) -> &[u8] {
        &self.oid
    }
}

pub type StashPreview = BoundedText;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StashAction {
    Apply,
    Pop,
    Drop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashActionIntent {
    action: StashAction,
    index: usize,
    oid: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum StashMutationError {
    #[error("stash entry is invalid")]
    InvalidEntry,
    #[error("stash action no longer matches repository state")]
    StaleIntent,
    #[error("stash message is invalid")]
    InvalidMessage,
    #[error(transparent)]
    Read(#[from] RepositoryError),
    #[error(transparent)]
    Mutation(#[from] RepositoryMutationError),
}

impl StashMutationError {
    pub fn effect(&self) -> MutationEffect {
        match self {
            Self::Mutation(source) => source.effect(),
            _ => MutationEffect::NoMutation,
        }
    }
}

impl RepositoryService {
    pub fn stash_entries(&self, repository: &Path) -> Result<Vec<StashEntry>, RepositoryError> {
        let deadline = Deadline::new(STASH_TIMEOUT);
        let output = self.complete(
            repository,
            "stash list",
            os_args(&["stash", "list", "--format=%gd%x00%H%x00%gs%x00%ct%x1e"]),
            false,
            STASH_LIST_LIMIT,
            &deadline,
        )?;
        parse_stash_entries(&output.stdout)
    }

    pub fn stash_preview(
        &self,
        repository: &Path,
        entry: &StashEntry,
    ) -> Result<StashPreview, RepositoryError> {
        validate_entry(entry).map_err(|_| RepositoryError::InvalidPath)?;
        let current = self
            .stash_entries(repository)?
            .into_iter()
            .find(|current| current.oid == entry.oid)
            .ok_or(RepositoryError::InvalidPath)?;
        self.diff(
            repository,
            "stash preview",
            vec![
                OsString::from("stash"),
                OsString::from("show"),
                OsString::from("--include-untracked"),
                OsString::from("-p"),
                OsString::from("--no-color"),
                OsString::from("--no-ext-diff"),
                OsString::from(current.reference),
            ],
        )
    }

    pub fn create_stash(
        &self,
        repository: &Path,
        message: Option<&str>,
        control: &MutationControl,
    ) -> Result<MutationOutcome, StashMutationError> {
        if !self.summary(repository)?.is_dirty() {
            return Ok(MutationOutcome::NoMutation);
        }
        let message = message.map(str::trim).filter(|message| !message.is_empty());
        if message.is_some_and(|message| message.contains('\0')) {
            return Err(StashMutationError::InvalidMessage);
        }
        let mut args = os_args(&["stash", "push", "--include-untracked"]);
        if let Some(message) = message {
            args.push(OsString::from("-m"));
            args.push(OsString::from(message));
        }
        self.mutate(
            repository,
            "create stash",
            args,
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn prepare_stash_action(
        &self,
        repository: &Path,
        entry: &StashEntry,
        action: StashAction,
        _control: &MutationControl,
    ) -> Result<StashActionIntent, StashMutationError> {
        validate_entry(entry)?;
        let current = self
            .stash_entries(repository)?
            .into_iter()
            .find(|current| current.oid == entry.oid)
            .ok_or(StashMutationError::StaleIntent)?;
        Ok(StashActionIntent {
            action,
            index: current.index,
            oid: current.oid,
        })
    }

    pub fn apply_stash(
        &self,
        repository: &Path,
        intent: &StashActionIntent,
        control: &MutationControl,
    ) -> Result<MutationOutcome, StashMutationError> {
        self.run_stash_action(repository, intent, StashAction::Apply, control)
    }

    pub fn pop_stash(
        &self,
        repository: &Path,
        intent: &StashActionIntent,
        control: &MutationControl,
    ) -> Result<MutationOutcome, StashMutationError> {
        self.run_stash_action(repository, intent, StashAction::Pop, control)
    }

    pub fn drop_stash(
        &self,
        repository: &Path,
        intent: &StashActionIntent,
        control: &MutationControl,
    ) -> Result<MutationOutcome, StashMutationError> {
        self.run_stash_action(repository, intent, StashAction::Drop, control)
    }

    fn run_stash_action(
        &self,
        repository: &Path,
        intent: &StashActionIntent,
        expected_action: StashAction,
        control: &MutationControl,
    ) -> Result<MutationOutcome, StashMutationError> {
        if intent.action != expected_action || !valid_oid(&intent.oid) {
            return Err(StashMutationError::InvalidEntry);
        }
        let current = self.stash_entries(repository)?;
        let Some(entry) = current.get(intent.index) else {
            return Err(StashMutationError::StaleIntent);
        };
        if entry.oid != intent.oid {
            return Err(StashMutationError::StaleIntent);
        }
        let operation = match expected_action {
            StashAction::Apply => "apply stash",
            StashAction::Pop => "pop stash",
            StashAction::Drop => "drop stash",
        };
        let mut args = os_args(&["stash"]);
        match expected_action {
            StashAction::Apply => args.extend(os_args(&["apply", "--index"])),
            StashAction::Pop => args.extend(os_args(&["pop", "--index"])),
            StashAction::Drop => args.push(OsString::from("drop")),
        }
        args.push(OsString::from(&entry.reference));
        self.mutate(
            repository,
            operation,
            args,
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }
}

fn parse_stash_entries(input: &[u8]) -> Result<Vec<StashEntry>, RepositoryError> {
    let mut entries = Vec::new();
    for record in input.split(|byte| *byte == 0x1e) {
        let record = trim_ascii_whitespace(record);
        if record.is_empty() {
            continue;
        }
        let fields = record.split(|byte| *byte == 0).collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(RepositoryError::InvalidPath);
        }
        let reference = std::str::from_utf8(fields[0]).map_err(|_| RepositoryError::InvalidPath)?;
        let index = parse_reference(reference).ok_or(RepositoryError::InvalidPath)?;
        if !valid_oid(fields[1]) {
            return Err(RepositoryError::InvalidPath);
        }
        let subject = std::str::from_utf8(fields[2]).map_err(|_| RepositoryError::InvalidPath)?;
        let timestamp = std::str::from_utf8(fields[3])
            .map_err(|_| RepositoryError::InvalidPath)?
            .parse()
            .map_err(|_| RepositoryError::InvalidPath)?;
        let (branch, message) = parse_subject(subject);
        entries.push(StashEntry {
            index,
            reference: reference.to_owned(),
            oid: fields[1].to_vec(),
            branch,
            message,
            timestamp,
        });
    }
    Ok(entries)
}

fn validate_entry(entry: &StashEntry) -> Result<(), StashMutationError> {
    if parse_reference(&entry.reference) != Some(entry.index) || !valid_oid(&entry.oid) {
        return Err(StashMutationError::InvalidEntry);
    }
    Ok(())
}

fn parse_reference(reference: &str) -> Option<usize> {
    reference
        .strip_prefix("stash@{")?
        .strip_suffix('}')?
        .parse()
        .ok()
}

fn valid_oid(oid: &[u8]) -> bool {
    matches!(oid.len(), 40 | 64) && oid.iter().all(u8::is_ascii_hexdigit)
}

fn parse_subject(subject: &str) -> (Option<String>, String) {
    for prefix in ["On ", "WIP on "] {
        if let Some(value) = subject.strip_prefix(prefix)
            && let Some((branch, message)) = value.split_once(": ")
        {
            return (Some(branch.to_owned()), message.to_owned());
        }
    }
    (None, subject.to_owned())
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

fn os_args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[cfg(test)]
mod tests {
    use crate::execution_environment::ExecutionEnvironment;
    use crate::git::GitOptions;
    use crate::repository::{
        MutationControl, MutationOutcome, RepositoryOptions, RepositoryService, StashAction,
        StashMutationError,
    };
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};
    use std::path::Path;
    use std::process::Command;

    fn git(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write(path: &Path, value: &str) {
        std::fs::write(path, value).unwrap();
    }

    fn repository() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        git(temp.path(), &["init", "-q", "-b", "main"]);
        git(temp.path(), &["config", "user.name", "Muxy Test"]);
        git(temp.path(), &["config", "user.email", "muxy@example.test"]);
        write(&temp.path().join("tracked.txt"), "base\n");
        git(temp.path(), &["add", "tracked.txt"]);
        git(temp.path(), &["commit", "-q", "-m", "base"]);
        temp
    }

    fn service(home: &Path) -> RepositoryService {
        let environment = ExecutionEnvironment::fallback([
            (
                OsString::from("PATH"),
                std::env::var_os("PATH").unwrap_or_default(),
            ),
            (OsString::from("HOME"), home.as_os_str().to_owned()),
            (
                OsString::from("XDG_CONFIG_HOME"),
                home.join("config").into_os_string(),
            ),
        ]);
        let executable = environment.resolve_executable(OsStr::new("git")).unwrap();
        RepositoryService::new(RepositoryOptions {
            git: GitOptions {
                executable,
                environment: HashMap::new(),
            },
            environment,
        })
    }

    #[test]
    fn repository_stash_lists_metadata_and_returns_a_bounded_patch_preview() {
        let repo = repository();
        let service = service(repo.path());
        write(&repo.path().join("tracked.txt"), "first\n");
        write(&repo.path().join("untracked.txt"), "untracked\n");
        assert_eq!(
            service
                .create_stash(repo.path(), Some("first work"), &MutationControl::default())
                .unwrap(),
            MutationOutcome::Success
        );
        write(&repo.path().join("tracked.txt"), "second\n");
        service
            .create_stash(
                repo.path(),
                Some("second work"),
                &MutationControl::default(),
            )
            .unwrap();

        let entries = service.stash_entries(repo.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].reference, "stash@{0}");
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        assert_eq!(entries[0].message, "second work");
        assert!(matches!(entries[0].oid.len(), 40 | 64));
        assert!(entries[0].timestamp > 0);
        assert_eq!(entries[1].index, 1);
        assert_eq!(entries[1].message, "first work");

        let preview = service.stash_preview(repo.path(), &entries[1]).unwrap();
        assert!(preview.text.contains("tracked.txt"));
        assert!(preview.text.contains("untracked.txt"));
        assert!(!preview.truncated);
    }

    #[test]
    fn repository_stash_apply_pop_and_confirmed_drop_revalidate_the_captured_oid() {
        let repo = repository();
        let service = service(repo.path());
        let control = MutationControl::default();
        write(&repo.path().join("tracked.txt"), "saved\n");
        service
            .create_stash(repo.path(), Some("saved"), &control)
            .unwrap();
        let entry = service.stash_entries(repo.path()).unwrap().remove(0);

        let apply = service
            .prepare_stash_action(repo.path(), &entry, StashAction::Apply, &control)
            .unwrap();
        assert_eq!(
            service.apply_stash(repo.path(), &apply, &control).unwrap(),
            MutationOutcome::Success
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("tracked.txt")).unwrap(),
            "saved\n"
        );
        assert_eq!(service.stash_entries(repo.path()).unwrap().len(), 1);

        git(repo.path(), &["reset", "--hard", "-q", "HEAD"]);
        let pop = service
            .prepare_stash_action(repo.path(), &entry, StashAction::Pop, &control)
            .unwrap();
        assert_eq!(
            service.pop_stash(repo.path(), &pop, &control).unwrap(),
            MutationOutcome::Success
        );
        assert!(service.stash_entries(repo.path()).unwrap().is_empty());

        git(repo.path(), &["reset", "--hard", "-q", "HEAD"]);
        write(&repo.path().join("tracked.txt"), "drop me\n");
        service
            .create_stash(repo.path(), Some("drop me"), &control)
            .unwrap();
        let original = service.stash_entries(repo.path()).unwrap().remove(0);
        let stale = service
            .prepare_stash_action(repo.path(), &original, StashAction::Drop, &control)
            .unwrap();
        write(&repo.path().join("tracked.txt"), "new top\n");
        service
            .create_stash(repo.path(), Some("new top"), &control)
            .unwrap();
        assert!(matches!(
            service.drop_stash(repo.path(), &stale, &control),
            Err(StashMutationError::StaleIntent)
        ));
        let current = service
            .prepare_stash_action(repo.path(), &original, StashAction::Drop, &control)
            .unwrap();
        service.drop_stash(repo.path(), &current, &control).unwrap();
        assert_eq!(service.stash_entries(repo.path()).unwrap().len(), 1);
    }

    #[test]
    fn repository_stash_rejects_clean_create_and_stale_or_malformed_entries() {
        let repo = repository();
        let service = service(repo.path());
        let control = MutationControl::default();
        assert_eq!(
            service.create_stash(repo.path(), None, &control).unwrap(),
            MutationOutcome::NoMutation
        );
        let malformed = crate::repository::StashEntry {
            index: 0,
            reference: "stash@{0};touch outside".to_owned(),
            oid: b"invalid".to_vec(),
            branch: None,
            message: String::new(),
            timestamp: 0,
        };
        assert!(matches!(
            service.prepare_stash_action(repo.path(), &malformed, StashAction::Apply, &control),
            Err(StashMutationError::InvalidEntry)
        ));
    }
}
