use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io;
use std::ops::RangeInclusive;
use std::time::Duration;

use muxy_app_core::browser::automation::{self, DEFAULT_NAVIGATION_WAIT, DEFAULT_WAIT};
use serde_json::{Map, Value};

use crate::args::Command;
use crate::manage::help;

/// How much longer than a quick reply a screenshot may take.
const SCREENSHOT_TIME: Duration = Duration::from_secs(20);

/// A browser command as the request it sends to the desktop app.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Invocation {
    pub(crate) verb: &'static str,
    pub(crate) args: Map<String, Value>,
    pub(crate) output: Output,
    /// How much longer than a quick reply the app may take: what a wait
    /// waits for, or capturing a screenshot.
    pub(crate) extra_time: Duration,
}

/// How the app's reply is printed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Output {
    /// A string as is, such as a new tab's ID.
    Text,
    /// `ok`, once the app has done it.
    Ok,
    Json,
    /// One tab-separated line per tab.
    Tabs,
    /// Title, URL and text, each on its own line.
    Page,
    /// The PNG as base64, ready to decode.
    Screenshot,
}

impl Invocation {
    fn new(verb: &'static str) -> Self {
        Self {
            verb,
            args: Map::new(),
            output: Output::Json,
            extra_time: Duration::ZERO,
        }
    }
    fn arg(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.args.insert(key.to_owned(), value.into());
        self
    }
    fn optional(self, key: &str, value: Option<impl Into<Value>>) -> Self {
        match value {
            Some(value) => self.arg(key, value),
            None => self,
        }
    }
    fn output(self, output: Output) -> Self {
        Self { output, ..self }
    }
    /// Gives the app as long as the wait it was asked for, as the app
    /// itself reads `timeoutMs`.
    fn waiting(self, default: Duration) -> Self {
        Self {
            extra_time: automation::timeout(&self.args, default),
            ..self
        }
    }
}

