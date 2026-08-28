use super::{Category, Field, SettingsModal, SettingsPickerTarget, SliderSpec};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Corner, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, anchored, deferred, div, px,
};
use muxy_api::picker::path_service::{self, LocationStatus, PathService};
use muxy_core::prefs::ScalePreset;
use muxy_core::prefs::settings;
use muxy_core::settings_catalog;
use muxy_ui::components::SymbolGlyph;
use muxy_ui::controls::{self, Choice, Grab, Style};
use serde_json::Value;

mod ai;
mod appearance;
mod backup;
mod browser;
mod composer;
mod general;
mod mobile;
mod notifications;
mod projects;
mod quick_terminal;
mod remote_devices;
mod terminal;
mod voice;

const LOCALIZATION: &str = "muxy.localization";
const APP_LAYOUT: &str = "muxy.appLayout";
const TRANSPARENCY: &str = "muxy.app.transparency";
const BLUR: &str = "muxy.app.blur";
const BACKGROUND_STYLE: &str = "muxy.appBackgroundStyle";
const TAB_MAX_WIDTH: &str = "muxy.tabs.maxWidth";
const IDLE_THRESHOLD: &str = "muxy.terminalOffline.idleThresholdSeconds";
const OFFLINE_ENABLED: &str = "muxy.terminalOffline.enabled";
const PICKER_MODE: &str = "muxy.projectPicker.mode";
const SEARCH_LOCATION: &str = "muxy.projectPicker.defaultDirectory";
const SORT_MODE: &str = "muxy.projectSortMode";
const FILE_OPENER: &str = "muxy.defaultFileOpener";
const WORKTREE_TEMPLATE: &str = "muxy.general.defaultWorktreePathTemplate";
const WORKTREE_PARENT: &str = "muxy.general.defaultWorktreeParentPath";
const WORKTREE_MODE: &str = "worktree.location.mode";
const SUGGESTED_TEMPLATE: &str = "../{base-dir}.{branch}";

const TRANSPARENCY_SLIDER: SliderSpec = SliderSpec {
    key: TRANSPARENCY,
    min: 0.0,
    max: 55.0,
    integral: true,
    zero_at_maximum: false,
};

const BLUR_SLIDER: SliderSpec = SliderSpec {
    key: BLUR,
    min: 0.0,
    max: 100.0,
    integral: true,
    zero_at_maximum: false,
};

const TAB_WIDTH_SLIDER: SliderSpec = SliderSpec {
    key: TAB_MAX_WIDTH,
    min: 120.0,
    max: 400.0,
    integral: false,
    zero_at_maximum: true,
};

pub fn content(
    modal: &SettingsModal,
    category: Category,
    cx: &mut Context<SettingsModal>,
) -> Vec<AnyElement> {
    match category {
        Category::General => general::content(modal, cx),
        Category::Appearance => appearance::content(modal, cx),
        Category::Terminal => terminal::content(modal, cx),
        Category::Projects => projects::content(modal, cx),
        Category::Shortcuts => super::commands::shortcuts(modal),
        Category::Commands => super::commands::content(modal, cx),
        Category::Json => super::json_editor::content(modal, cx),
        Category::Browser => browser::content(modal, cx),
        Category::RichInput => composer::content(modal, cx),
        Category::Ai => ai::content(modal, cx),
        Category::Voice => voice::content(modal, cx),
        Category::Notifications => notifications::content(modal, cx),
        Category::QuickTerminal => quick_terminal::content(modal, cx),
        Category::Mobile => mobile::content(modal, cx),
        Category::RemoteDevices => remote_devices::content(modal),
        Category::Backup => backup::content(modal),
    }
}

