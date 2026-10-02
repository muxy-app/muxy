use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Focusable, InteractiveElement, IntoElement, ParentElement, Styled, Window,
    div, px,
};
use muxy_app_core::settings::CustomCommand;
use muxy_ui::{controls, form};

use super::{Category, Change, SettingsEvent, SettingsView};

pub(super) fn matching(pane: &SettingsView) -> Vec<usize> {
    pane.snapshot
        .settings
        .commands
        .iter()
        .enumerate()
        .filter_map(|(index, command)| {
            pane.matches(
                Category::Commands,
                &format!("Custom commands {} {}", command.name, command.command),
            )
            .then_some(index)
        })
        .collect()
}

pub(super) fn heading(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(pane.note("Run a saved shell command in a new tab in the selected project. Assign a direct shortcut or find it in the command palette.", false))
        .child(div().flex().child(controls::button(
            pane.style(),
            "add-command",
            "Add Command",
            pane.command_editor.is_none(),
            cx.listener(|pane, _, window, cx| pane.edit_command(None, window, cx)),
        ).debug_selector(|| "settings-add-command".into())))
        .when(pane.snapshot.settings.commands.is_empty() && pane.command_editor.is_none(), |body| {
            body.child(pane.note("No custom commands yet.", false))
        })
        .when(pane.command_editor.is_some(), |body| body.child(editor(pane, cx)))
        .when_some(pane.errors.get("commands"), |body, error| body.child(pane.note(error, true)))
        .into_any_element()
}

fn editor(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(form::row(
            pane.layout_style(),
            "Name",
            None,
            pane.field("command-name"),
            pane.compact,
        ))
        .child(form::row(
            pane.layout_style(),
            "Command",
            Some("A single-line shell command, such as cargo test or npm run dev."),
            pane.field("command-text"),
            pane.compact,
        ))
        .child(
            div()
                .flex()
                .gap(px(6.0))
                .child(
                    controls::button(
                        pane.style(),
                        "save-command",
                        "Save",
                        true,
                        cx.listener(|pane, _, _, cx| pane.save_command(cx)),
                    )
                    .debug_selector(|| "settings-save-command".into()),
                )
                .child(
                    controls::button(
                        pane.style(),
                        "cancel-command",
                        "Cancel",
                        true,
                        cx.listener(|pane, _, _, cx| pane.cancel_command(cx)),
                    )
                    .debug_selector(|| "settings-cancel-command".into()),
                ),
        )
        .into_any_element()
}

pub(super) fn row(pane: &SettingsView, index: usize, cx: &mut Context<SettingsView>) -> AnyElement {
    let command = &pane.snapshot.settings.commands[index];
    let id = command.shortcut_id();
    let chord = pane.snapshot.settings.keymap.binding(&id);
    let label = if pane.recording.as_deref() == Some(id.as_str()) {
        "Press a shortcut…"
    } else {
        chord.map_or("Record shortcut", muxy_app_core::settings::KeyChord::as_str)
    };
    let focus = &pane.results.command_focus[&id];
    let enabled = pane.command_editor.is_none();
    let (record, unassign, edit, delete) =
        (id.clone(), id.clone(), command.clone(), command.id.clone());
    let controls = div()
        .debug_selector({
            let id = id.clone();
            move || format!("settings-command-actions-{id}")
        })
        .flex()
        .flex_wrap()
        .gap(px(6.0))
        .child(
            controls::button(
                pane.style(),
                &id,
                label,
                enabled,
                cx.listener(move |pane, _, window, cx| pane.begin_recording(&record, window, cx)),
            )
            .track_focus(&focus[0])
            .debug_selector({
                let id = id.clone();
                move || format!("settings-record-{id}")
            }),
        )
        .child(
            controls::button(
                pane.style(),
                &format!("unassign-{id}"),
                "Unassign",
                enabled && chord.is_some(),
                cx.listener(move |pane, _, _, cx| {
                    pane.recording = None;
                    cx.emit(SettingsEvent::Change(Change::Binding(
                        unassign.clone(),
                        None,
                    )));
                }),
            )
            .track_focus(&focus[1]),
        )
        .child(
            controls::button(
                pane.style(),
                &format!("edit-{id}"),
                "Edit",
                enabled,
                cx.listener(move |pane, _, window, cx| {
                    pane.edit_command(Some(edit.clone()), window, cx);
                }),
            )
            .track_focus(&focus[2])
            .debug_selector({
                let id = id.clone();
                move || format!("settings-edit-{id}")
            }),
        )
        .child(
            controls::button(
                pane.style(),
                &format!("delete-{id}"),
                "Delete",
                enabled,
                cx.listener(move |pane, _, _, cx| {
                    pane.recording = None;
                    cx.emit(SettingsEvent::Change(Change::RemoveCommand(delete.clone())));
                }),
            )
            .track_focus(&focus[3])
            .debug_selector({
                let id = id.clone();
                move || format!("settings-delete-{id}")
            }),
        );
    pane.row_with_description(
        &id,
        &command.name,
        Some(&command.command),
        controls.into_any_element(),
        pane.compact,
    )
}

impl SettingsView {
    fn edit_command(
        &mut self,
        command: Option<CustomCommand>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let command = command.unwrap_or_else(|| CustomCommand::new(String::new(), String::new()));
        self.fields["command-name"].update(cx, |input, cx| input.set_text(&command.name, cx));
        self.fields["command-text"].update(cx, |input, cx| input.set_text(&command.command, cx));
        self.command_editor = Some(command);
        self.recording = None;
        self.errors.remove("commands");
        self.category = Category::Commands;
        self.section = None;
        self.query.clear();
        self.search.update(cx, |input, cx| input.set_text("", cx));
        self.results.reset();
        self.fields["command-name"].focus_handle(cx).focus(window);
        cx.notify();
    }

    pub(super) fn save_command(&mut self, cx: &mut Context<Self>) {
        let Some(mut command) = self.command_editor.clone() else {
            return;
        };
        command.name = self.fields["command-name"].read(cx).text().trim().into();
        command.command = self.fields["command-text"].read(cx).text().trim().into();
        cx.emit(SettingsEvent::Change(Change::Command(command)));
    }

    pub(super) fn cancel_command(&mut self, cx: &mut Context<Self>) {
        self.command_editor = None;
        self.errors.remove("commands");
        self.results.dirty = true;
        cx.notify();
    }
}