/// Reads the words after `muxy browser`.
pub(crate) fn parse(arguments: &[OsString]) -> io::Result<Command> {
    let words = arguments
        .iter()
        .map(|word| {
            word.to_str()
                .ok_or_else(|| invalid("text arguments must be UTF-8"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let wants_help = words
        .iter()
        .take_while(|word| **word != "--")
        .any(|word| matches!(*word, "--help" | "-h"));
    if words.is_empty() || wants_help {
        return Ok(Command::Usage(help::BROWSER));
    }
    invocation(&words).map(|invocation| Command::Browser(Box::new(invocation)))
}

/// The app verb that each action sends.
fn verb(action: &str) -> Option<&'static str> {
    Some(match action {
        "open" => "browser.open",
        "navigate" => "browser.navigate",
        "list" => "browser.list",
        "read" => "browser.read",
        "close" => "browser.close",
        "eval" => "browser.eval",
        "click" => "browser.click",
        "type" => "browser.type",
        "fill" => "browser.fill",
        "press" => "browser.press",
        "select" => "browser.select",
        "hover" => "browser.hover",
        "scroll-into-view" => "browser.scrollIntoView",
        "check" | "uncheck" => "browser.setChecked",
        "is" => "browser.is",
        "wait" => "browser.wait",
        "wait-for" => "browser.waitFor",
        "wait-for-navigation" => "browser.waitForNavigation",
        "get-text" => "browser.getText",
        "get-html" => "browser.getHTML",
        "get-value" => "browser.getValue",
        "get-attribute" => "browser.getAttribute",
        "get-count" => "browser.getCount",
        "find" => "browser.find",
        "snapshot" => "browser.snapshot",
        "reload" => "browser.reload",
        "back" => "browser.back",
        "forward" => "browser.forward",
        "screenshot" => "browser.screenshot",
        "storage get" => "browser.storage.get",
        "storage set" => "browser.storage.set",
        "storage clear" => "browser.storage.clear",
        "cookies get" => "browser.cookies.get",
        "cookies set" => "browser.cookies.set",
        "cookies delete" => "browser.cookies.delete",
        "cookies clear" => "browser.cookies.clear",
        _ => return None,
    })
}

#[allow(clippy::too_many_lines)]
fn invocation(words: &[&str]) -> io::Result<Invocation> {
    let (action, rest) = match words {
        [group @ ("storage" | "cookies"), action, rest @ ..] => (format!("{group} {action}"), rest),
        [action, rest @ ..] => ((*action).to_owned(), rest),
        [] => return Err(unknown()),
    };
    let request = Invocation::new(verb(&action).ok_or_else(unknown)?);
    let usage = |shape: &str| format!("{action} {shape}");
    let simple = |shape: &str, count| Given::parse(rest, usage(shape), count, &[], &[]);
    Ok(match action.as_str() {
        "open" => {
            let given = Given::parse(
                rest,
                usage(OPEN),
                0..=1,
                &["--project", "--worktree", "--profile"],
                &["--split"],
            )?;
            request
                .optional("url", given.optional(0))
                .arg("split", given.flag("--split"))
                .optional("project", given.value("--project"))
                .optional("worktree", given.value("--worktree"))
                .optional("profile", given.value("--profile"))
                .output(Output::Text)
        }
        "list" => {
            let given = Given::parse(
                rest,
                usage("[--project PROJECT] [--worktree WORKTREE] [--json]"),
                0..=0,
                &["--project", "--worktree"],
                &["--json"],
            )?;
            request
                .optional("project", given.value("--project"))
                .optional("worktree", given.value("--worktree"))
                .output(if given.flag("--json") {
                    Output::Json
                } else {
                    Output::Tabs
                })
        }
        "read" => {
            let given = Given::parse(rest, usage("<tab-id> [--json]"), 1..=1, &[], &["--json"])?;
            request
                .arg("tabId", given.word(0))
                .output(if given.flag("--json") {
                    Output::Json
                } else {
                    Output::Page
                })
        }
        "navigate" => {
            let given = simple("<tab-id> <url>", 2..=2)?;
            request
                .arg("tabId", given.word(0))
                .arg("url", given.word(1))
                .output(Output::Ok)
        }
        "close" => {
            let given = simple("<tab-id>", 1..=1)?;
            request.arg("tabId", given.word(0)).output(Output::Ok)
        }
        "reload" | "back" | "forward" | "cookies clear" => {
            let given = simple("<tab-id>", 1..=1)?;
            request.arg("tabId", given.word(0))
        }
        "screenshot" => {
            let given = simple("<tab-id>", 1..=1)?;
            Invocation {
                extra_time: SCREENSHOT_TIME,
                ..request
                    .arg("tabId", given.word(0))
                    .output(Output::Screenshot)
            }
        }
        "eval" => {
            let given = simple("<tab-id> <javascript>", 2..=2)?;
            request
                .arg("tabId", given.word(0))
                .arg("script", given.word(1))
        }
        "click" | "hover" | "scroll-into-view" | "get-text" | "get-value" | "get-count" => {
            let given = simple("<tab-id> <selector>", 2..=2)?;
            request
                .arg("tabId", given.word(0))
                .arg("selector", given.word(1))
        }
        "check" | "uncheck" => {
            let given = simple("<tab-id> <selector>", 2..=2)?;
            request
                .arg("tabId", given.word(0))
                .arg("selector", given.word(1))
                .arg("checked", action == "check")
        }
        "fill" => {
            let given = simple("<tab-id> <selector> <text>", 3..=3)?;
            request
                .arg("tabId", given.word(0))
                .arg("selector", given.word(1))
                .arg("text", given.word(2))
        }
        "type" => {
            let given = Given::parse(
                rest,
                usage("<tab-id> <selector> <text> [--submit]"),
                3..=3,
                &[],
                &["--submit"],
            )?;
            request
                .arg("tabId", given.word(0))
                .arg("selector", given.word(1))
                .arg("text", given.word(2))
                .arg("submit", given.flag("--submit"))
        }
        "press" => {
            let given = simple("<tab-id> <key> [selector]", 2..=3)?;
            request
                .arg("tabId", given.word(0))
                .arg("key", given.word(1))
                .optional("selector", given.optional(2))
        }
        "select" => {
            let given = simple("<tab-id> <selector> <value>", 3..=3)?;
            request
                .arg("tabId", given.word(0))
                .arg("selector", given.word(1))
                .arg("value", given.word(2))
        }
        "is" => {
            let given = simple(
                "<tab-id> visible|hidden|enabled|disabled|checked <selector>",
                3..=3,
            )?;
            request
                .arg("tabId", given.word(0))
                .arg(
                    "property",
                    given.choice(1, &["visible", "hidden", "enabled", "disabled", "checked"])?,
                )
                .arg("selector", given.word(2))
        }
        "get-attribute" => {
            let given = simple("<tab-id> <selector> <attribute>", 3..=3)?;
            request
                .arg("tabId", given.word(0))
                .arg("selector", given.word(1))
                .arg("attribute", given.word(2))
        }
        "get-html" | "snapshot" => {
            let given = simple("<tab-id> [selector]", 1..=2)?;
            request
                .arg("tabId", given.word(0))
                .optional("selector", given.optional(1))
        }
        "find" => {
            let given = simple("<tab-id> role|text|label|placeholder|testid <value>", 3..=3)?;
            request
                .arg("tabId", given.word(0))
                .arg(
                    "kind",
                    given.choice(1, &["role", "text", "label", "placeholder", "testid"])?,
                )
                .arg("value", given.word(2))
        }
        "wait" => {
            let given = Given::parse(
                rest,
                usage(WAIT),
                1..=1,
                &[
                    "--selector",
                    "--text",
                    "--url-contains",
                    "--function",
                    "--timeout-ms",
                ],
                &[],
            )?;
            request
                .arg("tabId", given.word(0))
                .optional("selector", given.value("--selector"))
                .optional("text", given.value("--text"))
                .optional("urlContains", given.value("--url-contains"))
                .optional("function", given.value("--function"))
                .optional(
                    "timeoutMs",
                    number(given.value("--timeout-ms"), "--timeout-ms")?,
                )
                .waiting(DEFAULT_WAIT)
        }
        "wait-for" => {
            let given = simple("<tab-id> <selector> [ms]", 2..=3)?;
            request
                .arg("tabId", given.word(0))
                .arg("selector", given.word(1))
                .optional("timeoutMs", number(given.optional(2), "ms")?)
                .waiting(DEFAULT_WAIT)
        }
        "wait-for-navigation" => {
            let given = Given::parse(
                rest,
                usage("<tab-id> [ms] [--timeout-ms N]"),
                1..=2,
                &["--timeout-ms"],
                &[],
            )?;
            let timeout = match (given.optional(1), given.value("--timeout-ms")) {
                (Some(_), Some(_)) => return Err(given.usage()),
                (Some(ms), None) => number(Some(ms), "ms")?,
                (None, flag) => number(flag, "--timeout-ms")?,
            };
            request
                .arg("tabId", given.word(0))
                .optional("timeoutMs", timeout)
                .waiting(DEFAULT_NAVIGATION_WAIT)
        }
        "storage get" => {
            let given = simple("<tab-id> <key> [local|session]", 2..=3)?;
            request
                .arg("tabId", given.word(0))
                .arg("key", given.word(1))
                .arg("kind", given.storage(2)?)
        }
        "storage set" => {
            let given = simple("<tab-id> <key> <value> [local|session]", 3..=4)?;
            request
                .arg("tabId", given.word(0))
                .arg("key", given.word(1))
                .arg("value", given.word(2))
                .arg("kind", given.storage(3)?)
        }
        "storage clear" => {
            let given = simple("<tab-id> [local|session]", 1..=2)?;
            request
                .arg("tabId", given.word(0))
                .arg("kind", given.storage(1)?)
        }
        "cookies get" => {
            let given = simple("<tab-id> [url]", 1..=2)?;
            request
                .arg("tabId", given.word(0))
                .optional("url", given.optional(1))
        }
        "cookies set" => {
            let given = Given::parse(
                rest,
                usage(SET_COOKIE),
                4..=4,
                &["--path", "--expires"],
                &["--secure", "--http-only"],
            )?;
            request
                .arg("tabId", given.word(0))
                .arg("name", given.word(1))
                .arg("value", given.word(2))
                .arg("domain", given.word(3))
                .optional("path", given.value("--path"))
                .optional("secure", given.flag("--secure").then_some(true))
                .optional("httpOnly", given.flag("--http-only").then_some(true))
                .optional("expires", number(given.value("--expires"), "--expires")?)
        }
        "cookies delete" => {
            let given = simple("<tab-id> <name> [domain]", 2..=3)?;
            request
                .arg("tabId", given.word(0))
                .arg("name", given.word(1))
                .optional("domain", given.optional(2))
        }
        _ => return Err(unknown()),
    })
}

const OPEN: &str = "[url] [--split] [--project PROJECT] [--worktree WORKTREE] [--profile PROFILE]";
const WAIT: &str = "<tab-id> [--selector SELECTOR] [--text TEXT] [--url-contains TEXT] \
                    [--function JAVASCRIPT] [--timeout-ms N]";
const SET_COOKIE: &str = "<tab-id> <name> <value> <domain> [--path PATH] [--secure] \
                          [--http-only] [--expires UNIX-SECONDS]";

/// The words after the action: positional words, options with their values,
/// and flags. Only a word that is exactly one of the action's options counts
/// as one, so other words starting with a dash stay positional, and `--`
/// ends options.
#[derive(Debug)]
struct Given<'a> {
    usage: String,
    words: Vec<&'a str>,
    values: BTreeMap<&'static str, &'a str>,
    flags: BTreeSet<&'static str>,
}

impl<'a> Given<'a> {
    fn parse(
        rest: &[&'a str],
        usage: String,
        count: RangeInclusive<usize>,
        values: &[&'static str],
        flags: &[&'static str],
    ) -> io::Result<Self> {
        let mut given = Self {
            usage,
            words: Vec::new(),
            values: BTreeMap::new(),
            flags: BTreeSet::new(),
        };
        let mut rest = rest.iter().copied();
        while let Some(word) = rest.next() {
            if word == "--" {
                given.words.extend(rest);
                break;
            }
            if let Some(option) = values.iter().copied().find(|option| *option == word) {
                let value = rest
                    .next()
                    .ok_or_else(|| invalid(&format!("missing value for {option}")))?;
                if given.values.insert(option, value).is_some() {
                    return Err(invalid(&format!("duplicate option {option}")));
                }
            } else if let Some(flag) = flags.iter().copied().find(|flag| *flag == word) {
                if !given.flags.insert(flag) {
                    return Err(invalid(&format!("duplicate option {flag}")));
                }
            } else {
                given.words.push(word);
            }
        }
        if count.contains(&given.words.len()) {
            Ok(given)
        } else {
            Err(given.usage())
        }
    }
    fn usage(&self) -> io::Error {
        invalid(&format!("usage: muxy browser {}", self.usage))
    }
    fn word(&self, index: usize) -> &'a str {
        self.words[index]
    }
    fn optional(&self, index: usize) -> Option<&'a str> {
        self.words.get(index).copied()
    }
    fn value(&self, option: &str) -> Option<&'a str> {
        self.values.get(option).copied()
    }
    fn flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }
    fn choice(&self, index: usize, choices: &[&str]) -> io::Result<&'a str> {
        Some(self.word(index))
            .filter(|word| choices.contains(word))
            .ok_or_else(|| self.usage())
    }
    /// Which web storage a `storage` action uses: `local` unless given.
    fn storage(&self, index: usize) -> io::Result<&'a str> {
        match self.optional(index) {
            None => Ok("local"),
            Some(_) => self.choice(index, &["local", "session"]),
        }
    }
}