pub fn fields(modal: &SettingsModal, category: Category) -> Vec<Field> {
    if category == Category::Commands {
        return super::commands::fields(modal);
    }
    if category == Category::Json {
        if !super::json_editor::is_user_pane(modal) {
            return Vec::new();
        }
        return vec![Field {
            id: super::json_editor::EDITOR_FIELD.to_owned(),
            value: settings::load_user_text(),
            placeholder: String::new(),
            monospaced: true,
            multiline: true,
        }];
    }
    if category != Category::Projects {
        return extra_fields(category);
    }
    match projects::worktree_mode(modal) {
        projects::WorktreeMode::Template => vec![Field {
            id: WORKTREE_TEMPLATE.to_owned(),
            value: settings::string_value(WORKTREE_TEMPLATE, ""),
            placeholder: SUGGESTED_TEMPLATE.to_owned(),
            monospaced: true,
            multiline: false,
        }],
        projects::WorktreeMode::Folder => vec![Field {
            id: WORKTREE_PARENT.to_owned(),
            value: settings::string_value(WORKTREE_PARENT, ""),
            placeholder: "/path/to/worktrees".to_owned(),
            monospaced: true,
            multiline: false,
        }],
        projects::WorktreeMode::Default => Vec::new(),
    }
}

pub fn commits_on_change(id: &str) -> bool {
    matches!(id, MOBILE_PORT | MOBILE_CAP)
}

pub fn commit_field(
    modal: &mut SettingsModal,
    id: &str,
    text: &str,
    cx: &mut Context<SettingsModal>,
) {
    if super::commands::commit_field(modal, id, text, cx) || commit_extra_field(modal, id, text, cx)
    {
        return;
    }
    if matches!(id, WORKTREE_TEMPLATE | WORKTREE_PARENT) {
        projects::persist_worktree_location(modal, projects::worktree_mode(modal), text, cx);
    }
}

fn visible(
    modal: &SettingsModal,
    category: Category,
    title: &str,
    footer: Option<&str>,
    shows_divider: bool,
    children: Vec<AnyElement>,
) -> Option<AnyElement> {
    if !settings_catalog::section_matches(modal.query(), category, title) {
        return None;
    }
    Some(controls::section(
        modal.style(),
        title,
        footer,
        shows_divider,
        children,
    ))
}

fn toggle_row(
    style: Style,
    label: &str,
    key: &'static str,
    default: bool,
    cx: &mut Context<SettingsModal>,
) -> AnyElement {
    let value = settings::bool_value(key, default);
    controls::row(
        style,
        label,
        controls::toggle(
            style,
            key,
            value,
            cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
                let next = !settings::bool_value(key, default);
                modal.write(key, Value::Bool(next), cx);
            }),
        ),
    )
}

fn picker_row(
    modal: &SettingsModal,
    label: &str,
    key: &'static str,
    default: &'static str,
    choices: Vec<Choice>,
    cx: &mut Context<SettingsModal>,
) -> AnyElement {
    let style = modal.style();
    let selected = settings::string_value(key, default);
    let popover = modal.picker(key).cloned();
    let toggle_choices = choices.clone();
    let toggle_selected = selected.clone();
    controls::row(
        style,
        label,
        controls::picker(
            style,
            key,
            choices,
            &selected,
            popover,
            cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
                modal.toggle_picker(
                    key,
                    toggle_choices.clone(),
                    toggle_selected.clone(),
                    SettingsPickerTarget::Setting,
                    cx,
                )
            }),
        ),
    )
}

fn segmented_row(
    modal: &SettingsModal,
    label: &str,
    key: &'static str,
    default: &'static str,
    choices: Vec<Choice>,
    cx: &mut Context<SettingsModal>,
) -> AnyElement {
    let style = modal.style();
    let selected = settings::string_value(key, default);
    controls::row(
        style,
        label,
        controls::segmented(
            style,
            key,
            choices,
            &selected,
            cx.listener(
                move |modal: &mut SettingsModal, value: &SharedString, _, cx| {
                    modal.write(key, Value::String(value.to_string()), cx);
                },
            ),
        ),
    )
}

