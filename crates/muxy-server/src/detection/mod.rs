mod regions;

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use muxy_protocol::{AgentProvider, AgentState};
use muxy_terminal::{ProgramState, ProgramStatus};
use regex::Regex;
use serde::Deserialize;

#[derive(Clone, Copy)]
struct DetectionInput<'a> {
    screen: &'a str,
    osc_title: &'a str,
    osc_progress: &'a str,
}

#[derive(Deserialize)]
struct Manifest {
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
struct Rule {
    id: String,
    state: AgentState,
    priority: i32,
    region: String,
    #[serde(default)]
    visible_idle: bool,
    #[serde(default)]
    skip_state_update: bool,
    #[serde(flatten)]
    gate: Gate,
}

#[derive(Default, Deserialize)]
struct Gate {
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    line_regex: Vec<String>,
    #[serde(default)]
    all: Vec<Self>,
    #[serde(default)]
    any: Vec<Self>,
    #[serde(default, rename = "not")]
    none: Vec<Self>,
}

struct Matcher {
    contains: Vec<String>,
    regex: Vec<Regex>,
    lines: Vec<Regex>,
    all: Vec<Self>,
    any: Vec<Self>,
    none: Vec<Self>,
}

impl Matcher {
    fn compile(gate: Gate) -> Result<Self, regex::Error> {
        Ok(Self {
            contains: gate
                .contains
                .into_iter()
                .map(|s| s.to_lowercase())
                .collect(),
            regex: gate
                .regex
                .iter()
                .map(|s| Regex::new(s))
                .collect::<Result<_, _>>()?,
            lines: gate
                .line_regex
                .iter()
                .map(|s| Regex::new(s))
                .collect::<Result<_, _>>()?,
            all: gate
                .all
                .into_iter()
                .map(Self::compile)
                .collect::<Result<_, _>>()?,
            any: gate
                .any
                .into_iter()
                .map(Self::compile)
                .collect::<Result<_, _>>()?,
            none: gate
                .none
                .into_iter()
                .map(Self::compile)
                .collect::<Result<_, _>>()?,
        })
    }

    fn matches(&self, text: &str, lower: &str) -> bool {
        self.contains.iter().all(|s| lower.contains(s))
            && self.regex.iter().all(|r| r.is_match(text))
            && self
                .lines
                .iter()
                .all(|r| text.lines().any(|line| r.is_match(line)))
            && self.all.iter().all(|m| m.matches(text, lower))
            && (self.any.is_empty() || self.any.iter().any(|m| m.matches(text, lower)))
            && !self.none.iter().any(|m| m.matches(text, lower))
    }
}

struct CompiledRule {
    rule: Rule,
    matcher: Matcher,
}

const MANIFESTS: &[(AgentProvider, &str)] = &[
    (AgentProvider::Claude, include_str!("manifests/claude.toml")),
    (AgentProvider::Codex, include_str!("manifests/codex.toml")),
    (
        AgentProvider::OpenCode,
        include_str!("manifests/opencode.toml"),
    ),
    (AgentProvider::Cursor, include_str!("manifests/cursor.toml")),
    (
        AgentProvider::Copilot,
        include_str!("manifests/github-copilot.toml"),
    ),
    (AgentProvider::Droid, include_str!("manifests/droid.toml")),
    (AgentProvider::Pi, include_str!("manifests/pi.toml")),
    (AgentProvider::Grok, include_str!("manifests/grok.toml")),
    (AgentProvider::Kiro, include_str!("manifests/kiro.toml")),
    (AgentProvider::Xal, include_str!("manifests/xal.toml")),
    (
        AgentProvider::Antigravity,
        include_str!("manifests/antigravity.toml"),
    ),
    (AgentProvider::OhMyPi, include_str!("manifests/omp.toml")),
    (AgentProvider::Hermes, include_str!("manifests/hermes.toml")),
];

fn compile(source: &str) -> Result<Vec<CompiledRule>, String> {
    let mut manifest: Manifest = toml::from_str(source).map_err(|e| e.to_string())?;
    manifest
        .rules
        .sort_by_key(|r| std::cmp::Reverse(r.priority));
    manifest
        .rules
        .into_iter()
        .map(|mut rule| {
            let matcher = Matcher::compile(std::mem::take(&mut rule.gate))
                .map_err(|e| format!("{}: {e}", rule.id))?;
            Ok(CompiledRule { rule, matcher })
        })
        .collect()
}

/// Compiles a provider's rules the first time it runs, so a server holds only the
/// regexes of agents it has seen. The largest set takes a few milliseconds.
fn rules(provider: AgentProvider) -> &'static [CompiledRule] {
    static RULES: [OnceLock<Vec<CompiledRule>>; MANIFESTS.len()] =
        [const { OnceLock::new() }; MANIFESTS.len()];
    let Some(index) = MANIFESTS.iter().position(|(p, _)| *p == provider) else {
        return &[];
    };
    RULES[index].get_or_init(|| {
        compile(MANIFESTS[index].1).unwrap_or_else(|error| {
            log::error!("{} detection: {error}", provider.name());
            Vec::new()
        })
    })
}

