use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Focusable, InteractiveElement, IntoElement, ParentElement, Styled, Window,
    div,
};
use muxy_app_core::opener::FileRule;
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::{controls, form, tr};

use super::{Category, PickerKind, SettingsEvent, SettingsView, Snapshot};
use crate::model::preferences::Change;

pub(super) struct RuleEditor {
    pub(super) previous: Option<String>,
    pub(super) rule: FileRule,
}

pub(super) fn opener_label(snapshot: &Snapshot, id: &str) -> String {
    match id {
        "ask" => tr!("Always ask").to_string(),
        "system.finder" => translate(crate::opener::FILE_MANAGER).to_string(),
        _ => snapshot
            .file_openers
            .iter()
            .find(|handler| handler.id == id)
            .map_or_else(
                || tr!("%@ (unavailable)", id).to_string(),
                |handler| handler.title.clone(),
            ),
    }
}

pub(super) fn rows(pane: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if pane.matches(Category::General, tr_key!("Always show the file opener")) {
        let ask = pane.snapshot.settings.openers.always_ask;
        rows.push(pane.row(
            "always-ask-files",
            tr_key!("Always show the file opener"),
            pane.toggle("always-ask-files", ask, Change::AlwaysAskFiles(!ask), cx),
        ));
    }
    if pane.matches(Category::General, tr_key!("File type rules")) {
        rows.push(pane.framed_row("file-rules", None, |description| {
            div().flex().flex_col()
                .child(form::row(pane.layout_style(), &tr!("File type rules"), description,
                    controls::button(pane.style(), "add-file-rule", controls::Button::secondary(&tr!("Add Rule"))
                        .enabled(pane.file_rule_editor.is_none()),
                        cx.listener(|pane, _, window, cx| pane.edit_file_rule(None, window, cx)))
                        .debug_selector(|| "settings-add-file-rule".into()).into_any_element(), pane.compact))
                .when(pane.file_rule_editor.is_some(), |body| body.child(editor(pane, cx)))
                .children(pane.snapshot.settings.openers.file_rules.iter().enumerate().map(|(index, rule)| rule_row(pane, index, rule, cx)))
                .when(pane.snapshot.settings.openers.file_rules.is_empty(), |body| body.child(pane.note(&tr!("No custom rules. Matching extensions open automatically; overlaps show a chooser. Unmatched local files go to the file manager."), false)))
        }));
    }
    if pane.matches(Category::General, tr_key!("Registered file types")) {
        rows.push(pane.framed_row("file-registrations", None, |description| {
            div()
                .flex()
                .flex_col()
                .child(form::row(
                    pane.layout_style(),
                    &tr!("Registered file types"),
                    description,
                    div().into_any_element(),
                    pane.compact,
                ))
                .children(pane.snapshot.file_openers.iter().map(|handler| {
                    let patterns = if handler.patterns.is_empty() {
                        tr!("Custom rules only").to_string()
                    } else {
                        handler.patterns.join(", ")
                    };
                    form::row(
                        pane.layout_style(),
                        &handler.title,
                        Some(&patterns),
                        div().into_any_element(),
                        pane.compact,
                    )
                }))
                .when(pane.snapshot.file_openers.is_empty(), |body| {
                    body.child(pane.note(&tr!("No enabled extensions register file types."), false))
                })
        }));
    }
    rows
}