fn appended_stored(mut choices: Vec<Choice>, selected: &str, label: String) -> Vec<Choice> {
    if selected.is_empty() || choices.iter().any(|choice| choice.value == selected) {
        return choices;
    }
    choices.push(Choice {
        value: selected.to_owned(),
        label,
        enabled: false,
    });
    choices
}

fn unavailable_label(selected: &str, fallback: &str) -> String {
    match selected.split_once(':') {
        Some((extension, member)) if !extension.is_empty() && !member.is_empty() => {
            format!("{extension} ({member}, unavailable)")
        }
        _ => fallback.to_owned(),
    }
}

const BROWSER_ENABLED: &str = "muxy.browser.enabled";
const BROWSER_LINKS: &str = "muxy.browser.openLinksInBuiltIn";
const BROWSER_PROFILE: &str = "muxy.browser.defaultProfileID";
const BROWSER_ENGINE: &str = "muxy.browser.searchEngine";
const HOME_PAGE: &str = "muxy.browser.homePageURL";
const BLANK_PAGE: &str = "about:blank";
const COMPOSER_FONT: &str = "editor.richInputFontFamily";
const COMPOSER_IMAGE: &str = "editor.richInputImageStrategy";
const COMMIT_PROVIDER: &str = muxy_core::repository_ai::COMMIT_PROVIDER_KEY;
const COMMIT_PROMPT_KEY: &str = muxy_core::repository_ai::COMMIT_PROMPT_KEY;
const PR_PROVIDER: &str = muxy_core::repository_ai::CREATE_PULL_REQUEST_PROVIDER_KEY;
const PR_PROMPT_KEY: &str = muxy_core::repository_ai::CREATE_PULL_REQUEST_PROMPT_KEY;
const MOBILE_ENABLED: &str = settings::MOBILE_KEYS.enabled;
const MOBILE_PORT: &str = settings::MOBILE_KEYS.port;
const MOBILE_CAP: &str = settings::MOBILE_KEYS.scrollback_cap;
const QUICK_WIDTH: &str = "muxy.quickTerminal.width";
const QUICK_HEIGHT: &str = "muxy.quickTerminal.height";
const QUICK_TRANSPARENCY: &str = "muxy.quickTerminal.transparency";
const QUICK_BLUR: &str = "muxy.quickTerminal.blur";

const COMMIT_PROMPT: &str = muxy_core::repository_ai::COMMIT_PROMPT;
const PULL_REQUEST_PROMPT: &str = muxy_core::repository_ai::CREATE_PULL_REQUEST_PROMPT;

const QUICK_TRANSPARENCY_SLIDER: SliderSpec = SliderSpec {
    key: QUICK_TRANSPARENCY,
    min: 0.0,
    max: 55.0,
    integral: true,
    zero_at_maximum: false,
};

const QUICK_BLUR_SLIDER: SliderSpec = SliderSpec {
    key: QUICK_BLUR,
    min: 0.0,
    max: 100.0,
    integral: true,
    zero_at_maximum: false,
};

