//! Runtime consent prompts and extension dialogs. Both are native sheets on the
//! main window, so they share one queue and at most one is on screen.

use std::time::{Duration, SystemTime};

use gpui::{Context, SharedString, Window};
use muxy_app_core::extensions::{AuditEntry, Choice, Consent, Gate, Request, Rule, timestamp};
use muxy_ui::dialog::ConsentResponse;
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::tr;
use serde_json::{Value, json};

use super::{AppModel, Call};

const PROMPT_TIMEOUT: Duration = Duration::from_secs(60);
const PROMPTS_PER_EXTENSION: usize = 5;
const MAX_TEXT: usize = 2000;
const MAX_WAITING: usize = 64;
/// Buttons a dialog gets when the extension names none. They are shown in the
/// app language but reported to the extension in English.
const DEFAULT_BUTTONS: [&str; 2] = [tr_key!("OK"), tr_key!("Cancel")];

pub(super) enum Waiting {
    Consent {
        id: u64,
        call: Call,
        request: Request,
    },
    Dialog(Call),
}

impl Waiting {
    fn call(&self) -> &Call {
        match self {
            Self::Consent { call, .. } | Self::Dialog(call) => call,
        }
    }

    pub(super) fn owner(&self) -> &str {
        &self.call().owner
    }

    pub(super) fn fail(self, error: &str, cx: &mut gpui::App) {
        match self {
            Self::Consent { call, .. } | Self::Dialog(call) => {
                call.reply.send(Err(error.into()), cx);
            }
        }
    }
}

/// The sheet currently on screen; dropping it dismisses the native window.
pub(super) struct Sheet {
    id: u64,
    pub(super) owner: String,
    dialog: bool,
    _handle: Box<dyn std::any::Any>,
}

impl Sheet {
    pub(super) fn new(id: u64, owner: &str, dialog: bool, handle: Box<dyn std::any::Any>) -> Self {
        Self {
            id,
            owner: owner.into(),
            dialog,
            _handle: handle,
        }
    }
}

fn denial(verb: &str, request: &Request) -> String {
    let value = request
        .pattern
        .split_once(':')
        .map_or("", |(_, value)| value);
    match request.gate {
        Gate::Exec => "exec: user denied consent for exec".into(),
        Gate::HttpFetch => format!("http request failed: user denied consent for {value}"),
        Gate::GitWrite => format!("user denied consent for git.{value}"),
        Gate::FilesWrite if verb == "projects.create" => {
            "user denied consent for projects.create".into()
        }
        Gate::FilesWrite => format!("user denied consent for files.{value}"),
        gate => format!("user denied consent for {}", gate.name()),
    }
}

/// What the audit log records for a prompt answer, as main names it.
fn decision(choice: Choice) -> &'static str {
    match choice {
        Choice::AllowAndRemember | Choice::Allow => "allow",
        Choice::Cancel | Choice::DenyAndRemember => "deny",
        Choice::Block => "blocked",
    }
}

fn choice(response: ConsentResponse) -> Choice {
    match response {
        ConsentResponse::AllowAndRemember => Choice::AllowAndRemember,
        ConsentResponse::Allow => Choice::Allow,
        ConsentResponse::Cancel => Choice::Cancel,
        ConsentResponse::DenyAndRemember => Choice::DenyAndRemember,
        ConsentResponse::Block => Choice::Block,
    }
}

fn clamp(text: &str) -> String {
    text.chars().take(MAX_TEXT).collect()
}

/// The prompt's opening sentence, whole per gate so a translation can place
/// the extension's name anywhere.
fn purpose(gate: Gate, owner: &str) -> SharedString {
    match gate {
        Gate::Exec => tr!("%@ wants to run a shell command.", owner),
        Gate::PanesSend => tr!("%@ wants to type into a terminal.", owner),
        Gate::PanesSendKeys => tr!("%@ wants to press keys in a terminal.", owner),
        Gate::PanesReadScreen => tr!("%@ wants to read terminal output.", owner),
        Gate::TabsOpenForeign => tr!("%@ wants to open another extension's tab.", owner),
        Gate::TabsRunCommand => tr!("%@ wants to open a terminal that runs a command.", owner),
        Gate::GitWrite => tr!("%@ wants to modify the git repository.", owner),
        Gate::FilesWrite => tr!("%@ wants to modify workspace files.", owner),
        Gate::HttpFetch => tr!("%@ wants to make a network request.", owner),
        Gate::ProjectsDelete => tr!("%@ wants to delete a project.", owner),
    }
}