fn detect(provider: AgentProvider, input: DetectionInput<'_>) -> Option<(AgentState, bool)> {
    for compiled in rules(provider) {
        let text = regions::region(input, &compiled.rule.region);
        if compiled.matcher.matches(text, &text.to_lowercase()) {
            return (!compiled.rule.skip_state_update)
                .then_some((compiled.rule.state, compiled.rule.visible_idle));
        }
    }
    // Like herdr, known-agent fallback is idle, subject to transition confirmation.
    Some((AgentState::Idle, false))
}

#[derive(Debug, Default)]
pub(crate) struct Detector {
    pub(crate) provider: Option<AgentProvider>,
    pub(crate) state: AgentState,
    pending: Option<Instant>,
    last_input: Option<(String, String, String)>,
    background: bool,
    cycle: bool,
}

impl Detector {
    #[cfg(test)]
    pub(crate) fn update(
        &mut self,
        provider: Option<AgentProvider>,
        screen: String,
        title: &str,
        progress: &str,
        now: Instant,
    ) -> bool {
        self.update_with_background(provider, screen, title, progress, false, now)
    }

    /// Like `update`, where `background` says the agent reported work running
    /// that its screen may not show, such as subagents. That work keeps an
    /// idle screen working; anything the screen shows still takes priority.
    pub(crate) fn update_with_background(
        &mut self,
        provider: Option<AgentProvider>,
        screen: String,
        title: &str,
        progress: &str,
        background: bool,
        now: Instant,
    ) -> bool {
        if self.provider != provider {
            *self = Self {
                provider,
                ..Self::default()
            };
        }
        let Some(provider) = provider else {
            return false;
        };
        let input = (screen, title.to_owned(), progress.to_owned());
        let unchanged = self.last_input.as_ref() == Some(&input) && self.background == background;
        if unchanged && self.pending.is_none() {
            return false;
        }
        let result = detect(
            provider,
            DetectionInput {
                screen: &input.0,
                osc_title: &input.1,
                osc_progress: &input.2,
            },
        );
        self.last_input = Some(input);
        self.background = background;
        let Some((mut next, explicit_idle)) = result else {
            self.pending = None;
            return false;
        };
        if background && next == AgentState::Idle {
            next = AgentState::Working;
        }
        if next == AgentState::Idle
            && matches!(self.state, AgentState::Working | AgentState::Blocked)
        {
            let has_signal = self
                .last_input
                .as_ref()
                .is_some_and(|(screen, title, progress)| {
                    !screen.trim().is_empty() || !title.trim().is_empty() || !progress.is_empty()
                });
            // A cleared screen is never evidence that a working cycle finished.
            if !has_signal {
                self.pending = None;
                return false;
            }
            let started = *self.pending.get_or_insert(now);
            let confirmation = Duration::from_millis(if explicit_idle { 300 } else { 700 });
            if now.duration_since(started) < confirmation {
                return false;
            }
        }
        self.settle(next)
    }

    /// Takes the state the agent reported itself (OSC 7501), which needs no
    /// confirmation and outranks what the screen shows.
    pub(crate) fn report(&mut self, provider: AgentProvider, state: ProgramState) -> bool {
        if self.provider != Some(provider) {
            *self = Self {
                provider: Some(provider),
                ..Self::default()
            };
        }
        self.last_input = None;
        self.settle(match state {
            ProgramState::Working => AgentState::Working,
            ProgramState::Blocked => AgentState::Blocked,
            ProgramState::Idle | ProgramState::Done | ProgramState::Error => AgentState::Idle,
        })
    }

    /// Forgets that work started, as when a terminal switches to another of
    /// its agent's tabs: the work it showed hasn't finished just because the
    /// next tab's screen is idle.
    pub(crate) fn forget_cycle(&mut self) {
        self.cycle = false;
    }

    fn settle(&mut self, next: AgentState) -> bool {
        self.pending = None;
        let completed = self.cycle && next == AgentState::Idle;
        if next == AgentState::Working {
            self.cycle = true;
        }
        if next == AgentState::Idle || next == AgentState::Unknown {
            self.cycle = false;
        }
        self.state = next;
        completed
    }
}