pub fn extra_fields(category: Category) -> Vec<Field> {
    let field = |key: &str, value: String, placeholder: &str, rows: bool| Field {
        id: key.to_owned(),
        value,
        placeholder: placeholder.to_owned(),
        monospaced: rows,
        multiline: rows,
    };
    let mobile_port_default = settings::MOBILE_POLICY.default_port().to_string();
    match category {
        Category::Browser => vec![field(
            HOME_PAGE,
            home_page_draft(),
            "https://example.com",
            false,
        )],
        Category::RichInput => vec![field(
            COMPOSER_FONT,
            editor_string(COMPOSER_FONT, "SF Mono"),
            "SF Mono",
            false,
        )],
        Category::Ai => vec![
            field(
                COMMIT_PROMPT_KEY,
                settings::string_value(COMMIT_PROMPT_KEY, COMMIT_PROMPT),
                "",
                true,
            ),
            field(
                PR_PROMPT_KEY,
                settings::string_value(PR_PROMPT_KEY, PULL_REQUEST_PROMPT),
                "",
                true,
            ),
        ],
        Category::Mobile => vec![
            field(
                MOBILE_PORT,
                settings::i64_value(MOBILE_PORT, settings::MOBILE_POLICY.default_port() as i64)
                    .to_string(),
                &mobile_port_default,
                false,
            ),
            field(
                MOBILE_CAP,
                settings::i64_value(MOBILE_CAP, settings::MOBILE_POLICY.default_scrollback_cap())
                    .to_string(),
                "8",
                false,
            ),
        ],
        Category::QuickTerminal => vec![
            field(
                QUICK_WIDTH,
                settings::i64_value(QUICK_WIDTH, 720).to_string(),
                "720",
                false,
            ),
            field(
                QUICK_HEIGHT,
                settings::i64_value(QUICK_HEIGHT, 430).to_string(),
                "430",
                false,
            ),
        ],
        _ => Vec::new(),
    }
}

pub fn commit_extra_field(
    modal: &mut SettingsModal,
    id: &str,
    text: &str,
    cx: &mut Context<SettingsModal>,
) -> bool {
    match id {
        HOME_PAGE => {
            let trimmed = text.trim();
            let value = if trimmed.is_empty() {
                BLANK_PAGE.to_owned()
            } else {
                trimmed.to_owned()
            };
            modal.write(HOME_PAGE, Value::String(value), cx);
        }
        COMPOSER_FONT => {
            settings::set_editor_setting("richInputFontFamily", Value::String(text.to_owned()));
            modal.refresh(cx);
        }
        COMMIT_PROMPT_KEY | PR_PROMPT_KEY => {
            let key: &'static str = if id == COMMIT_PROMPT_KEY {
                COMMIT_PROMPT_KEY
            } else {
                PR_PROMPT_KEY
            };
            modal.write(key, Value::String(text.to_owned()), cx);
        }
        MOBILE_PORT => {
            let valid = text.trim().parse::<i64>().ok().filter(|port| {
                u16::try_from(*port).is_ok_and(|port| settings::MOBILE_POLICY.is_valid_port(port))
            });
            match valid {
                Some(port) => {
                    modal.set_error(id, None, cx);
                    modal.write(MOBILE_PORT, Value::Number(port.into()), cx);
                }
                None => modal.set_error(id, Some("Enter a port between 1024 and 65535."), cx),
            }
        }
        MOBILE_CAP => {
            let valid = text
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|cap| settings::MOBILE_POLICY.is_valid_scrollback_cap(*cap));
            match valid {
                Some(cap) => {
                    modal.set_error(id, None, cx);
                    modal.write(MOBILE_CAP, Value::Number(cap.into()), cx);
                }
                None => modal.set_error(id, Some("Enter a value between 1 and 128 MB."), cx),
            }
        }
        QUICK_WIDTH | QUICK_HEIGHT => {
            let (key, low, high) = if id == QUICK_WIDTH {
                (QUICK_WIDTH, 480, 1200)
            } else {
                (QUICK_HEIGHT, 280, 800)
            };
            if let Some(value) = text
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|value| (low..=high).contains(value))
            {
                modal.write(key, Value::Number(value.into()), cx);
            }
        }
        _ => return false,
    }
    true
}

fn editor_string(key: &str, default: &str) -> String {
    let name = key.strip_prefix("editor.").unwrap_or(key);
    settings::editor_setting(name, Value::String(default.to_owned()))
        .as_str()
        .unwrap_or(default)
        .to_owned()
}

fn home_page_draft() -> String {
    let stored = settings::string_value(HOME_PAGE, BLANK_PAGE);
    if stored == BLANK_PAGE {
        String::new()
    } else {
        stored
    }
}