fn block_label(gate: Gate) -> SharedString {
    match gate {
        Gate::Exec => tr!("Block all shell commands from this extension"),
        Gate::PanesSend => tr!("Block all terminal input from this extension"),
        Gate::PanesSendKeys => tr!("Block all terminal keystrokes from this extension"),
        Gate::PanesReadScreen => tr!("Block all terminal output reads from this extension"),
        Gate::TabsOpenForeign => tr!("Block all foreign tab opens from this extension"),
        Gate::TabsRunCommand => tr!("Block all auto-run terminal commands from this extension"),
        Gate::GitWrite => tr!("Block all git changes from this extension"),
        Gate::FilesWrite => tr!("Block all file changes from this extension"),
        Gate::HttpFetch => tr!("Block all network requests from this extension"),
        Gate::ProjectsDelete => tr!("Block all project deletions from this extension"),
    }
}

impl AppModel {
    /// What the saved rules decide for a gated call, and the deciding rule.
    /// Without readable rules every gated call is denied.
    fn consent(&self, owner: &str, request: &Request) -> (Consent, Option<String>) {
        match &self.extensions.grants {
            Ok(grants) => grants
                .rule(owner, request)
                .map_or((Consent::Ask, None), |rule| (rule.consent, Some(rule.id()))),
            Err(_) => (Consent::Deny, None),
        }
    }

    /// Appends a gated decision to the profile's audit log, as main does.
    /// `reason` tags a denial nobody answered: a timeout, a cancel, or too many
    /// waiting prompts.
    fn audit(
        &self,
        call: &Call,
        request: &Request,
        decision: &str,
        rule: Option<String>,
        reason: Option<&str>,
    ) {
        let entry = AuditEntry {
            timestamp: timestamp(SystemTime::now()),
            extension_id: call.owner.clone(),
            verb: request.gate.name().into(),
            payload_summary: reason.map_or_else(
                || request.summary.clone(),
                |reason| format!("{} [{reason}]", request.summary),
            ),
            decision: decision.into(),
            rule_id: rule,
            source: match request.gate {
                Gate::Exec => "exec",
                Gate::HttpFetch => "http",
                _ => "muxy-api",
            }
            .into(),
        };
        self.extensions.logs.audit(&self.extensions.audit, entry);
    }

    /// Rejects a queued prompt or dialog whose extension stopped.
    pub(super) fn fail_waiting(&self, waiting: Waiting, error: &str, cx: &mut gpui::App) {
        if let Waiting::Consent { call, request, .. } = &waiting {
            self.audit(call, request, "deny", None, Some("cancelled"));
        }
        waiting.fail(error, cx);
    }

    /// Runs a gated call now, rejects it, or queues a prompt, per the saved rules.
    pub(super) fn gate(
        &mut self,
        mut call: Call,
        request: Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (consent, rule) = self.consent(&call.owner, &request);
        match consent {
            Consent::Allow => {
                self.audit(&call, &request, "allow", rule, None);
                call.approved = true;
                self.dispatch_extension(call, window, cx);
            }
            Consent::Deny | Consent::Blocked => {
                self.audit(&call, &request, "deny", rule, None);
                call.reply.send(Err(denial(&call.verb, &request)), cx);
            }
            Consent::Ask => {
                let queued = self
                    .extensions
                    .waiting
                    .iter()
                    .filter(|waiting| {
                        matches!(waiting, Waiting::Consent { .. })
                            && waiting.call().owner == call.owner
                    })
                    .count()
                    + usize::from(
                        self.extensions
                            .sheet
                            .as_ref()
                            .is_some_and(|sheet| !sheet.dialog && sheet.owner == call.owner),
                    );
                if queued >= PROMPTS_PER_EXTENSION {
                    self.audit(&call, &request, "deny", None, Some("queue-flood"));
                    call.reply.send(Err(denial(&call.verb, &request)), cx);
                    return;
                }
                self.extensions.next += 1;
                let id = self.extensions.next;
                self.extensions
                    .waiting
                    .push_back(Waiting::Consent { id, call, request });
                cx.spawn(async move |model, cx| {
                    cx.background_executor().timer(PROMPT_TIMEOUT).await;
                    let _ = model.update(cx, |model, cx| model.expire_prompt(id, cx));
                })
                .detach();
                self.next_prompt(window, cx);
            }
        }
    }

