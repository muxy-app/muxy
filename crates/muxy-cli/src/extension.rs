//! `muxy extension`: manages the extensions of the desktop app on this
//! computer. Each command is one request to the app over its socket, except
//! `dev`, which keeps building and reloading one.

pub(crate) mod dev;

use std::error::Error;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::args::Command;
use crate::browser::call_app;
use crate::manage::help;

/// How much longer than a quick reply reading and restarting an extension,
/// which may have to ask its open pages first, can take.
const CHANGE_TIME: Duration = Duration::from_secs(30);
/// How long the user has to approve a publish, and Git and GitHub to do it.
const PUBLISH_TIME: Duration = Duration::from_secs(15 * 60);
const DEFAULT_LOG_LINES: u64 = 100;
const MAX_LOG_LINES: u64 = 10_000;

/// An extension command as the request it sends to the desktop app.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Invocation {
    verb: &'static str,
    args: Map<String, Value>,
    json: bool,
    extra_time: Duration,
}

pub(crate) fn parse(arguments: &[OsString]) -> io::Result<Command> {
    if arguments.is_empty()
        || arguments
            .iter()
            .any(|word| word == "--help" || word == "-h")
        || arguments.first().is_some_and(|word| word == "help")
    {
        return Ok(Command::Usage(help::EXTENSION));
    }
    let mut words: Vec<&str> = arguments
        .iter()
        .map(|word| {
            word.to_str()
                .ok_or_else(|| invalid("arguments must be UTF-8"))
        })
        .collect::<io::Result<_>>()?;
    if words.first() == Some(&"page") {
        return page(&words[1..]).map(|invocation| Command::Browser(Box::new(invocation)));
    }
    if words.first() == Some(&"dev") {
        return dev::parse(&words[1..]).map(Command::ExtensionDev);
    }
    if words.get(1) == Some(&"--") {
        words.remove(1);
    } else if words.first() != Some(&"list")
        && words.get(1).is_some_and(|word| word.starts_with('-'))
    {
        return Err(invalid(
            "expected a name or folder; use -- before one that starts with a dash",
        ));
    }
    let invocation = match words.as_slice() {
        ["list"] => request("extension.list", Map::new(), false),
        ["list", "--json"] => request("extension.list", Map::new(), true),
        ["status", name] => named("extension.status", name, false)?,
        ["status", name, "--json"] => named("extension.status", name, true)?,
        ["logs", name] => logs(name, DEFAULT_LOG_LINES)?,
        ["logs", name, "--lines", lines] => logs(name, line_count(lines)?)?,
        ["load", folder] => load(folder)?,
        ["enable", name] => changed("extension.enable", name)?,
        ["disable", name] => changed("extension.disable", name)?,
        ["reload", name] => changed("extension.reload", name)?,
        ["unload", name] => changed("extension.unload", name)?,
        ["validate", target] => validate(target, false)?,
        ["validate", target, "--json"] => validate(target, true)?,
        ["publish", name] => publish(name, false)?,
        ["publish", name, "--json"] => publish(name, true)?,
        ["pages", name] => named("extension.pages", name, false)?,
        ["pages", name, "--json"] => named("extension.pages", name, true)?,
        ["open", name, surface] => open(name, surface, None)?,
        ["open", name, surface, "--data", data] => open(name, surface, Some(data))?,
        ["run", name, command] => run_command(name, command)?,
        ["emit", name, event] => emit(name, event, None)?,
        ["emit", name, event, payload] => emit(name, event, Some(payload))?,
        _ => {
            return Err(invalid(
                "unknown extension command; run muxy extension --help",
            ));
        }
    };
    Ok(Command::Extension(Box::new(invocation)))
}

pub(crate) fn run(invocation: Invocation) -> Result<(), Box<dyn Error>> {
    let reply = call_app(
        invocation.verb,
        invocation.args,
        true,
        invocation.extra_time,
    )?;
    let mut stdout = io::stdout().lock();
    if invocation.json {
        writeln!(stdout, "{reply}")?;
    } else {
        write!(
            stdout,
            "{}",
            display_text(invocation.verb, &reply, crate::output::Display::stdout())
        )?;
    }
    let problems = problem_count(invocation.verb, &reply);
    if problems > 0 {
        return Err(format!("{problems} thing(s) to fix before publishing").into());
    }
    Ok(())
}

fn request(verb: &'static str, args: Map<String, Value>, json: bool) -> Invocation {
    Invocation {
        verb,
        args,
        json,
        extra_time: Duration::ZERO,
    }
}