/// The agent to update and the status report it made, from the agent
/// running in the foreground, the agent last seen, and the latest report
/// (OSC 7501). A report naming another program, or another agent, is not
/// the agent's; one naming an agent recognizes it even behind SSH.
pub(crate) fn reporting_agent(
    running: Option<AgentProvider>,
    previous: Option<AgentProvider>,
    status: Option<ProgramStatus>,
) -> (Option<AgentProvider>, Option<ProgramStatus>) {
    let Some(status) = status else {
        return (running, None);
    };
    if status.app.is_empty() {
        return (running.or(previous), Some(status));
    }
    match (running, alias(&status.app)) {
        (_, None) => (running, None),
        (Some(running), Some(named)) if running != named => (Some(running), None),
        (_, named) => (named, Some(status)),
    }
}

pub(crate) fn identify(name: &str, argv: &[String]) -> Option<AgentProvider> {
    let name = basename(name);
    if let Some(provider) = alias(name) {
        return Some(provider);
    }
    let runtime = argv.first().map_or(name, |arg| basename(arg));
    if let Some(provider) = alias(runtime) {
        return Some(provider);
    }
    if !matches!(
        runtime,
        "node" | "bun" | "python" | "python3" | "bash" | "sh" | "zsh" | "fish"
    ) && !runtime.starts_with("python3.")
    {
        return None;
    }
    if let Some(program) = python_program(runtime, argv) {
        return program
            .contains("hermes_cli.main")
            .then_some(AgentProvider::Hermes);
    }
    // Only inspect the executable/script position, never arbitrary prompt arguments.
    let script = argv.get(1)?;
    if script.starts_with('-') {
        return None;
    }
    alias(basename(script)).or_else(|| {
        let parts: Vec<_> = script.split('/').collect();
        if parts
            .windows(2)
            .any(|parts| parts == ["cursor-agent", "versions"])
            && basename(script) == "index.js"
        {
            return Some(AgentProvider::Cursor);
        }
        parts.windows(2).find_map(|part| match part {
            ["@anthropic-ai", "claude-code"] => Some(AgentProvider::Claude),
            ["@openai", "codex"] => Some(AgentProvider::Codex),
            ["@github", "copilot"] => Some(AgentProvider::Copilot),
            ["@mariozechner", "pi-coding-agent"] => Some(AgentProvider::Pi),
            _ => None,
        })
    })
}

/// The program text a Python interpreter runs with `-c`, found by reading only
/// the interpreter's own options, so arguments passed to the program are never
/// read. Installers such as Hermes's launch their agent this way.
fn python_program<'a>(runtime: &str, argv: &'a [String]) -> Option<&'a str> {
    if !runtime.starts_with("python") {
        return None;
    }
    let mut options = argv.iter().skip(1);
    while let Some(option) = options.next() {
        match option.as_str() {
            "-c" => return options.next().map(String::as_str),
            "-X" | "-W" => {
                options.next();
            }
            option if option.starts_with('-') && option != "-" && option != "--" => {}
            _ => return None,
        }
    }
    None
}

/// The agent a member of a foreground process group runs. A one-shot
/// `claude -p` under another program is that program's helper, not a session
/// in this terminal; one the terminal runs directly still counts.
pub(crate) fn identify_member(name: &str, argv: &[String], leader: bool) -> Option<AgentProvider> {
    let provider = identify(name, argv)?;
    let one_shot = provider == AgentProvider::Claude
        && argv
            .iter()
            .skip(1)
            .any(|arg| arg == "-p" || arg == "--print");
    (leader || !one_shot).then_some(provider)
}

fn basename(value: &str) -> &str {
    value.rsplit('/').next().unwrap_or(value)
}

fn alias(value: &str) -> Option<AgentProvider> {
    Some(match value.trim_end_matches(".js") {
        "claude" | "claude-code" => AgentProvider::Claude,
        "codex" => AgentProvider::Codex,
        "opencode" | "opencode2" => AgentProvider::OpenCode,
        "cursor-agent" | "cursor" | "agent" => AgentProvider::Cursor,
        "copilot" | "github-copilot" => AgentProvider::Copilot,
        "droid" => AgentProvider::Droid,
        "pi" => AgentProvider::Pi,
        "grok" | "grok-build" => AgentProvider::Grok,
        "kiro" | "kiro-cli" => AgentProvider::Kiro,
        "xal" | "xal-dev" => AgentProvider::Xal,
        "agy" | "antigravity" | "antigravity-cli" => AgentProvider::Antigravity,
        "omp" | "oh-my-pi" => AgentProvider::OhMyPi,
        "hermes" | "hermes-agent" => AgentProvider::Hermes,
        _ => return None,
    })
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests;