    fn expire_prompt(&mut self, id: u64, cx: &mut Context<Self>) {
        let position = self.extensions.waiting.iter().position(
            |waiting| matches!(waiting, Waiting::Consent { id: queued, .. } if *queued == id),
        );
        if let Some(Waiting::Consent { call, request, .. }) =
            position.and_then(|index| self.extensions.waiting.remove(index))
        {
            self.audit(&call, &request, "deny", None, Some("timeout"));
            call.reply.send(Err(denial(&call.verb, &request)), cx);
        } else if self
            .extensions
            .sheet
            .as_ref()
            .is_some_and(|sheet| sheet.id == id)
        {
            self.extensions.sheet = None;
        }
    }

    pub(super) fn next_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while self.extensions.sheet.is_none() {
            let Some(waiting) = self.extensions.waiting.pop_front() else {
                return;
            };
            match waiting {
                Waiting::Dialog(call) => self.show_dialog(call, window, cx),
                Waiting::Consent { id, call, request } => {
                    self.prompt(id, call, request, window, cx);
                }
            }
        }
    }

    fn prompt(
        &mut self,
        id: u64,
        mut call: Call,
        request: Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.call_live(&call, cx) {
            self.audit(&call, &request, "deny", None, Some("cancelled"));
            call.reply.send(Err("extension call expired".into()), cx);
            return;
        }
        let (consent, rule) = self.consent(&call.owner, &request);
        match consent {
            Consent::Allow => {
                self.audit(&call, &request, "allow", rule, None);
                call.approved = true;
                self.dispatch_extension(call, window, cx);
                return;
            }
            Consent::Deny | Consent::Blocked => {
                self.audit(&call, &request, "deny", rule, None);
                call.reply.send(Err(denial(&call.verb, &request)), cx);
                return;
            }
            Consent::Ask => (),
        }
        let details = request
            .details
            .iter()
            .map(|line| clamp(line))
            .collect::<Vec<_>>()
            .join("\n");
        let message = format!(
            "{}\n\n{details}\n\n{}",
            purpose(request.gate, &call.owner),
            tr!(
                "\u{201c}Remember\u{201d} saves the rule: %@",
                request.scope()
            )
        );
        let (sender, receiver) = async_channel::bounded(1);
        let sheet = muxy_ui::dialog::consent(
            window,
            &tr!("Allow %@?", &call.owner),
            &message,
            &block_label(request.gate),
            move |response| {
                let _ = sender.try_send(response);
            },
        );
        match sheet {
            Ok(sheet) => {
                self.extensions.sheet = Some(Sheet::new(id, &call.owner, false, Box::new(sheet)));
            }
            Err(error) => {
                self.audit(&call, &request, "deny", None, Some("cancelled"));
                call.reply.send(Err(error.to_string()), cx);
                return;
            }
        }
        self.await_consent(id, call, request, receiver, cx);
    }

    /// Applies the user's choice once it arrives. A remembered choice is saved
    /// only while the call is still live.
    pub(super) fn await_consent(
        &mut self,
        id: u64,
        call: Call,
        request: Request,
        receiver: async_channel::Receiver<ConsentResponse>,
        cx: &mut Context<Self>,
    ) {
        let handle = self.window;
        let local = self.extensions.local.clone();
        cx.spawn(async move |model, cx| {
            let choice = choice(receiver.recv().await.unwrap_or(ConsentResponse::Cancel));
            let live = model
                .update(cx, |model, cx| model.call_live(&call, cx))
                .unwrap_or(false);
            let remembered = if live
                && matches!(
                    choice,
                    Choice::AllowAndRemember | Choice::DenyAndRemember | Choice::Block
                ) {
                let owner = call.owner.clone();
                let rule = request.clone();
                local
                    .submit(move |state| {
                        state
                            .grants
                            .as_mut()
                            .map_err(|error| error.clone())?
                            .remember(&owner, &rule, choice)?;
                        Ok(Some(state.snapshot()))
                    })
                    .await
            } else {
                Ok(None)
            };
            let _ = handle.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| {
                    // A timeout or a stopped extension closes the sheet first,
                    // which also reports a cancel; only the user's own answer
                    // arrives while the sheet is still up.
                    let answered = model
                        .extensions
                        .sheet
                        .as_ref()
                        .is_some_and(|sheet| sheet.id == id);
                    if answered {
                        model.extensions.sheet = None;
                    }
                    match remembered {
                        Ok(snapshot) => {
                            if let Some(snapshot) = snapshot {
                                model.receive_extension_snapshot(snapshot, cx);
                            }
                            let live = model.call_live(&call, cx);
                            match (live, answered) {
                                (false, _) => {
                                    model.audit(&call, &request, "deny", None, Some("cancelled"));
                                }
                                (true, false) => {
                                    model.audit(&call, &request, "deny", None, Some("timeout"));
                                }
                                (true, true) => model.audit(
                                    &call,
                                    &request,
                                    decision(choice),
                                    request.rule(choice).as_ref().map(Rule::id),
                                    None,
                                ),
                            }
                            if choice.allows() && live {
                                let mut call = call;
                                call.approved = true;
                                model.dispatch_extension(call, window, cx);
                            } else {
                                call.reply.send(Err(denial(&call.verb, &request)), cx);
                            }
                        }
                        Err(error) => call.reply.send(Err(error), cx),
                    }
                    model.next_prompt(window, cx);
                });
            });
        })
        .detach();
    }

    /// `muxy.dialog.*`: at most one per extension, queued behind other sheets.
    pub(super) fn extension_dialog(
        &mut self,
        call: Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = dialog_options(&call.verb, &call.args) {
            call.reply.send(Err(error), cx);
            return;
        }
        let busy = self
            .extensions
            .sheet
            .as_ref()
            .is_some_and(|sheet| sheet.dialog && sheet.owner == call.owner)
            || self.extensions.waiting.iter().any(
                |waiting| matches!(waiting, Waiting::Dialog(queued) if queued.owner == call.owner),
            );
        if busy {
            call.reply.send(
                Err("a dialog is already open for this extension".into()),
                cx,
            );
        } else if self.extensions.sheet.is_some() {
            if self.extensions.waiting.len() >= MAX_WAITING {
                call.reply
                    .send(Err("too many pending extension dialogs".into()), cx);
            } else {
                self.extensions.waiting.push_back(Waiting::Dialog(call));
            }
        } else {
            self.show_dialog(call, window, cx);
        }
    }

    fn show_dialog(&mut self, call: Call, window: &mut Window, cx: &mut Context<Self>) {
        if !self.call_live(&call, cx) {
            call.reply.send(Err("extension call expired".into()), cx);
            return;
        }
        self.extensions.next += 1;
        let id = self.extensions.next;
        let (sender, receiver) = async_channel::bounded::<Option<String>>(1);
        let shown: std::io::Result<Box<dyn std::any::Any>> = if call.verb == "dialog.pickFolder" {
            let start = call.args["default"]
                .as_str()
                .map(expand_home)
                .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))
                .unwrap_or_else(|| "/".into());
            muxy_ui::dialog::choose_folder_with(
                &clamp(call.args["title"].as_str().unwrap_or("")),
                &clamp(call.args["message"].as_str().unwrap_or("")),
                &start,
                move |path| {
                    let _ = sender.try_send(path.map(|path| path.display().to_string()));
                },
            )
            .map(|picker| Box::new(picker) as Box<dyn std::any::Any>)
        } else {
            dialog_options(&call.verb, &call.args).map_or_else(
                |error| Err(std::io::Error::other(error)),
                |options| {
                    muxy_ui::dialog::present(window, options, move |value| {
                        let _ = sender.try_send(value);
                    })
                    .map(|sheet| Box::new(sheet) as Box<dyn std::any::Any>)
                },
            )
        };
        match shown {
            Ok(handle) => {
                self.extensions.sheet = Some(Sheet::new(id, &call.owner, true, handle));
            }
            Err(error) => {
                let error = if error.to_string().contains("parent") {
                    "no window available to present the dialog".into()
                } else {
                    error.to_string()
                };
                call.reply.send(Err(error), cx);
                return;
            }
        }
        let handle = self.window;
        cx.spawn(async move |model, cx| {
            let value = receiver.recv().await.ok().flatten();
            let _ = handle.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| {
                    if model
                        .extensions
                        .sheet
                        .as_ref()
                        .is_some_and(|sheet| sheet.id == id)
                    {
                        model.extensions.sheet = None;
                    }
                    if model.call_live(&call, cx) {
                        let result = dialog_result(&call.verb, &call.args, value, cx);
                        call.reply.send(Ok(result), cx);
                    } else {
                        call.reply.send(Err("extension call expired".into()), cx);
                    }
                    model.next_prompt(window, cx);
                });
            });
        })
        .detach();
    }
}