fn name_argument(name: &str) -> io::Result<Map<String, Value>> {
    if name.is_empty() {
        return Err(invalid("expected an extension name"));
    }
    Ok(Map::from_iter([("name".to_owned(), json!(name))]))
}

fn named(verb: &'static str, name: &str, json: bool) -> io::Result<Invocation> {
    Ok(request(verb, name_argument(name)?, json))
}

fn changed(verb: &'static str, name: &str) -> io::Result<Invocation> {
    Ok(Invocation {
        extra_time: CHANGE_TIME,
        ..named(verb, name, false)?
    })
}

fn logs(name: &str, lines: u64) -> io::Result<Invocation> {
    let mut invocation = named("extension.logs", name, false)?;
    invocation.args.insert("lines".to_owned(), json!(lines));
    Ok(invocation)
}

fn line_count(lines: &str) -> io::Result<u64> {
    lines
        .parse()
        .ok()
        .filter(|lines| (1..=MAX_LOG_LINES).contains(lines))
        .ok_or_else(|| invalid("--lines must be a number from 1 to 10000"))
}

fn folder_argument(folder: &str) -> io::Result<String> {
    if folder.is_empty() {
        return Err(invalid("expected a folder"));
    }
    std::path::absolute(Path::new(folder))?
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("the folder's path must be UTF-8"))
}

fn load(folder: &str) -> io::Result<Invocation> {
    Ok(Invocation {
        verb: "extension.load",
        args: Map::from_iter([("path".to_owned(), json!(folder_argument(folder)?))]),
        json: false,
        extra_time: CHANGE_TIME,
    })
}

/// A bare word names a loaded extension, and anything with a path in it a folder.
fn validate(target: &str, json: bool) -> io::Result<Invocation> {
    let args = if target.contains('/') || target == "." || target == ".." {
        Map::from_iter([("path".to_owned(), json!(folder_argument(target)?))])
    } else {
        name_argument(target)?
    };
    Ok(Invocation {
        extra_time: CHANGE_TIME,
        ..request("extension.validate", args, json)
    })
}

fn publish(name: &str, json: bool) -> io::Result<Invocation> {
    Ok(Invocation {
        extra_time: PUBLISH_TIME,
        ..named("extension.publish", name, json)?
    })
}

/// The `muxy browser` page actions an extension page takes too, and the
/// verb each sends.
const PAGE_VERBS: [(&str, &str); 24] = [
    ("browser.read", "extension.page.read"),
    ("browser.screenshot", "extension.page.screenshot"),
    ("browser.eval", "extension.page.eval"),
    ("browser.click", "extension.page.click"),
    ("browser.type", "extension.page.type"),
    ("browser.fill", "extension.page.fill"),
    ("browser.press", "extension.page.press"),
    ("browser.select", "extension.page.select"),
    ("browser.hover", "extension.page.hover"),
    ("browser.scrollIntoView", "extension.page.scrollIntoView"),
    ("browser.setChecked", "extension.page.setChecked"),
    ("browser.is", "extension.page.is"),
    ("browser.wait", "extension.page.wait"),
    ("browser.waitFor", "extension.page.waitFor"),
    ("browser.getText", "extension.page.getText"),
    ("browser.getHTML", "extension.page.getHTML"),
    ("browser.getValue", "extension.page.getValue"),
    ("browser.getAttribute", "extension.page.getAttribute"),
    ("browser.getCount", "extension.page.getCount"),
    ("browser.find", "extension.page.find"),
    ("browser.snapshot", "extension.page.snapshot"),
    ("browser.storage.get", "extension.page.storage.get"),
    ("browser.storage.set", "extension.page.storage.set"),
    ("browser.storage.clear", "extension.page.storage.clear"),
];

/// `muxy extension page <action> <page-id> …`: a `muxy browser` page action
/// aimed at an extension page.
fn page(words: &[&str]) -> io::Result<crate::browser::Invocation> {
    let as_page = |error: io::Error| {
        invalid(
            &error
                .to_string()
                .replace("muxy browser", "muxy extension page")
                .replace("<tab-id>", "<page-id>"),
        )
    };
    let unknown = || invalid("unknown page command; run muxy extension --help");
    let mut invocation = crate::browser::args::invocation(words).map_err(as_page)?;
    invocation.verb = PAGE_VERBS
        .iter()
        .find(|(browser, _)| *browser == invocation.verb)
        .map(|(_, page)| *page)
        .ok_or_else(unknown)?;
    let page = invocation.args.remove("tabId").ok_or_else(unknown)?;
    invocation.args.insert("pageId".to_owned(), page);
    Ok(invocation)
}

