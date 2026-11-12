//! App-level verbs: the window's panels and layout (`ui`), built-in and
//! custom commands, the composer, project layouts, preferences, the
//! extension's own settings, the clipboard, and app details.

use gpui::{ClipboardItem, Context, Window};
use muxy_app_core::extensions::SettingKind;
use muxy_app_core::project_layouts::{Config, DIRECTORY, discover};
use muxy_app_core::settings::{AppLayout, InterfaceSize};
use muxy_protocol::ServerPath;
use serde_json::{Map, Value, json};

use super::{AppModel, Call};
use crate::model::preferences::Change;
use crate::views::settings::Category;

fn layout_name(layout: AppLayout) -> &'static str {
    match layout {
        AppLayout::ProjectFocused => "project",
        AppLayout::TabFocused => "tab",
        AppLayout::AgentsFocused => "agents",
    }
}

fn interface_size_name(size: InterfaceSize) -> &'static str {
    match size {
        InterfaceSize::Regular => "regular",
        InterfaceSize::Large => "large",
        InterfaceSize::ExtraLarge => "extraLarge",
        InterfaceSize::Huge => "huge",
    }
}

fn settings_category(section: &str) -> Result<Category, String> {
    Ok(match section {
        "general" => Category::General,
        "ai" => Category::Ai,
        "composer" => Category::Composer,
        "quickTerminal" => Category::QuickTerminal,
        "appearance" => Category::Appearance,
        "terminal" => Category::Terminal,
        "keyboard" => Category::Keyboard,
        "commands" => Category::Commands,
        "browser" => Category::Browser,
        "servers" => Category::Servers,
        "extensions" => Category::Extensions,
        "backup" => Category::Backup,
        "about" => Category::About,
        other => return Err(format!("unknown settings section '{other}'")),
    })
}

fn flag(value: &Value, key: &str) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| format!("{key} must be true or false"))
}

/// Whether `value` suits a setting declared with `kind`.
fn fits(kind: SettingKind, value: &Value) -> bool {
    match kind {
        SettingKind::String => value.is_string(),
        SettingKind::Bool => value.is_boolean(),
        SettingKind::Number => value.is_number(),
    }
}

impl AppModel {
    /// What `ui.state` reports, and `ui.changed` watches.
    pub(super) fn ui_state(&self) -> Value {
        json!({
            "sidebar": self.appearance.sidebar_expanded,
            "agentsPanel": self.agents_panel_shown(),
            "agentSessions": self.appearance.agent_sessions_visible,
            "composer": self.composer.view.is_some(),
            "quickTerminal": self.quick.visible,
            "floatsHidden": self.state.floats_hidden(),
            "layout": layout_name(self.appearance.layout),
        })
    }

    /// The theme in use, as `app.info` and `theme.changed` report it.
    pub(super) fn theme_state(&self) -> Value {
        let name = self
            .themes
            .active_name(self.appearance.theme(self.dark), self.dark);
        json!({
            "name": name,
            "colorScheme": if self.dark { "dark" } else { "light" },
        })
    }

    /// The preferences `preferences.get` reports and `preferences.set` takes.
    pub(super) fn preference_values(&self) -> Value {
        json!({
            "theme.light": self.appearance.light_theme,
            "theme.dark": self.appearance.dark_theme,
            "interfaceSize": interface_size_name(self.appearance.interface_size),
            "sidebar": self.appearance.sidebar_expanded,
            "statusBar": self.appearance.status_bar_visible,
            "paneHeaders": self.appearance.pane_headers,
            "tips": self.appearance.tips_visible,
            "confirmProcess": self.settings.window.confirm_running_process,
            "terminal.fontSize": self.terminal.font_size,
            "terminal.fontFamily": self.terminal.font_families.first(),
        })
    }

