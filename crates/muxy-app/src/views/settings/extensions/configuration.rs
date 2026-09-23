//! Installed-extension details read from the running app: the settings the
//! manifest declares, and the extension's recent log lines.

use gpui::prelude::FluentBuilder;
use gpui::{AppContext, Context, IntoElement, ParentElement, Styled, div};
use muxy_app_core::extensions::{Extension, SettingKind};
use muxy_ui::{
    controls, form,
    text_input::{InputEvent, InputStyle, TextInput},
};
use serde_json::{Map, Value};

use super::ExtensionsView;

/// What a number field stores: a number, or nothing so the default applies.
fn number(text: &str) -> Option<Value> {
    let text = text.trim();
    text.parse::<i64>().map(Value::from).ok().or_else(|| {
        text.parse::<f64>()
            .ok()
            .and_then(|number| serde_json::Number::from_f64(number).map(Value::Number))
    })
}

fn display(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

impl ExtensionsView {
    /// Creates the text inputs for the selected extension's string and
    /// number settings, filled with their current values.
    pub(super) fn sync_setting_inputs(
        &mut self,
        extension: &Extension,
        stored: &Map<String, Value>,
        cx: &mut Context<Self>,
    ) {
        for setting in &extension.manifest.settings {
            let key = (extension.name.clone(), setting.key.clone());
            if setting.kind == SettingKind::Bool || self.setting_inputs.contains_key(&key) {
                continue;
            }
            let value = stored.get(&setting.key).unwrap_or(&setting.default_value);
            let input = cx.new(|cx| {
                TextInput::new(InputStyle::field(&self.theme, &self.metrics), cx)
                    .with_text(display(value))
            });
            let (owner, name, kind) = (key.0.clone(), key.1.clone(), setting.kind);
            self.subscriptions.push(cx.subscribe(
                &input,
                move |view: &mut Self, input, event, cx| {
                    if !matches!(event, InputEvent::Changed) {
                        return;
                    }
                    let text = input.read(cx).text().to_owned();
                    let value = if kind == SettingKind::Number {
                        number(&text)
                    } else {
                        Some(Value::String(text))
                    };
                    view.store_setting(&owner, &name, value, cx);
                },
            ));
            self.setting_inputs.insert(key, input);
        }
    }

    fn store_setting(
        &mut self,
        owner: &str,
        key: &str,
        value: Option<Value>,
        cx: &mut Context<Self>,
    ) {
        let task = self.model.update(cx, |model, cx| {
            model.set_extension_setting(owner, key, value, cx)
        });
        cx.spawn(async move |view, cx| {
            let result = match task {
                Ok(task) => task.await,
                Err(error) => Err(error.to_string()),
            };
            if let Err(error) = result {
                let _ = view.update(cx, |view, cx| {
                    view.error = Some(error);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(super) fn settings_section(
        &self,
        extension: &Extension,
        stored: &Map<String, Value>,
        cx: &Context<Self>,
    ) -> Option<gpui::Div> {
        if extension.manifest.settings.is_empty() {
            return None;
        }
        let mut rows = div().flex().flex_col();
        for setting in &extension.manifest.settings {
            let id = format!("extension-setting-{}-{}", extension.name, setting.key);
            let control = if setting.kind == SettingKind::Bool {
                let on = stored
                    .get(&setting.key)
                    .unwrap_or(&setting.default_value)
                    .as_bool()
                    .unwrap_or(false);
                let (owner, key) = (extension.name.clone(), setting.key.clone());
                controls::toggle(
                    self.style(),
                    &id,
                    on,
                    cx.listener(move |view, _, _, cx| {
                        view.store_setting(&owner, &key, Some(Value::Bool(!on)), cx);
                    }),
                )
            } else if let Some(input) = self
                .setting_inputs
                .get(&(extension.name.clone(), setting.key.clone()))
            {
                controls::text_field(self.style(), &id, input, Some(controls::CONTROL_WIDTH))
            } else {
                continue;
            };
            rows = rows.child(form::row(
                self.style(),
                &setting.title,
                setting.description.as_deref(),
                control,
                false,
            ));
        }
        Some(self.section("Settings", rows))
    }

    /// Recent `[log]`, `[warn]`, `[err]`, and `[muxy]` lines, newest last.
    pub(super) fn logs_section(
        &self,
        extension: &Extension,
        lines: &[String],
        background: Option<bool>,
    ) -> gpui::Div {
        self.section(
            "Logs",
            div()
                .flex()
                .flex_col()
                .gap(self.metrics.spacing4())
                .when_some(background, |logs, running| {
                    logs.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(self.metrics.spacing4())
                            .child("Background script")
                            .child(
                                self.badge(if running { "Running" } else { "Stopped" }, running),
                            ),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .font_family(".SystemUIFontMonospaced")
                        .text_size(self.metrics.font_footnote())
                        .text_color(self.theme.fg_muted)
                        .children(
                            lines
                                .iter()
                                .map(|line| div().child(line.clone()).into_any_element()),
                        )
                        .when(lines.is_empty(), |logs| {
                            logs.child(format!(
                                "No output yet. Logs are also written to {}.",
                                extension.directory.join("logs/output.log").display()
                            ))
                        }),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_settings_store_numbers_or_fall_back_to_the_default() {
        assert_eq!(number(" 42 "), Some(Value::from(42)));
        assert_eq!(number("1.5"), Some(serde_json::json!(1.5)));
        assert_eq!(number(""), None);
        assert_eq!(number("fast"), None);
    }
}
