use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
};
use muxy_ui::l10n::tr_key;
use muxy_ui::{controls, tr};

use super::{Category, PickerKind, SettingsEvent, SettingsView};
use crate::model::preferences::Change;
use crate::repository_actions::Action;

pub(super) const PROMPTS: [(Action, &str); 2] = [
    (Action::Commit, "ai-commit-prompt"),
    (Action::CreatePullRequest, "ai-create-pr-prompt"),
];

pub(super) fn provider_id(action: Action) -> &'static str {
    match action {
        Action::Commit => "ai-commit-provider",
        Action::CreatePullRequest => "ai-create-pr-provider",
    }
}

pub(crate) fn prompt_action(id: &str) -> Option<Action> {
    PROMPTS
        .iter()
        .find(|(_, prompt)| *prompt == id)
        .map(|(action, _)| *action)
}

fn provider_label(view: &SettingsView, key: &str) -> SharedString {
    let installed = |id: &str| view.snapshot.ai_installed.contains(&id);
    match view
        .snapshot
        .settings
        .ai
        .providers
        .get(key)
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
    {
        Some(id) => match crate::ai::provider(id) {
            Some(provider) if installed(provider.id) => provider.name.into(),
            Some(provider) => tr!("%@ · Not installed", provider.name),
            None => tr!("%@ · Unsupported", id),
        },
        None => view
            .snapshot
            .ai_installed
            .first()
            .and_then(|id| crate::ai::provider(id))
            .map_or_else(
                || tr!("Auto · None installed"),
                |provider| tr!("Auto · %@", provider.name),
            ),
    }
}

fn prompt_row(
    view: &SettingsView,
    id: &'static str,
    label: &'static str,
    default_prompt: &'static str,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let input = &view.fields[id];
    let edited = input.read(cx).text().trim() != default_prompt;
    let restore = controls::button(
        view.style(),
        &format!("{id}-restore"),
        controls::Button::secondary(&tr!("Restore Default"))
            .named(&tr!("Restore Default %@", &muxy_ui::l10n::translate(label)))
            .enabled(edited),
        cx.listener(move |_, _, _, cx| {
            cx.emit(SettingsEvent::Change(Change::Field(
                id,
                default_prompt.into(),
            )));
        }),
    )
    .debug_selector(move || format!("{id}-restore"));
    view.row_with(
        id,
        label,
        div()
            .flex()
            .flex_col()
            .items_end()
            .gap(view.metrics.spacing3())
            .child(
                div()
                    .w_full()
                    .child(controls::text_area(view.style(), id, input, Some(88.0))),
            )
            .child(restore)
            .into_any_element(),
        true,
    )
}

pub(super) fn rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    for (action, prompt_id) in PROMPTS {
        let (provider, prompt) = match action {
            Action::Commit => (tr_key!("Commit provider"), tr_key!("Commit prompt")),
            Action::CreatePullRequest => (
                tr_key!("Pull request provider"),
                tr_key!("Pull request prompt"),
            ),
        };
        if view.matches(Category::Ai, provider) {
            rows.push(view.row(
                provider_id(action),
                provider,
                view.picker(
                    PickerKind::AiProvider(action),
                    &provider_label(view, action.key()),
                    cx,
                ),
            ));
        }
        if view.matches(Category::Ai, prompt) {
            rows.push(prompt_row(
                view,
                prompt_id,
                prompt,
                action.default_prompt(),
                cx,
            ));
        }
    }
    if view.matches(Category::Ai, tr_key!("Prompt provider")) {
        rows.push(view.row(
            "prompt-provider",
            tr_key!("Prompt provider"),
            view.picker(
                PickerKind::PromptProvider,
                &provider_label(view, crate::prompt::PROMPT_PROVIDER),
                cx,
            ),
        ));
    }
    if view.matches(Category::Ai, tr_key!("Prompt model")) {
        rows.push(view.row(
            crate::prompt::PROMPT_MODEL,
            tr_key!("Prompt model"),
            view.field(crate::prompt::PROMPT_MODEL),
        ));
    }
    if view.matches(Category::Ai, tr_key!("Prompt instructions")) {
        rows.push(prompt_row(
            view,
            crate::prompt::PROMPT_INSTRUCTIONS,
            tr_key!("Prompt instructions"),
            crate::prompt::DEFAULT_INSTRUCTIONS,
            cx,
        ));
    }
    rows
}