    pub(super) fn ui_call(
        &mut self,
        call: &Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let visible = call.args["visible"].as_bool();
        match call.verb.as_str() {
            "ui.state" => return Ok(self.ui_state()),
            "ui.setSidebar" => {
                if visible != Some(self.appearance.sidebar_expanded) {
                    self.toggle_sidebar(window, cx);
                }
            }
            "ui.setAgentsPanel" => {
                let shown = self.agents_panel_shown();
                match visible {
                    Some(true) | None if !shown => self.toggle_agents_panel(window, cx),
                    Some(false) | None if shown => self.hide_agents_panel(window, cx),
                    _ => {}
                }
            }
            "ui.setAgentSessions" => {
                if visible != Some(self.appearance.agent_sessions_visible) {
                    self.toggle_agent_sessions(window, cx);
                }
            }
            "ui.setLayout" => {
                let layout = match call.args["layout"].as_str().unwrap_or("") {
                    "project" => AppLayout::ProjectFocused,
                    "tab" => AppLayout::TabFocused,
                    "agents" => AppLayout::AgentsFocused,
                    other => return Err(format!("invalid layout '{other}'")),
                };
                self.set_layout(layout, cx);
            }
            "ui.openCommandPalette" => {
                if !matches!(
                    self.overlay,
                    Some(crate::views::overlays::Overlay::Commands(_))
                ) {
                    self.toggle_command_palette(window, cx);
                }
            }
            "ui.openSettings" => {
                let category =
                    settings_category(call.args["section"].as_str().unwrap_or("general"))?;
                self.show_settings_category(category, window, cx);
            }
            "ui.back" | "ui.forward" => {
                let forward = call.verb == "ui.forward";
                let possible = self.can_navigate(forward);
                if possible {
                    self.navigate(forward, cx);
                }
                return Ok(json!(possible));
            }
            "ui.toggleFloats" => self.toggle_floats(cx),
            "ui.quickTerminal" => self.quick_terminal_call(&call.args, cx)?,
            _ => return Err(format!("unsupported extension API: {}", call.verb)),
        }
        Ok(Value::Null)
    }

    fn quick_terminal_call(&mut self, args: &Value, cx: &mut Context<Self>) -> Result<(), String> {
        if !self.settings.quick_terminal.enabled {
            return Err("the Quick Terminal is turned off".into());
        }
        match args["action"].as_str().unwrap_or("") {
            "toggle" => self.toggle_quick_terminal(cx),
            "show" if !self.quick.visible => self.toggle_quick_terminal(cx),
            "hide" if self.quick.visible => self.hide_quick_terminal(true, cx),
            "show" | "hide" => {}
            "pin" => {
                if args["pinned"].as_bool().unwrap_or(true) != self.settings.quick_terminal.pinned {
                    self.toggle_quick_terminal_pin(cx);
                }
            }
            other => return Err(format!("invalid action '{other}'")),
        }
        Ok(())
    }

    pub(super) fn commands_call(
        &mut self,
        call: &Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        use crate::views::workspace::commands;
        if call.verb != "commands.run" && call.verb != "commands.list" {
            return Err(format!("unsupported extension API: {}", call.verb));
        }
        if call.verb == "commands.list" {
            let enabled = self.can_run_custom_command();
            let custom = self.settings.commands.iter().map(|command| {
                json!({
                    "id": command.shortcut_id(),
                    "kind": "custom",
                    "title": command.name,
                    "enabled": enabled,
                    "shortcut": self.settings.keymap.binding(&command.shortcut_id()).map(ToString::to_string),
                })
            });
            return Ok(Value::Array(
                commands::extension_commands(self)
                    .into_iter()
                    .chain(custom)
                    .collect(),
            ));
        }
        let id = call.args["id"].as_str().unwrap_or("");
        if let Some(custom) = id.strip_prefix("command.") {
            if !self
                .settings
                .commands
                .iter()
                .any(|command| command.id == custom)
            {
                return Err(format!("command not found {id}"));
            }
            if !self.can_run_custom_command() || self.overlay.is_some() {
                return Err(format!("command {id} is not available now"));
            }
            self.run_custom_command(custom, cx);
            return Ok(Value::Null);
        }
        commands::run_extension_command(self, id, window, cx)
            .unwrap_or_else(|| Err(format!("command not found {id}")))
            .map(|()| Value::Null)
    }

