//! Runtime consent prompts and extension dialogs. Both are native sheets on the
//! main window, so they share one queue and at most one is on screen.

use std::time::Duration;

use gpui::{Context, Window};
use muxy_app_core::extensions::{Choice, Consent, Gate, Request};
use muxy_ui::dialog::ConsentResponse;
use serde_json::{Value, json};

use super::{AppModel, Call};

const PROMPT_TIMEOUT: Duration = Duration::from_secs(60);
const PROMPTS_PER_EXTENSION: usize = 5;
const MAX_TEXT: usize = 2000;
const MAX_WAITING: usize = 64;

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

impl AppModel {
    fn consent(&self, owner: &str, request: &Request) -> Consent {
        self.extensions
            .grants
            .as_ref()
            .map_or(Consent::Deny, |grants| grants.decision(owner, request))
    }

    /// Runs a gated call now, rejects it, or queues a prompt, per the saved rules.
    pub(super) fn gate(
        &mut self,
        mut call: Call,
        request: Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.consent(&call.owner, &request) {
            Consent::Allow => {
                call.approved = true;
                self.dispatch_extension(call, window, cx);
            }
            Consent::Deny | Consent::Blocked => {
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
            call.reply.send(Err("extension call expired".into()), cx);
            return;
        }
        match self.consent(&call.owner, &request) {
            Consent::Allow => {
                call.approved = true;
                self.dispatch_extension(call, window, cx);
                return;
            }
            Consent::Deny | Consent::Blocked => {
                call.reply.send(Err(denial(&call.verb, &request)), cx);
                return;
            }
            Consent::Ask => (),
        }
        let message = format!(
            "{} {}.\n\n{}\n\n\u{201c}Remember\u{201d} saves the rule: {}",
            call.owner,
            request.gate.purpose(),
            request
                .details
                .iter()
                .map(|line| clamp(line))
                .collect::<Vec<_>>()
                .join("\n"),
            request.scope()
        );
        let (sender, receiver) = async_channel::bounded(1);
        let sheet = muxy_ui::dialog::consent(
            window,
            &format!("Allow {}?", call.owner),
            &message,
            &format!("Block all {} from this extension", request.gate.kind()),
            move |response| {
                let _ = sender.try_send(response);
            },
        );
        match sheet {
            Ok(sheet) => {
                self.extensions.sheet = Some(Sheet {
                    id,
                    owner: call.owner.clone(),
                    dialog: false,
                    _handle: Box::new(sheet),
                });
            }
            Err(error) => {
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
                    if model
                        .extensions
                        .sheet
                        .as_ref()
                        .is_some_and(|sheet| sheet.id == id)
                    {
                        model.extensions.sheet = None;
                    }
                    match remembered {
                        Ok(snapshot) => {
                            if let Some(snapshot) = snapshot {
                                model.receive_extension_snapshot(snapshot, cx);
                            }
                            if choice.allows() && model.call_live(&call, cx) {
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
                self.extensions.sheet = Some(Sheet {
                    id,
                    owner: call.owner.clone(),
                    dialog: true,
                    _handle: handle,
                });
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
            let mut buttons = vec!["OK".to_owned()];
            if !message.is_empty() {
                buttons.push("Copy".into());
            }
            (buttons, None, None, None)
        }
        "dialog.prompt" => {
            if title.is_empty() && message.is_empty() {
                return Err("prompt requires title or message".into());
            }
            let confirm = text("confirm").unwrap_or_else(|| "OK".into());
            let cancel = text("cancel").unwrap_or_else(|| "Cancel".into());
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
            let mut buttons: Vec<String> = args["buttons"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(clamp)
                .filter(|label| !label.is_empty())
                .collect();
            if buttons.is_empty() {
                buttons = vec!["OK".into(), "Cancel".into()];
            }
            buttons.truncate(3);
            let default = text("default");
            if let Some(index) = default
                .as_ref()
                .and_then(|label| buttons.iter().position(|button| button == label))
            {
                let label = buttons.remove(index);
                buttons.insert(0, label);
            }
            (buttons, None, text("cancel"), None)
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

fn dialog_result(verb: &str, args: &Value, value: Option<String>, cx: &mut gpui::App) -> Value {
    match verb {
        "dialog.alert" => {
            if value.as_deref() == Some("Copy") {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                    args["message"].as_str().unwrap_or("").to_owned(),
                ));
            }
            Value::Null
        }
        "dialog.confirm" if value.is_some() && value.as_deref() == args["cancel"].as_str() => {
            Value::Null
        }
        "dialog.prompt" => json!(value.map(|text| clamp(&text))),
        _ => json!(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialogs_follow_main_button_rules() {
        let options = dialog_options(
            "dialog.confirm",
            &json!({"title":"Delete?","buttons":["Delete","", "Cancel","Keep","More"],"default":"Cancel","cancel":"Cancel"}),
        )
        .expect("confirm");
        assert_eq!(options.buttons, ["Cancel", "Delete", "Keep"]);
        assert_eq!(options.cancel_button.as_deref(), Some("Cancel"));
        assert_eq!(
            dialog_options("dialog.confirm", &json!({})).unwrap_err(),
            "dialog requires title or message"
        );
        assert_eq!(
            dialog_options("dialog.alert", &json!({})).unwrap_err(),
            "alert requires title or message"
        );
        let alert = dialog_options("dialog.alert", &json!({"message":"copy me"})).expect("alert");
        assert_eq!(alert.buttons, ["OK", "Copy"]);
        let prompt = dialog_options("dialog.prompt", &json!({"title":"Name","default":"a"}))
            .expect("prompt");
        assert_eq!(prompt.buttons, ["OK", "Cancel"]);
        assert_eq!(prompt.input, Some(("a".into(), String::new())));
        let long =
            dialog_options("dialog.confirm", &json!({"title":"x".repeat(3000)})).expect("long");
        assert_eq!(long.title.chars().count(), MAX_TEXT);
    }

    #[test]
    fn denials_use_main_messages() {
        let request = |verb: &str, args: Value| {
            Request::for_call("ext", verb, &args, "/repo").expect("gated")
        };
        assert_eq!(
            denial("exec", &request("exec", json!({"argv":["ls"]}))),
            "exec: user denied consent for exec"
        );
        assert_eq!(
            denial("git.push", &request("git.push", json!({}))),
            "user denied consent for git.push"
        );
        assert_eq!(
            denial(
                "files.delete",
                &request("files.delete", json!({"paths":["a"]}))
            ),
            "user denied consent for files.delete"
        );
        assert_eq!(
            denial(
                "http.fetch",
                &Request::http("api.github.com", "GET", "https://api.github.com")
            ),
            "http request failed: user denied consent for api.github.com"
        );
        assert_eq!(
            denial("panes.send", &request("panes.send", json!({"paneID":"p"}))),
            "user denied consent for panes.send"
        );
    }
}