fn expand_home(path: &str) -> std::path::PathBuf {
    match (path.strip_prefix('~'), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            std::path::PathBuf::from(home).join(rest.trim_start_matches('/'))
        }
        _ => path.into(),
    }
}

/// Builds the native dialog, applying main's validation and 2000-character caps.
fn dialog_options(verb: &str, args: &Value) -> Result<muxy_ui::dialog::DialogOptions, String> {
    let text = |field: &str| args[field].as_str().map(clamp);
    let title = text("title").unwrap_or_default();
    let message = text("message").unwrap_or_default();
    let (buttons, default_button, cancel_button, input) = match verb {
        "dialog.pickFolder" => return Ok(muxy_ui::dialog::DialogOptions::default()),
        "dialog.alert" => {
            if title.is_empty() && args["message"].as_str().unwrap_or("").is_empty() {
                return Err("alert requires title or message".into());
            }
            let mut buttons = vec![tr!("OK").into()];
            if !message.is_empty() {
                buttons.push(tr!("Copy").into());
            }
            (buttons, None, None, None)
        }
        "dialog.prompt" => {
            if title.is_empty() && message.is_empty() {
                return Err("prompt requires title or message".into());
            }
            let confirm = text("confirm").unwrap_or_else(|| tr!("OK").into());
            let cancel = text("cancel").unwrap_or_else(|| tr!("Cancel").into());
            (
                vec![confirm, cancel.clone()],
                None,
                Some(cancel),
                Some((
                    text("default").unwrap_or_default(),
                    text("placeholder").unwrap_or_default(),
                )),
            )
        }
        _ => {
            if title.is_empty() && message.is_empty() {
                return Err("dialog requires title or message".into());
            }
            let mut buttons = extension_buttons(args);
            let defaults = buttons.is_empty();
            if defaults {
                buttons = Vec::from(DEFAULT_BUTTONS.map(|label| translate(label).into()));
            }
            buttons.truncate(3);
            let label =
                |field: &str| text(field).map(|label| if defaults { shown(label) } else { label });
            let default = label("default");
            if let Some(index) = default
                .as_ref()
                .and_then(|label| buttons.iter().position(|button| button == label))
            {
                let label = buttons.remove(index);
                buttons.insert(0, label);
            }
            (buttons, None, label("cancel"), None)
        }
    };
    Ok(muxy_ui::dialog::DialogOptions {
        title,
        message,
        buttons,
        default_button,
        cancel_button,
        input,
        style: if verb == "dialog.prompt" {
            String::new()
        } else {
            text("style").unwrap_or_default()
        },
    })
}

