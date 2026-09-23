use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use muxy_ui::controls;

use super::{Category, Change, PickerKind, SettingsEvent, SettingsView};
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

fn provider_label(view: &SettingsView, action: Action) -> String {
    let installed = |id: &str| view.snapshot.ai_installed.contains(&id);
    match view
        .snapshot
        .settings
        .ai
        .providers
        .get(action.key())
        .filter(|id| !id.is_empty())
    {
        Some(id) => match crate::ai::provider(id) {
            Some(provider) if installed(provider.id) => provider.name.to_owned(),
            Some(provider) => format!("{} · Not installed", provider.name),
            None => format!("{id} · Unsupported"),
        },
        None => view
            .snapshot
            .ai_installed
            .first()
            .and_then(|id| crate::ai::provider(id))
            .map_or_else(
                || "Auto · None installed".to_owned(),
                |provider| format!("Auto · {}", provider.name),
            ),
    }
}

pub(super) fn rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    for (action, prompt_id) in PROMPTS {
        let (provider, prompt) = match action {
            Action::Commit => ("Commit provider", "Commit prompt"),
            Action::CreatePullRequest => ("Pull request provider", "Pull request prompt"),
        };
        if view.matches(Category::Ai, provider) {
            rows.push(view.row(
                provider_id(action),
                provider,
                view.picker(
                    PickerKind::AiProvider(action),
                    &provider_label(view, action),
                    cx,
                ),
            ));
        }
        if view.matches(Category::Ai, prompt) {
            let input = &view.fields[prompt_id];
            let edited = input.read(cx).text().trim() != action.default_prompt();
            let restore = controls::button(
                view.style(),
                &format!("{prompt_id}-restore"),
                "Restore Default",
                edited,
                cx.listener(move |_, _, _, cx| {
                    cx.emit(SettingsEvent::Change(Change::Field(
                        prompt_id,
                        action.default_prompt().into(),
                    )));
                }),
            );
            rows.push(
                view.row_with(
                    prompt_id,
                    prompt,
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap(view.metrics.spacing3())
                        .child(div().w_full().child(controls::text_area(
                            view.style(),
                            prompt_id,
                            input,
                            Some(88.0),
                        )))
                        .child(restore)
                        .into_any_element(),
                    true,
                ),
            );
        }
    }
    rows
}
