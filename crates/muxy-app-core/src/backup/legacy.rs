use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{ImportReport, Result, settings_source};
use crate::settings::{CustomCommand, KeyChord, Keymap, Settings};

const MAPPINGS: &[(&str, &[&str])] = &[
    ("muxy.theme.dark", &["appearance", "dark_theme"]),
    ("muxy.theme.light", &["appearance", "light_theme"]),
    ("muxy.showStatusBar", &["appearance", "status_bar_visible"]),
    ("muxy.tips.visible", &["appearance", "tips_visible"]),
    (
        "muxy.general.autoExpandWorktreesOnProjectSwitch",
        &["appearance", "auto_expand_worktrees"],
    ),
    (
        "muxy.worktrees.showUnreadIndicator",
        &["appearance", "worktree_show_unread"],
    ),
    (
        "muxy.general.autoCopyTerminalSelection",
        &["clipboard", "copy_on_select"],
    ),
    (
        "muxy.tabs.confirmCloseRunningProcess",
        &["window", "confirm_running_process"],
    ),
    (
        "muxy.general.defaultWorktreePathTemplate",
        &["worktrees", "default_location", "path_template"],
    ),
    (
        "muxy.general.defaultWorktreeParentPath",
        &["worktrees", "default_location", "parent_path"],
    ),
    (
        "muxy.projectPicker.defaultDirectory",
        &["projects", "search_root"],
    ),
    (
        "muxy.sidebarCollapsedStyle",
        &["appearance", "sidebar_collapsed_style"],
    ),
    ("muxy.activeSidebar", &["appearance", "extension_sidebar"]),
    ("muxy.localization", &["appearance", "language"]),
    ("muxy.appLayout", &["appearance", "layout"]),
    (
        "editor.richInputImageStrategy",
        &["composer", "image_strategy"],
    ),
    ("muxy.richInput.floating", &["composer", "pinned"]),
    ("muxy.quickTerminal.enabled", &["quick_terminal", "enabled"]),
    ("muxy.quickTerminal.width", &["quick_terminal", "width"]),
    ("muxy.quickTerminal.height", &["quick_terminal", "height"]),
    (
        "muxy.quickTerminal.transparency",
        &["quick_terminal", "transparency"],
    ),
    ("muxy.quickTerminal.blur", &["quick_terminal", "blur"]),
    ("shortcuts.quickTerminal", &["quick_terminal", "shortcut"]),
    (
        "muxy.richInput.presentationMode",
        &["composer", "presentation"],
    ),
    ("muxy.richInput.position", &["composer", "position"]),
    ("muxy.richInput.fontSize", &["composer", "font_size"]),
    ("muxy.richInput.broadcast", &["composer", "broadcast"]),
    (
        "muxy.richInput.clearAfterSending",
        &["composer", "clear_after_sending"],
    ),
    (
        "muxy.richInput.clearOnClose",
        &["composer", "clear_on_close"],
    ),
    ("editor.richInputFontFamily", &["composer", "font_family"]),
    (
        "editor.richInputLineHeightMultiplier",
        &["composer", "line_height"],
    ),
    ("muxy.recording.autoSend", &["composer", "voice_auto_send"]),
    ("muxy.recording.language", &["composer", "language"]),
    (
        "muxy.ai.repositoryActions.commit.provider",
        &["ai", "providers", "commit"],
    ),
    (
        "muxy.ai.repositoryActions.commit.prompt",
        &["ai", "prompts", "commit"],
    ),
    (
        "muxy.ai.repositoryActions.createPullRequest.provider",
        &["ai", "providers", "create_pr"],
    ),
    (
        "muxy.ai.repositoryActions.createPullRequest.prompt",
        &["ai", "prompts", "create_pr"],
    ),
];

pub fn import_settings(source: &[u8], current: &Settings) -> Result<(Settings, ImportReport)> {
    let values: Map<String, Value> = serde_json::from_slice(source)?;
    let mut document: toml::Value = toml::from_str(&settings_source(current)?)?;
    let mut report = ImportReport::default();
    for (key, value) in &values {
        if matches!(key.as_str(), "shortcuts.app" | "shortcuts.customCommands") {
            continue;
        }
        let mapped = MAPPINGS.iter().find(|(old, _)| *old == key);
        let Some((_, path)) = mapped else {
            report.skipped.push(key.clone());
            continue;
        };
        let result = (|| -> Result<toml::Value> {
            let mut candidate = document.clone();
            let value = match (key.as_str(), value) {
                ("muxy.appLayout", Value::String(value)) => Value::String(action_id(value)),
                ("editor.richInputImageStrategy", Value::String(value))
                    if value == "inlinePath" =>
                {
                    Value::String("inline_path".into())
                }
                ("muxy.richInput.floating", Value::Bool(value)) => Value::Bool(!value),
                _ => value.clone(),
            };
            set(&mut candidate, path, toml::Value::try_from(value)?);
            let settings: Settings = candidate.clone().try_into()?;
            settings.validate()?;
            settings.worktrees.default_location.validate()?;
            Ok(candidate)
        })();
        match result {
            Ok(candidate) => {
                document = candidate;
                report.imported += 1;
            }
            Err(_) => report.skipped.push(key.clone()),
        }
    }
    let mut settings: Settings = document.try_into()?;
    if let Some(bindings) = values.get("shortcuts.app") {
        import_bindings(bindings, &mut settings, &mut report)?;
    }
    if let Some(commands) = values.get("shortcuts.customCommands") {
        import_commands(commands, &mut settings, &mut report)?;
    }
    settings.validate()?;
    Ok((settings, report))
}