fn number(text: Option<&str>, name: &str) -> io::Result<Option<u64>> {
    text.map(|text| {
        text.parse()
            .map_err(|_| invalid(&format!("invalid value for {name}")))
    })
    .transpose()
}

fn unknown() -> io::Error {
    invalid("unknown browser command; run muxy browser --help")
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn command(words: &[&str]) -> io::Result<Command> {
        parse(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }

    fn invocation(words: &[&str]) -> io::Result<Invocation> {
        match command(words)? {
            Command::Browser(invocation) => Ok(*invocation),
            other => Err(io::Error::other(format!("{other:?} is not a request"))),
        }
    }

    fn request(words: &[&str]) -> io::Result<(&'static str, Value)> {
        let invocation = invocation(words)?;
        Ok((invocation.verb, Value::Object(invocation.args)))
    }

    /// Every action, as a command line split at spaces, with the request it
    /// sends.
    const ACTIONS: &[(&str, &str, &str)] = &[
        ("open", "browser.open", r#"{"split":false}"#),
        (
            "open a.test --split --project App --worktree fix --profile Work",
            "browser.open",
            r#"{"url":"a.test","split":true,"project":"App","worktree":"fix","profile":"Work"}"#,
        ),
        (
            "navigate T a.test",
            "browser.navigate",
            r#"{"tabId":"T","url":"a.test"}"#,
        ),
        ("list", "browser.list", "{}"),
        (
            "list --project App --worktree fix --json",
            "browser.list",
            r#"{"project":"App","worktree":"fix"}"#,
        ),
        ("read T", "browser.read", r#"{"tabId":"T"}"#),
        ("close T", "browser.close", r#"{"tabId":"T"}"#),
        (
            "eval T 1+1",
            "browser.eval",
            r#"{"tabId":"T","script":"1+1"}"#,
        ),
        (
            "click T #go",
            "browser.click",
            r##"{"tabId":"T","selector":"#go"}"##,
        ),
        (
            "type T #q rust",
            "browser.type",
            r##"{"tabId":"T","selector":"#q","text":"rust","submit":false}"##,
        ),
        (
            "type T #q rust --submit",
            "browser.type",
            r##"{"tabId":"T","selector":"#q","text":"rust","submit":true}"##,
        ),
        (
            "fill T #q rust",
            "browser.fill",
            r##"{"tabId":"T","selector":"#q","text":"rust"}"##,
        ),
        (
            "press T Enter",
            "browser.press",
            r#"{"tabId":"T","key":"Enter"}"#,
        ),
        (
            "press T Enter #q",
            "browser.press",
            r##"{"tabId":"T","key":"Enter","selector":"#q"}"##,
        ),
        (
            "select T #s L",
            "browser.select",
            r##"{"tabId":"T","selector":"#s","value":"L"}"##,
        ),
        (
            "hover T #m",
            "browser.hover",
            r##"{"tabId":"T","selector":"#m"}"##,
        ),
        (
            "scroll-into-view T #m",
            "browser.scrollIntoView",
            r##"{"tabId":"T","selector":"#m"}"##,
        ),
        (
            "check T #ok",
            "browser.setChecked",
            r##"{"tabId":"T","selector":"#ok","checked":true}"##,
        ),
        (
            "uncheck T #ok",
            "browser.setChecked",
            r##"{"tabId":"T","selector":"#ok","checked":false}"##,
        ),
        (
            "is T visible #ok",
            "browser.is",
            r##"{"tabId":"T","property":"visible","selector":"#ok"}"##,
        ),
        ("wait T", "browser.wait", r#"{"tabId":"T"}"#),
        (
            "wait T --selector #a --text Done --url-contains /done --function ready --timeout-ms 250",
            "browser.wait",
            r##"{"tabId":"T","selector":"#a","text":"Done","urlContains":"/done","function":"ready","timeoutMs":250}"##,
        ),
        (
            "wait-for T #a",
            "browser.waitFor",
            r##"{"tabId":"T","selector":"#a"}"##,
        ),
        (
            "wait-for T #a 800",
            "browser.waitFor",
            r##"{"tabId":"T","selector":"#a","timeoutMs":800}"##,
        ),
        (
            "wait-for-navigation T",
            "browser.waitForNavigation",
            r#"{"tabId":"T"}"#,
        ),
        (
            "wait-for-navigation T 900",
            "browser.waitForNavigation",
            r#"{"tabId":"T","timeoutMs":900}"#,
        ),
        (
            "wait-for-navigation T --timeout-ms 700",
            "browser.waitForNavigation",
            r#"{"tabId":"T","timeoutMs":700}"#,
        ),
        (
            "get-text T h1",
            "browser.getText",
            r#"{"tabId":"T","selector":"h1"}"#,
        ),
        ("get-html T", "browser.getHTML", r#"{"tabId":"T"}"#),
        (
            "get-html T main",
            "browser.getHTML",
            r#"{"tabId":"T","selector":"main"}"#,
        ),
        (
            "get-value T #q",
            "browser.getValue",
            r##"{"tabId":"T","selector":"#q"}"##,
        ),
        (
            "get-attribute T a href",
            "browser.getAttribute",
            r#"{"tabId":"T","selector":"a","attribute":"href"}"#,
        ),
        (
            "get-count T li",
            "browser.getCount",
            r#"{"tabId":"T","selector":"li"}"#,
        ),
        (
            "find T role button",
            "browser.find",
            r#"{"tabId":"T","kind":"role","value":"button"}"#,
        ),
        ("snapshot T", "browser.snapshot", r#"{"tabId":"T"}"#),
        (
            "snapshot T form",
            "browser.snapshot",
            r#"{"tabId":"T","selector":"form"}"#,
        ),
        ("reload T", "browser.reload", r#"{"tabId":"T"}"#),
        ("back T", "browser.back", r#"{"tabId":"T"}"#),
        ("forward T", "browser.forward", r#"{"tabId":"T"}"#),
        ("screenshot T", "browser.screenshot", r#"{"tabId":"T"}"#),
        (
            "storage get T k",
            "browser.storage.get",
            r#"{"tabId":"T","key":"k","kind":"local"}"#,
        ),
        (
            "storage set T k v session",
            "browser.storage.set",
            r#"{"tabId":"T","key":"k","value":"v","kind":"session"}"#,
        ),
        (
            "storage clear T",
            "browser.storage.clear",
            r#"{"tabId":"T","kind":"local"}"#,
        ),
        ("cookies get T", "browser.cookies.get", r#"{"tabId":"T"}"#),
        (
            "cookies get T a.test",
            "browser.cookies.get",
            r#"{"tabId":"T","url":"a.test"}"#,
        ),
        (
            "cookies set T id 1 a.test",
            "browser.cookies.set",
            r#"{"tabId":"T","name":"id","value":"1","domain":"a.test"}"#,
        ),
        (
            "cookies set T id 1 a.test --path /app --secure --http-only --expires 1767225600",
            "browser.cookies.set",
            r#"{"tabId":"T","name":"id","value":"1","domain":"a.test","path":"/app","secure":true,"httpOnly":true,"expires":1767225600}"#,
        ),
        (
            "cookies delete T id",
            "browser.cookies.delete",
            r#"{"tabId":"T","name":"id"}"#,
        ),
        (
            "cookies delete T id a.test",
            "browser.cookies.delete",
            r#"{"tabId":"T","name":"id","domain":"a.test"}"#,
        ),
        (
            "cookies clear T",
            "browser.cookies.clear",
            r#"{"tabId":"T"}"#,
        ),
    ];

    #[test]
    fn every_action_sends_its_verb_and_arguments() -> Result<(), Box<dyn std::error::Error>> {
        for (line, verb, args) in ACTIONS {
            let words: Vec<_> = line.split(' ').collect();
            let args: Value = serde_json::from_str(args)?;
            assert_eq!(request(&words)?, (*verb, args), "{line}");
        }
        Ok(())
    }

    #[test]
    fn replies_print_by_action_and_waits_get_their_time() -> io::Result<()> {
        for (words, output) in [
            (vec!["open"], Output::Text),
            (vec!["list"], Output::Tabs),
            (vec!["list", "--json"], Output::Json),
            (vec!["read", "T"], Output::Page),
            (vec!["read", "T", "--json"], Output::Json),
            (vec!["navigate", "T", "a.test"], Output::Ok),
            (vec!["close", "T"], Output::Ok),
            (vec!["eval", "T", "1"], Output::Json),
            (vec!["screenshot", "T"], Output::Screenshot),
        ] {
            assert_eq!(invocation(&words)?.output, output, "{words:?}");
        }
        for (words, extra_time) in [
            (vec!["click", "T", "#a"], Duration::ZERO),
            (vec!["screenshot", "T"], SCREENSHOT_TIME),
            (vec!["wait", "T"], DEFAULT_WAIT),
            (
                vec!["wait", "T", "--timeout-ms", "100"],
                Duration::from_millis(100),
            ),
            (vec!["wait-for", "T", "#a"], DEFAULT_WAIT),
            (vec!["wait-for", "T", "#a", "90000"], automation::MAX_WAIT),
            (vec!["wait-for-navigation", "T"], DEFAULT_NAVIGATION_WAIT),
        ] {
            assert_eq!(invocation(&words)?.extra_time, extra_time, "{words:?}");
        }
        Ok(())
    }

    #[test]
    fn options_go_anywhere_and_only_known_ones_count() -> io::Result<()> {
        assert_eq!(
            request(&["wait", "--timeout-ms", "100", "T", "--selector", "#a"])?,
            request(&["wait", "T", "--selector", "#a", "--timeout-ms", "100"])?
        );
        assert_eq!(
            request(&["type", "--submit", "T", "#q", "rust"])?,
            request(&["type", "T", "#q", "rust", "--submit"])?
        );
        assert_eq!(
            request(&["eval", "T", "-1"])?,
            ("browser.eval", json!({"tabId": "T", "script": "-1"}))
        );
        assert_eq!(
            request(&["eval", "T", "--", "--help"])?,
            ("browser.eval", json!({"tabId": "T", "script": "--help"}))
        );
        assert_eq!(
            request(&["type", "T", "#q", "--", "--submit"])?,
            (
                "browser.type",
                json!({"tabId": "T", "selector": "#q", "text": "--submit", "submit": false})
            )
        );
        assert_eq!(
            request(&["wait", "T", "--text", "--selector"])?,
            ("browser.wait", json!({"tabId": "T", "text": "--selector"}))
        );
        Ok(())
    }

    #[test]
    fn help_is_asked_for_by_name_or_by_nothing() -> io::Result<()> {
        for words in [
            vec![],
            vec!["--help"],
            vec!["-h"],
            vec!["click", "--help"],
            vec!["storage", "get", "-h"],
        ] {
            assert_eq!(command(&words)?, Command::Usage(help::BROWSER), "{words:?}");
        }
        Ok(())
    }

    #[test]
    fn wrong_arguments_name_the_usage() {
        for words in [
            vec!["click", "T"],
            vec!["frobnicate", "T"],
            vec!["storage"],
            vec!["cookies", "eat", "T"],
            vec!["open", "a.test", "b.test"],
            vec!["open", "--project"],
            vec!["navigate", "T"],
            vec!["list", "T"],
            vec!["read"],
            vec!["navigate", "T", "a.test", "--json"],
            vec!["is", "T", "shiny", "#a"],
            vec!["find", "T", "css", "#a"],
            vec!["storage", "get", "T", "k", "disk"],
            vec!["wait-for", "T", "#a", "soon"],
            vec!["wait", "T", "--timeout-ms", "-5"],
            vec!["wait", "T", "--text", "a", "--text", "b"],
            vec!["wait-for-navigation", "T", "100", "--timeout-ms", "200"],
            vec!["type", "T", "#q", "a", "--submit", "--submit"],
            vec!["cookies", "set", "T", "id", "1"],
            vec![
                "cookies",
                "set",
                "T",
                "id",
                "1",
                "a.test",
                "--expires",
                "soon",
            ],
        ] {
            assert!(command(&words).is_err(), "{words:?}");
        }
    }
}