fn editor(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let Some(editor) = &pane.file_rule_editor else {
        return div().into_any_element();
    };
    div()
        .flex()
        .flex_col()
        .child(form::row(
            pane.layout_style(),
            &tr!("File pattern"),
            Some(&tr!("Examples: *.md, *.tar.gz, Dockerfile, or docs/*.txt.")),
            pane.field("file-pattern"),
            pane.compact,
        ))
        .child(form::row(
            pane.layout_style(),
            &tr!("Open with"),
            None,
            pane.picker(
                PickerKind::FileOpener,
                &opener_label(&pane.snapshot, &editor.rule.opener),
                cx,
            ),
            pane.compact,
        ))
        .child(
            div()
                .flex()
                .gap(pane.layout_metrics.scaled(6.0))
                .child(
                    controls::button(
                        pane.style(),
                        "save-file-rule",
                        controls::Button::secondary(&tr!("Save")),
                        cx.listener(|pane, _, _, cx| pane.save_file_rule(cx)),
                    )
                    .debug_selector(|| "settings-save-file-rule".into()),
                )
                .child(controls::button(
                    pane.style(),
                    "cancel-file-rule",
                    controls::Button::secondary(&tr!("Cancel")),
                    cx.listener(|pane, _, _, cx| pane.cancel_file_rule(cx)),
                )),
        )
        .into_any_element()
}

fn rule_row(
    pane: &SettingsView,
    index: usize,
    rule: &FileRule,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let mut buttons = div()
        .flex()
        .flex_wrap()
        .gap(pane.layout_metrics.scaled(6.0));
    let enabled = pane.file_rule_editor.is_none();
    for (action, label, allowed, change) in [
        (
            "up",
            tr!("Move Up"),
            index > 0,
            Change::MoveFileRule(rule.pattern.clone(), true),
        ),
        (
            "down",
            tr!("Move Down"),
            index + 1 < pane.snapshot.settings.openers.file_rules.len(),
            Change::MoveFileRule(rule.pattern.clone(), false),
        ),
        (
            "remove",
            tr!("Remove"),
            true,
            Change::RemoveFileRule(rule.pattern.clone()),
        ),
    ] {
        buttons = buttons.child(controls::button(
            pane.style(),
            &format!("file-rule-{action}-{index}"),
            controls::Button::secondary(&label)
                .named(&tr!("%@: %@", &rule.pattern, &label))
                .enabled(enabled && allowed),
            cx.listener(move |_, _, _, cx| cx.emit(SettingsEvent::Change(change.clone()))),
        ));
    }
    let edit = rule.clone();
    buttons = buttons.child(controls::button(
        pane.style(),
        &format!("edit-file-rule-{index}"),
        controls::Button::secondary(&tr!("Edit"))
            .named(&tr!("Edit %@", &rule.pattern))
            .enabled(enabled),
        cx.listener(move |pane, _, window, cx| pane.edit_file_rule(Some(edit.clone()), window, cx)),
    ));
    form::row(
        pane.layout_style(),
        &rule.pattern,
        Some(&opener_label(&pane.snapshot, &rule.opener)),
        buttons.into_any_element(),
        pane.compact,
    )
    .into_any_element()
}

impl SettingsView {
    fn edit_file_rule(
        &mut self,
        rule: Option<FileRule>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = rule.as_ref().map(|rule| rule.pattern.clone());
        let rule = rule.unwrap_or_else(|| FileRule {
            pattern: String::new(),
            opener: "ask".into(),
        });
        self.fields["file-pattern"].update(cx, |input, cx| input.set_text(&rule.pattern, cx));
        self.file_rule_editor = Some(RuleEditor { previous, rule });
        self.errors.remove("file-rules");
        self.results.dirty = true;
        self.fields["file-pattern"].focus_handle(cx).focus(window);
        cx.notify();
    }

    pub(super) fn save_file_rule(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.file_rule_editor else {
            return;
        };
        let rule = FileRule {
            pattern: self.fields["file-pattern"]
                .read(cx)
                .text()
                .trim()
                .to_owned(),
            opener: editor.rule.opener.clone(),
        };
        cx.emit(SettingsEvent::Change(Change::FileRule {
            previous: editor.previous.clone(),
            rule,
        }));
    }

    pub(super) fn cancel_file_rule(&mut self, cx: &mut Context<Self>) {
        self.file_rule_editor = None;
        self.errors.remove("file-rules");
        self.results.dirty = true;
        cx.notify();
    }
}
