use super::*;

pub(super) fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let mut sections = Vec::new();
    sections.extend(visible(
        modal,
        Category::Ai,
        "Commit and Push",
        Some("AI generates only the commit message. Muxy always stages all changes, commits, and pushes. An empty prompt uses the default. Do not include secrets."),
        true,
        ai_action(modal, COMMIT_PROVIDER, COMMIT_PROMPT_KEY, COMMIT_PROMPT, cx),
    ));
    sections.extend(visible(
        modal,
        Category::Ai,
        "Create Pull Request",
        Some("AI generates the title, summary, new branch name, and target branch. Muxy creates the branch, commit, push, and pull request. An empty prompt uses the default."),
        false,
        ai_action(modal, PR_PROVIDER, PR_PROMPT_KEY, PULL_REQUEST_PROMPT, cx),
    ));
    sections
}

fn ai_action(
    modal: &SettingsModal,
    provider_key: &'static str,
    prompt_key: &'static str,
    default_prompt: &'static str,
    cx: &mut Context<SettingsModal>,
) -> Vec<AnyElement> {
    let style = modal.style();
    let metrics = style.metrics;
    let selected = settings::string_value(provider_key, "");
    let mut choices = vec![Choice::new("", "Auto")];
    choices.extend(
        settings::AI_PROVIDERS
            .iter()
            .map(|(id, name)| Choice::new(*id, *name)),
    );

    let mut items = vec![picker_row(
        modal,
        "Provider",
        provider_key,
        "",
        appended_stored(choices, &selected, format!("{selected} (unavailable)")),
        cx,
    )];

    let current = modal.field_text(prompt_key, cx);
    let mut block = div()
        .flex()
        .flex_col()
        .gap(metrics.spacing3())
        .px(metrics.spacing6())
        .py(metrics.spacing3())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .child(
                    div()
                        .flex_grow()
                        .text_size(metrics.font_body())
                        .text_color(style.theme.fg)
                        .child(SharedString::from("Prompt")),
                )
                .child(controls::button(
                    style,
                    &format!("restore-{prompt_key}"),
                    "Restore Default",
                    current != default_prompt,
                    cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
                        modal.write(prompt_key, Value::String(default_prompt.to_owned()), cx);
                        modal.reset_field(prompt_key, default_prompt, cx);
                    }),
                )),
        );
    if let Some(field) = modal.field(prompt_key) {
        block = block.child(controls::text_area(style, prompt_key, field, Some(90.0)));
    }
    items.push(block.into_any_element());
    items
}