/// `panel:<id>`, `tab:<type>` or `modal:<entry>`.
fn open(name: &str, surface: &str, data: Option<&str>) -> io::Result<Invocation> {
    let (kind, id) = surface
        .split_once(':')
        .filter(|(kind, id)| matches!(*kind, "panel" | "tab" | "modal") && !id.is_empty())
        .ok_or_else(|| invalid("expected panel:<id>, tab:<type> or modal:<entry>"))?;
    let mut args = name_argument(name)?;
    args.insert("surface".to_owned(), json!(kind));
    args.insert("id".to_owned(), json!(id));
    if let Some(data) = data {
        let data: Value = serde_json::from_str(data).map_err(|_| invalid("--data must be JSON"))?;
        args.insert("data".to_owned(), data);
    }
    Ok(Invocation {
        extra_time: CHANGE_TIME,
        ..request("extension.open", args, false)
    })
}

fn run_command(name: &str, command: &str) -> io::Result<Invocation> {
    let mut invocation = named("extension.run", name, false)?;
    invocation.args.insert("command".to_owned(), json!(command));
    Ok(invocation)
}

fn emit(name: &str, event: &str, payload: Option<&str>) -> io::Result<Invocation> {
    let mut invocation = named("extension.emit", name, false)?;
    invocation.args.insert("event".to_owned(), json!(event));
    if let Some(payload) = payload {
        let payload: Value = serde_json::from_str(payload)
            .ok()
            .filter(Value::is_object)
            .ok_or_else(|| invalid("the payload must be a JSON object"))?;
        invocation.args.insert("payload".to_owned(), payload);
    }
    Ok(invocation)
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn strings<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a str> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

fn state(extension: &Value) -> &'static str {
    if extension.get("enabled").and_then(Value::as_bool) == Some(true) {
        "enabled"
    } else {
        "disabled"
    }
}

fn source(extension: &Value) -> &'static str {
    if extension.get("unpacked").and_then(Value::as_bool) == Some(true) {
        "unpacked"
    } else {
        "marketplace"
    }
}

fn list_row(extension: &Value) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\n",
        string(extension, "name"),
        string(extension, "version"),
        state(extension),
        source(extension),
        string(extension, "path"),
    )
}

fn display_text(verb: &str, reply: &Value, display: crate::output::Display) -> String {
    if !display.terminal || !matches!(verb, "extension.list" | "extension.status") {
        return text(verb, reply);
    }
    let extensions: Vec<_> = if verb == "extension.status" {
        vec![reply]
    } else {
        reply
            .get("extensions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .collect()
    };
    let rows: Vec<_> = extensions
        .iter()
        .map(|extension| {
            vec![
                string(extension, "name").into(),
                string(extension, "version").into(),
                state(extension).into(),
                source(extension).into(),
                string(extension, "path").into(),
            ]
        })
        .collect();
    let mut output = display.table(
        &["name", "version", "state", "source", "folder"],
        &rows,
        "No extensions loaded.",
    );
    for error in strings(reply, "load_errors") {
        let _ = writeln!(output, "load error: {}", crate::output::cell(&json!(error)));
    }
    if let Some(waiting) = waiting_prompts(reply) {
        let _ = writeln!(output, "{waiting}");
    }
    for line in strings(reply, "log") {
        let _ = writeln!(output, "{}", crate::output::cell(&json!(line)));
    }
    output
}

fn text(verb: &str, reply: &Value) -> String {
    let mut output = String::new();
    match verb {
        "extension.list" => {
            for extension in reply
                .get("extensions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                output.push_str(&list_row(extension));
            }
            for error in strings(reply, "load_errors") {
                let _ = writeln!(output, "load error: {error}");
            }
        }
        "extension.status" => {
            output.push_str(&list_row(reply));
            for error in strings(reply, "load_errors") {
                let _ = writeln!(output, "load error: {error}");
            }
            if let Some(waiting) = waiting_prompts(reply) {
                let _ = writeln!(output, "{waiting}");
            }
            for line in strings(reply, "log") {
                output.push_str(line);
                output.push('\n');
            }
        }
        "extension.logs" => {
            for line in strings(reply, "lines") {
                output.push_str(line);
                output.push('\n');
            }
        }
        "extension.load" => {
            output.push_str(string(reply, "name"));
            output.push('\n');
        }
        "extension.validate" => {
            let mut any = false;
            for problem in strings(reply, "problems") {
                any = true;
                output.push_str(problem);
                output.push('\n');
            }
            match github_gap(reply) {
                Some(gap) => {
                    let _ = writeln!(output, "{gap}");
                }
                None if !any => output.push_str("ready to publish\n"),
                None => {}
            }
        }
        "extension.publish" => {
            output.push_str(string(reply, "url"));
            output.push('\n');
        }
        "extension.pages" => {
            for page in reply.as_array().into_iter().flatten() {
                output.push_str(&page_row(page));
            }
        }
        "extension.open" => {
            output.push_str(string(reply, "pageId"));
            output.push('\n');
        }
        _ => output.push_str("ok\n"),
    }
    output
}

/// `pageId kind id instance loaded visible title`, tab-separated.
fn page_row(page: &Value) -> String {
    let flag = |key: &str, yes: &str, no: &str| {
        if page.get(key).and_then(Value::as_bool) == Some(true) {
            yes.to_owned()
        } else {
            no.to_owned()
        }
    };
    let field = |key: &str| {
        string(page, key)
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>()
    };
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        field("pageId"),
        field("kind"),
        field("id"),
        field("instance"),
        if page.get("error").is_some_and(Value::is_string) {
            format!("failed: {}", field("error"))
        } else {
            flag("loaded", "loaded", "loading")
        },
        flag("visible", "visible", "hidden"),
        field("title"),
    )
}