fn set(document: &mut toml::Value, path: &[&str], value: toml::Value) {
    let Some((key, rest)) = path.split_first() else {
        return;
    };
    let Some(table) = document.as_table_mut() else {
        return;
    };
    if rest.is_empty() {
        table.insert((*key).into(), value);
    } else {
        set(
            table
                .entry((*key).to_owned())
                .or_insert_with(|| toml::Value::Table(toml::Table::new())),
            rest,
            value,
        );
    }
}

fn action_id(old: &str) -> String {
    match old {
        "openProject" | "newProject" => "add_project".into(),
        "findInTerminal" => "find".into(),
        "toggleMaximizePane" => "toggle_zoom_pane".into(),
        "toggleRichInput" => "composer.toggle".into(),
        "toggleComposerVoice" => "composer.voice".into(),
        "submitRichInput" => "composer.submit".into(),
        "submitRichInputWithoutReturn" => "composer.insert".into(),
        _ => old
            .chars()
            .flat_map(|ch| {
                if ch.is_ascii_uppercase() {
                    vec!['_', ch.to_ascii_lowercase()]
                } else {
                    vec![ch]
                }
            })
            .collect(),
    }
}

fn chord(value: &Value) -> Result<KeyChord> {
    let combo: muxy_core::quick_terminal::keys::KeyCombo = serde_json::from_value(value.clone())?;
    Ok(combo
        .keystroke()
        .ok_or("Shortcut is not supported")?
        .parse()?)
}

fn import_bindings(
    value: &Value,
    settings: &mut Settings,
    report: &mut ImportReport,
) -> Result<()> {
    let bindings = value.as_object().ok_or("shortcuts.app must be an object")?;
    let mut overrides = settings.keymap.overrides().clone();
    let mut imported = Vec::new();
    for (action, value) in bindings {
        let id = action_id(action);
        if muxy_core::shortcuts::find(&id).is_none() {
            report.skipped.push(format!("shortcuts.app.{action}"));
            continue;
        }
        match chord(value) {
            Ok(chord) => {
                overrides.insert(id, chord.to_string());
                imported.push(action);
            }
            Err(_) => report.skipped.push(format!("shortcuts.app.{action}")),
        }
    }
    match serde_json::from_value::<Keymap>(serde_json::to_value(overrides)?) {
        Ok(keymap) => {
            settings.keymap = keymap;
            report.imported += imported.len();
        }
        Err(_) => report.skipped.extend(
            imported
                .into_iter()
                .map(|id| format!("shortcuts.app.{id} (conflicting shortcut)")),
        ),
    }
    Ok(())
}

fn import_commands(
    value: &Value,
    settings: &mut Settings,
    report: &mut ImportReport,
) -> Result<()> {
    let commands = value
        .get("shortcuts")
        .and_then(Value::as_array)
        .ok_or("Custom commands must contain a shortcuts list")?;
    let prefix = value
        .get("prefixCombo")
        .and_then(|value| value.get("key"))
        .and_then(Value::as_str)
        .is_some_and(|key| !key.is_empty());
    for value in commands {
        let mut candidate = settings.clone();
        let result = (|| -> Result<Settings> {
            let command = CustomCommand {
                id: value["id"]
                    .as_str()
                    .ok_or("Missing command ID")?
                    .to_lowercase(),
                name: value["name"]
                    .as_str()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or("Command")
                    .into(),
                command: value["command"].as_str().ok_or("Missing command")?.into(),
            };
            command.validate()?;
            if !prefix {
                candidate.keymap = candidate
                    .keymap
                    .with_binding(&command.shortcut_id(), Some(chord(&value["combo"])?))?;
            }
            candidate
                .commands
                .retain(|existing| existing.id != command.id);
            candidate.commands.push(command);
            candidate.validate()?;
            Ok(candidate)
        })();
        match result {
            Ok(candidate) => {
                *settings = candidate;
                report.imported += 1;
            }
            Err(_) => report.skipped.push(format!(
                "command {}",
                value["name"].as_str().unwrap_or("unknown")
            )),
        }
    }
    if prefix && !commands.is_empty() {
        report
            .skipped
            .push("Custom command prefix shortcuts (commands imported without shortcuts)".into());
    }
    Ok(())
}

pub(super) fn fallback_bindings(source: &[u8]) -> Result<Value> {
    let bindings: Vec<Value> = serde_json::from_slice(source)?;
    Ok(Value::Object(
        bindings
            .into_iter()
            .filter_map(|binding| {
                Some((binding["action"].as_str()?.into(), binding["combo"].clone()))
            })
            .collect(),
    ))
}

pub fn merge_legacy_files(
    settings: Option<&[u8]>,
    bindings: Option<&[u8]>,
    commands: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut values: BTreeMap<String, Value> = settings
        .map(serde_json::from_slice)
        .transpose()?
        .unwrap_or_default();
    if let Some(source) = bindings
        && !values.contains_key("shortcuts.app")
    {
        values.insert("shortcuts.app".into(), fallback_bindings(source)?);
    }
    if let Some(source) = commands
        && !values.contains_key("shortcuts.customCommands")
    {
        values.insert(
            "shortcuts.customCommands".into(),
            serde_json::from_slice(source)?,
        );
    }
    Ok(serde_json::to_vec(&values)?)
}
