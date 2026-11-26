mod claude;
mod process;
mod report;
mod store;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use muxy_protocol::{ExitReason, SessionId};

use report::Request;
pub(crate) use store::{Record, Store};

pub(crate) const TOKEN_VARIABLE: &str = "OSC7577_TOKEN";
pub(crate) const ARGV_VARIABLE: &str = "MUXY_RESUME_ARGV";
pub(crate) const PARENT_VARIABLE: &str = "MUXY_RESUME_PARENT";
const TIME_WINDOW: u64 = 60;
const EXIT_GRACE: Duration = Duration::from_secs(2);
const AGENT_POLL: Duration = Duration::from_secs(2);
const MAX_PROCESSES: usize = 64;
const NO_TERMINAL: [u64; 2] = [0, u32::MAX as u64];

pub(crate) fn token() -> Option<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    Some(muxy_core::base64::encode_url(&bytes))
}

pub(crate) fn launch(
    store: &Store,
    from: Option<SessionId>,
    settings: &crate::ServerSettings,
) -> Option<Record> {
    let record = store.take(from?)?;
    let program = record.argv.first()?;
    let usable = cfg!(unix)
        && settings.allows_resume(program)
        && process::current_user() != 0
        && record
            .argv
            .iter()
            .all(|argument| !argument.chars().any(char::is_control))
        && record.cwd.as_ref().is_none_or(|cwd| cwd.is_dir());
    usable.then_some(record)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Source {
    Reported,
    Agent,
}

#[derive(Debug)]
struct Owner {
    pid: u32,
    started: u64,
    source: Source,
    record: Record,
    exited: Option<Instant>,
}

#[derive(Debug)]
pub(crate) struct Tracker {
    session: SessionId,
    store: Store,
    token: Option<String>,
    shell: u32,
    terminal: Option<u64>,
    owner: Option<Owner>,
    times: HashMap<u32, u64>,
    claude: Option<PathBuf>,
    next_agent: Instant,
}

impl Tracker {
    pub(crate) fn new(session: SessionId, store: Store, token: Option<String>, shell: u32) -> Self {
        Self {
            session,
            store,
            claude: token.as_ref().and_then(|_| claude::configuration()),
            token,
            shell,
            terminal: None,
            owner: None,
            times: HashMap::new(),
            next_agent: Instant::now(),
        }
    }

    pub(crate) fn report(&mut self, body: &[u8], directory: impl FnOnce(u32) -> Option<PathBuf>) {
        let Some(token) = &self.token else {
            return;
        };
        let Some(report) = report::verify(body, token) else {
            return;
        };
        if report.time.abs_diff(unix_now()) > TIME_WINDOW
            || self
                .times
                .get(&report.pid)
                .is_some_and(|last| report.time < *last)
        {
            return;
        }
        let Some(identity) = self.member(report.pid) else {
            return;
        };
        if self.times.len() >= MAX_PROCESSES && !self.times.contains_key(&report.pid) {
            self.times.clear();
        }
        self.times.insert(report.pid, report.time);
        match report.request {
            Request::Resume { argv, cwd } => {
                let cwd = cwd.or_else(|| directory(report.pid));
                self.replace(Some(Owner {
                    pid: report.pid,
                    started: identity.started,
                    source: Source::Reported,
                    record: Record { argv, cwd },
                    exited: None,
                }));
            }
            Request::Clear | Request::Rejected => {
                if self
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.pid == report.pid)
                {
                    self.replace(None);
                }
            }
        }
    }

    pub(crate) fn claude(&mut self, pid: u32) {
        if self.token.is_none()
            || self
                .owner
                .as_ref()
                .is_some_and(|owner| owner.source == Source::Reported && owner.exited.is_none())
        {
            return;
        }
        let now = Instant::now();
        if now < self.next_agent {
            return;
        }
        let known = self.owner.as_ref().is_some_and(|owner| owner.pid == pid);
        self.next_agent = now + AGENT_POLL;
        let Some(record) = self
            .claude
            .as_deref()
            .and_then(|configuration| claude::record(configuration, pid))
        else {
            if known
                && self
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.source == Source::Agent)
            {
                self.replace(None);
            }
            return;
        };
        if known
            && self
                .owner
                .as_ref()
                .is_some_and(|owner| owner.record == record)
        {
            return;
        }
        let Some(identity) = self.member(pid) else {
            return;
        };
        self.replace(Some(Owner {
            pid,
            started: identity.started,
            source: Source::Agent,
            record,
            exited: None,
        }));
    }

    pub(crate) fn refresh(&mut self) {
        let Some(owner) = &mut self.owner else {
            return;
        };
        if process::identity(owner.pid).is_some_and(|identity| identity.started == owner.started) {
            owner.exited = None;
            return;
        }
        let now = Instant::now();
        if now.duration_since(*owner.exited.get_or_insert(now)) >= EXIT_GRACE {
            self.replace(None);
        }
    }

    pub(crate) fn ended(&mut self, reason: ExitReason) {
        if reason != ExitReason::ServerStopped {
            self.owner = None;
            self.store.save(self.session, None);
        }
    }

    fn member(&mut self, pid: u32) -> Option<process::Identity> {
        let terminal = if let Some(terminal) = self.terminal {
            terminal
        } else {
            let terminal = process::identity(self.shell)?.terminal;
            self.terminal = Some(terminal);
            terminal
        };
        let identity = process::identity(pid)?;
        (identity.user == process::current_user()
            && identity.terminal == terminal
            && !NO_TERMINAL.contains(&terminal))
        .then_some(identity)
    }

    fn replace(&mut self, owner: Option<Owner>) {
        let changed = self.owner.as_ref().map(|owner| &owner.record)
            != owner.as_ref().map(|owner| &owner.record);
        self.owner = owner;
        if changed {
            self.store
                .save(self.session, self.owner.as_ref().map(|owner| &owner.record));
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