    pub(super) fn composer_call(
        &mut self,
        call: &Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        match call.verb.as_str() {
            "composer.open" => {
                if self.composer.view.is_none() {
                    self.toggle_composer(window, cx);
                }
                if self.composer.view.is_none() {
                    return Err("the composer cannot open now".into());
                }
            }
            "composer.close" => {
                if self.composer.view.is_some() {
                    self.close_composer(cx);
                }
            }
            "composer.getDraft" => return Ok(json!(self.composer_text(cx))),
            "composer.setDraft" => {
                let text = call.args["text"].as_str().ok_or("text must be a string")?;
                self.set_composer_text(text.to_owned(), window, cx)?;
            }
            "composer.submit" | "composer.insert" => {
                self.send_composer(call.verb == "composer.submit", cx)?;
            }
            _ => return Err(format!("unsupported extension API: {}", call.verb)),
        }
        Ok(Value::Null)
    }

    pub(super) fn preferences_call(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        match call.verb.as_str() {
            "preferences.get" => Ok(self.preference_values()),
            "preferences.themes" => Ok(Value::Array(
                self.themes
                    .entries
                    .iter()
                    .map(|entry| json!({"name": entry.name}))
                    .collect(),
            )),
            "preferences.set" => {
                let key = call.args["key"].as_str().unwrap_or("");
                let change = self.preference_change(key, &call.args["value"])?;
                self.try_change_preference(change, cx)?;
                Ok(Value::Null)
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    fn preference_change(&self, key: &str, value: &Value) -> Result<Change, String> {
        Ok(match key {
            "theme.light" | "theme.dark" => {
                let name = value.as_str().ok_or("theme must be a string")?;
                if !self.themes.entries.iter().any(|entry| entry.name == name) {
                    return Err(format!("unknown theme '{name}'"));
                }
                Change::Theme(key == "theme.dark", name.to_owned())
            }
            "interfaceSize" => Change::InterfaceSize(match value.as_str().unwrap_or("") {
                "regular" => InterfaceSize::Regular,
                "large" => InterfaceSize::Large,
                "extraLarge" => InterfaceSize::ExtraLarge,
                "huge" => InterfaceSize::Huge,
                other => return Err(format!("invalid interface size '{other}'")),
            }),
            "sidebar" => Change::Sidebar(flag(value, key)?),
            "statusBar" => Change::StatusBar(flag(value, key)?),
            "paneHeaders" => Change::PaneHeaders(flag(value, key)?),
            "tips" => Change::Tips(flag(value, key)?),
            "confirmProcess" => Change::ConfirmProcess(flag(value, key)?),
            "terminal.fontSize" => {
                let size = value.as_f64().ok_or("terminal.fontSize must be a number")?;
                Change::Terminal("font-size", size.to_string())
            }
            "terminal.fontFamily" => {
                let family = value
                    .as_str()
                    .ok_or("terminal.fontFamily must be a string")?;
                Change::Terminal("font-family", family.to_owned())
            }
            other => return Err(format!("unknown preference '{other}'")),
        })
    }

    /// `settings.*`: the extension's own declared settings.
    pub(super) fn settings_call(&mut self, call: Call, cx: &mut Context<Self>) {
        let Some(declared) = self
            .extensions
            .registry
            .enabled(&call.owner)
            .map(|extension| extension.manifest.settings.clone())
        else {
            call.reply.send(Err("extension disabled".into()), cx);
            return;
        };
        let saved = self
            .extensions
            .settings
            .get(&call.owner)
            .cloned()
            .unwrap_or_default();
        let value = |setting: &muxy_app_core::extensions::Setting| {
            saved
                .get(&setting.key)
                .cloned()
                .unwrap_or_else(|| setting.default_value.clone())
        };
        let key = call.args["key"].as_str().unwrap_or("");
        let setting = declared.iter().find(|setting| setting.key == key);
        let reply = match (call.verb.as_str(), setting) {
            ("settings.all", _) => Ok(Value::Object(
                declared
                    .iter()
                    .map(|setting| (setting.key.clone(), value(setting)))
                    .collect::<Map<_, _>>(),
            )),
            ("settings.get" | "settings.set" | "settings.reset", None) => {
                Err(format!("setting '{key}' not declared in manifest"))
            }
            ("settings.get", Some(setting)) => Ok(value(setting)),
            ("settings.set", Some(setting)) if !fits(setting.kind, &call.args["value"]) => {
                Err(format!("setting '{key}' has another type"))
            }
            ("settings.set" | "settings.reset", Some(_)) => {
                let stored = (call.verb == "settings.set").then(|| call.args["value"].clone());
                let task = self.set_extension_setting(&call.owner, key, stored, cx);
                cx.spawn(async move |model, cx| {
                    let result = task.await.map(|()| Value::Null);
                    let _ = model.update(cx, |_, cx| call.reply.send(result, cx));
                })
                .detach();
                return;
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        };
        call.reply.send(reply, cx);
    }

    pub(super) fn clipboard_call(call: &Call, cx: &mut Context<Self>) -> Result<Value, String> {
        match call.verb.as_str() {
            "clipboard.readText" => {
                Ok(json!(cx.read_from_clipboard().and_then(|item| item.text())))
            }
            "clipboard.writeText" => {
                let text = call.args["text"].as_str().ok_or("text must be a string")?;
                cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
                Ok(Value::Null)
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    pub(super) fn app_call(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        match call.verb.as_str() {
            "app.info" => Ok(json!({
                "name": crate::app_name(),
                "version": env!("CARGO_PKG_VERSION"),
                "platform": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "language": self.appearance.language,
                "theme": self.theme_state(),
            })),
            "app.openExternal" => {
                let url = call.args["url"].as_str().unwrap_or("").trim();
                let scheme = url
                    .split_once("://")
                    .map(|(scheme, _)| scheme.to_ascii_lowercase());
                if !matches!(scheme.as_deref(), Some("http" | "https"))
                    || url.chars().any(|c| c.is_whitespace() || c.is_control())
                {
                    return Err("only http and https URLs can be opened".into());
                }
                cx.open_url(url);
                Ok(Value::Null)
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    /// `layouts.list` and `layouts.apply`: the project's `.muxy/layouts`,
    /// read on its server. Applying asks the user, as the layout picker does.
    pub(super) fn layouts_call(&mut self, call: Call, cx: &mut Context<Self>) {
        let Some(client) = self.call_client(&call) else {
            call.reply.send(Err("server is disconnected".into()), cx);
            return;
        };
        let project = call.project;
        let name = (call.verb == "layouts.apply")
            .then(|| call.args["name"].as_str().unwrap_or("").to_owned());
        let task = cx.background_executor().spawn(async move {
            let layouts = client
                .list_directory_async(project, ServerPath::from(DIRECTORY))
                .await
                .map(discover)
                .map_err(|error| error.to_string())?;
            let Some(name) = name else {
                return Ok((layouts, None));
            };
            let layout = layouts
                .iter()
                .find(|layout| layout.name == name)
                .cloned()
                .ok_or_else(|| format!("layout not found {name}"))?;
            let file = client
                .read_file_async(project, layout.path.clone())
                .await
                .map_err(|error| error.to_string())?;
            let config = Config::parse(&file.content)?;
            Ok((layouts, Some((layout.name, config))))
        });
        cx.spawn(async move |model, cx| {
            let result: Result<_, String> = task.await;
            let _ = model.update(cx, |model, cx| {
                if !model.call_live(&call, cx) {
                    call.reply.send(Err("extension call expired".into()), cx);
                    return;
                }
                let reply = result.and_then(|(layouts, chosen)| match chosen {
                    None => Ok(Value::Array(
                        layouts
                            .iter()
                            .map(|layout| {
                                json!({
                                    "name": layout.name,
                                    "path": String::from_utf8_lossy(&layout.path.0),
                                })
                            })
                            .collect(),
                    )),
                    Some((name, config)) => {
                        model.confirm_extension_layout(project, &name, config, cx)
                    }
                });
                call.reply.send(reply, cx);
            });
        })
        .detach();
    }

    fn confirm_extension_layout(
        &mut self,
        project: muxy_app_core::ProjectId,
        name: &str,
        config: Config,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let directory = self
            .state
            .project(project)
            .filter(|target| target.status() == muxy_app_core::ProjectStatus::Available)
            .map(|target| target.directory.clone())
            .ok_or("the project is not available")?;
        self.confirm_project_layout(project, directory, name, config, cx);
        Ok(Value::Null)
    }
}