fn extension_buttons(args: &Value) -> Vec<String> {
    args["buttons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(clamp)
        .filter(|label| !label.is_empty())
        .collect()
}

/// A default button's English label, as the dialog shows it.
fn shown(label: String) -> String {
    DEFAULT_BUTTONS
        .into_iter()
        .find(|key| *key == label)
        .map_or(label, |key| translate(key).into())
}

/// A default button's shown label, as the extension expects it.
fn reported(label: String) -> String {
    DEFAULT_BUTTONS
        .into_iter()
        .find(|key| translate(key).as_ref() == label.as_str())
        .map_or(label, str::to_owned)
}

fn dialog_result(verb: &str, args: &Value, value: Option<String>, cx: &mut gpui::App) -> Value {
    match verb {
        "dialog.alert" => {
            if value.as_deref() == Some(tr!("Copy").as_ref()) {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                    args["message"].as_str().unwrap_or("").to_owned(),
                ));
            }
            Value::Null
        }
        "dialog.prompt" => json!(value.map(|text| clamp(&text))),
        "dialog.pickFolder" => json!(value),
        _ => {
            let value = if extension_buttons(args).is_empty() {
                value.map(reported)
            } else {
                value
            };
            if verb == "dialog.confirm"
                && value.is_some()
                && value.as_deref() == args["cancel"].as_str()
            {
                Value::Null
            } else {
                json!(value)
            }
        }
    }
}