fn waiting_prompts(reply: &Value) -> Option<String> {
    let count = reply.get("waiting_prompts").and_then(Value::as_u64)?;
    (count > 0)
        .then(|| format!("waiting for the user to answer {count} permission prompt(s) in Muxy"))
}

/// What stops a package that is otherwise ready from being published.
fn github_gap(reply: &Value) -> Option<&'static str> {
    match string(reply, "github") {
        "missing" => {
            Some("publishing needs the GitHub CLI (gh): install it from https://cli.github.com")
        }
        "signed-out" => Some("the GitHub CLI (gh) isn't signed in: run gh auth login"),
        "unreachable" => Some("GitHub couldn't be reached, so the store wasn't checked"),
        _ => None,
    }
}

fn problem_count(verb: &str, reply: &Value) -> usize {
    if verb == "extension.validate" {
        strings(reply, "problems").count() + usize::from(github_gap(reply).is_some())
    } else {
        0
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(words: &[&str]) -> io::Result<Command> {
        parse(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }

    fn invocation(words: &[&str]) -> io::Result<Invocation> {
        match parsed(words)? {
            Command::Extension(invocation) => Ok(*invocation),
            other => Err(invalid(&format!("unexpected {other:?}"))),
        }
    }

    #[test]
    fn commands_become_app_requests() -> io::Result<()> {
        assert_eq!(invocation(&["list"])?.verb, "extension.list");
        assert!(invocation(&["list", "--json"])?.json);
        let reload = invocation(&["reload", "hello"])?;
        assert_eq!(reload.verb, "extension.reload");
        assert_eq!(reload.args["name"], "hello");
        assert_eq!(reload.extra_time, CHANGE_TIME);
        assert_eq!(
            invocation(&["logs", "hello"])?.args["lines"],
            DEFAULT_LOG_LINES
        );
        assert_eq!(
            invocation(&["logs", "hello", "--lines", "5"])?.args["lines"],
            5
        );
        assert_eq!(invocation(&["publish", "hello"])?.extra_time, PUBLISH_TIME);
        Ok(())
    }

    fn page_invocation(words: &[&str]) -> io::Result<crate::browser::Invocation> {
        match parsed(words)? {
            Command::Browser(invocation) => Ok(*invocation),
            other => Err(invalid(&format!("unexpected {other:?}"))),
        }
    }

    #[test]
    fn page_actions_take_a_page_instead_of_a_tab() -> io::Result<()> {
        let click = page_invocation(&["page", "click", "42", "#save"])?;
        assert_eq!(click.verb, "extension.page.click");
        assert_eq!(click.args["pageId"], "42");
        assert_eq!(click.args["selector"], "#save");
        assert!(!click.args.contains_key("tabId"));
        let shot = page_invocation(&["page", "screenshot", "42", "--out", "shot.png"])?;
        assert_eq!(shot.verb, "extension.page.screenshot");
        assert!(shot.out.as_deref().is_some_and(Path::is_absolute));
        assert_eq!(
            page_invocation(&["page", "storage", "set", "42", "k", "v"])?.verb,
            "extension.page.storage.set"
        );
        assert_eq!(
            page_invocation(&["page", "wait", "42", "--text", "Done"])?.args["text"],
            "Done"
        );
        for words in [
            vec!["page", "open", "https://example.com"],
            vec!["page", "navigate", "42", "https://example.com"],
            vec!["page", "cookies", "clear", "42"],
            vec!["page", "list"],
        ] {
            assert!(parsed(&words).is_err(), "{words:?}");
        }
        let usage = parsed(&["page", "click", "42"])
            .err()
            .map(|error| error.to_string());
        assert_eq!(
            usage.as_deref(),
            Some("usage: muxy extension page click <page-id> <selector>")
        );
        Ok(())
    }

    #[test]
    fn surfaces_commands_and_events_reach_the_app() -> io::Result<()> {
        let open = invocation(&["open", "hello", "panel:main", "--data", "{\"a\":1}"])?;
        assert_eq!(open.verb, "extension.open");
        assert_eq!(open.args["surface"], "panel");
        assert_eq!(open.args["id"], "main");
        assert_eq!(open.args["data"], json!({"a": 1}));
        assert_eq!(
            invocation(&["open", "hello", "modal:dialog.html"])?.args["id"],
            "dialog.html"
        );
        let run = invocation(&["run", "hello", "toggle"])?;
        assert_eq!(
            (run.verb, &run.args["command"]),
            ("extension.run", &json!("toggle"))
        );
        let emit = invocation(&["emit", "hello", "file.changed", "{\"path\":\"a\"}"])?;
        assert_eq!(emit.args["event"], "file.changed");
        assert_eq!(emit.args["payload"], json!({"path": "a"}));
        assert!(invocation(&["pages", "hello", "--json"])?.json);
        for words in [
            vec!["open", "hello", "panel"],
            vec!["open", "hello", "popover:menu"],
            vec!["open", "hello", "panel:main", "--data", "{"],
            vec!["emit", "hello", "file.changed", "[1]"],
            vec!["run", "hello"],
        ] {
            assert!(parsed(&words).is_err(), "{words:?}");
        }
        assert!(matches!(
            parsed(&["dev", "extensions/hello"])?,
            Command::ExtensionDev(_)
        ));
        Ok(())
    }

    #[test]
    fn pages_and_waiting_prompts_print_as_text() {
        let pages = json!([{
            "pageId": "7", "kind": "panel", "id": "main", "instance": null,
            "loaded": true, "visible": false, "title": "Hi\tthere\u{1b}[2J",
        }, {
            "pageId": "8", "kind": "tab", "id": "view", "error": "missing entry",
        }]);
        assert_eq!(
            text("extension.pages", &pages),
            "7\tpanel\tmain\t\tloaded\thidden\tHi there [2J\n8\ttab\tview\t\tfailed: missing entry\thidden\t\n"
        );
        assert_eq!(text("extension.open", &json!({"pageId": "7"})), "7\n");
        let status = json!({"name": "hello", "waiting_prompts": 2});
        assert!(text("extension.status", &status).contains("2 permission prompt(s)"));
        assert!(
            !text(
                "extension.status",
                &json!({"name": "hello", "waiting_prompts": 0})
            )
            .contains("prompt")
        );
    }

    #[test]
    fn folders_reach_the_app_as_absolute_paths() -> io::Result<()> {
        let load = invocation(&["load", "extensions/hello"])?;
        let path = load.args["path"].as_str().unwrap_or_default();
        assert!(Path::new(path).is_absolute(), "{path}");
        assert!(path.ends_with("extensions/hello"), "{path}");
        Ok(())
    }

    #[test]
    fn validate_takes_a_name_or_a_folder() -> io::Result<()> {
        assert_eq!(invocation(&["validate", "hello"])?.args["name"], "hello");
        assert!(
            invocation(&["validate", "./hello"])?
                .args
                .contains_key("path")
        );
        assert!(invocation(&["validate", "."])?.args.contains_key("path"));
        Ok(())
    }

    #[test]
    fn a_double_dash_allows_a_name_or_folder_that_starts_with_a_dash() -> io::Result<()> {
        assert_eq!(invocation(&["reload", "--", "-odd"])?.args["name"], "-odd");
        let load = invocation(&["load", "--", "-odd"])?;
        assert!(
            load.args["path"]
                .as_str()
                .unwrap_or_default()
                .ends_with("-odd")
        );
        assert!(parsed(&["reload", "-odd"]).is_err());
        Ok(())
    }

    #[test]
    fn bad_commands_are_refused() {
        for words in [
            vec!["install", "hello"],
            vec!["uninstall", "hello"],
            vec!["reload"],
            vec!["reload", "--json"],
            vec!["load", "--force"],
            vec!["logs", "hello", "--lines", "0"],
            vec!["logs", "hello", "--lines", "many"],
        ] {
            assert!(parsed(&words).is_err(), "{words:?}");
        }
    }

    #[test]
    fn help_and_host_rules_match_the_other_app_commands() -> io::Result<()> {
        assert_eq!(parsed(&[])?, Command::Usage(help::EXTENSION));
        assert_eq!(parsed(&["--help"])?, Command::Usage(help::EXTENSION));
        assert_eq!(
            parsed(&["reload", "--help"])?,
            Command::Usage(help::EXTENSION)
        );
        let host = ["--host", "example", "extension", "list"].map(OsString::from);
        assert!(crate::args::parse(&host).is_err());
        let topic = ["help", "extension"].map(OsString::from);
        assert_eq!(
            crate::args::parse(&topic)?.1,
            Command::Usage(help::EXTENSION)
        );
        Ok(())
    }

    #[test]
    fn replies_print_as_text() {
        let extension = json!({
            "name": "hello", "version": "0.1.0", "enabled": true,
            "unpacked": true, "path": "/work/hello",
        });
        assert_eq!(
            text(
                "extension.list",
                &json!({"extensions": [extension], "load_errors": ["bad"]})
            ),
            "hello\t0.1.0\tenabled\tunpacked\t/work/hello\nload error: bad\n"
        );
        assert_eq!(text("extension.reload", &json!(null)), "ok\n");
        assert_eq!(text("extension.load", &json!({"name": "hello"})), "hello\n");
        assert_eq!(
            text(
                "extension.validate",
                &json!({"problems": [], "github": "octocat"})
            ),
            "ready to publish\n"
        );
        let no_gh = json!({"problems": [], "github": "missing"});
        assert!(text("extension.validate", &no_gh).contains("GitHub CLI"));
        assert!(!text("extension.validate", &no_gh).contains("ready to publish"));
        assert_eq!(problem_count("extension.validate", &no_gh), 1);
        let signed_in = json!({"problems": [], "github": "octocat"});
        assert_eq!(problem_count("extension.validate", &signed_in), 0);
        assert_eq!(
            problem_count("extension.validate", &json!({"problems": ["a", "b"]})),
            2
        );
    }

    #[test]
    fn formatted_logs_escape_controls_without_changing_raw_output() {
        let line = "\u{1b}[2Jstatus\tchanged\r\nnext";
        let reply = json!({"name": "hello", "log": [line], "lines": [line]});
        let mut display = crate::output::Display::stdout();
        display.terminal = true;
        for verb in ["extension.list", "extension.status"] {
            let output = display_text(verb, &reply, display);
            assert!(!output.contains(line), "{verb}: {output:?}");
            assert!(output.ends_with("\\u{1b}[2Jstatus\\tchanged\\r\\nnext\n"));
        }
        let raw = format!("{line}\n");
        assert_eq!(display_text("extension.logs", &reply, display), raw);
        display.terminal = false;
        for verb in ["extension.list", "extension.status", "extension.logs"] {
            assert_eq!(display_text(verb, &reply, display), text(verb, &reply));
        }
        assert!(display_text("extension.status", &reply, display).ends_with(&raw));
    }

    #[test]
    fn every_command_in_the_authoring_skill_parses() {
        let skill = muxy_app_core::extensions::workspace::SKILL;
        let commands: Vec<&str> = skill
            .lines()
            .flat_map(|line| line.split('`'))
            .chain(skill.lines())
            .filter_map(|text| text.trim().strip_prefix("muxy extension "))
            .filter(|command| !command.contains('<') && !command.contains('`'))
            .collect();
        assert!(commands.len() > 12, "{commands:?}");
        for command in commands {
            let words: Vec<_> = command
                .split_whitespace()
                .map(|word| word.trim_matches('\''))
                .collect();
            assert!(parsed(&words).is_ok(), "muxy extension {command}");
        }
    }
}
