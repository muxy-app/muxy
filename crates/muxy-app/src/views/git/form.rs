use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Corner, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, px,
};
use muxy_ui::components::ButtonInteraction;
use muxy_ui::controls::{self, Choice, Style};

use super::{AppModel, Form, Repository};

#[allow(clippy::too_many_lines, reason = "Declarative Git creation form")]
pub(crate) fn render(
    form: &Form,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    let repository = model.git.projects.get(&form.project);
    let busy = repository.is_some_and(Repository::busy);
    let ready = model.session_listing_ready();
    let valid = !form.branch.read(cx).text().trim().is_empty()
        && (!form.worktree || !form.directory.read(cx).text().trim().is_empty());
    let mut view = muxy_ui::popover::surface(theme, m)
        .id("git-form-scroll")
        .debug_selector(|| "git-form".into())
        .w(m.scaled(460.0)
            .min((window.viewport_size().width - px(16.0)).max(px(0.0))))
        .max_h((window.viewport_size().height - m.scaled(80.0)).max(px(0.0)))
        .overflow_y_scroll()
        .p(m.spacing8())
        .gap(m.scaled(14.0))
        .text_color(theme.fg)
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .text_size(m.font_headline())
                .font_weight(FontWeight::SEMIBOLD)
                .child(if form.worktree {
                    "New Worktree"
                } else {
                    "New Branch"
                }),
        );
    if form.worktree {
        let mut choices = [
            Choice::new("new", "Create new branch"),
            Choice::new("existing", "Use existing branch"),
        ];
        for choice in &mut choices {
            choice.enabled = !busy;
        }
        view = view.child(controls::segmented(
            style,
            "worktree-branch-mode",
            &choices,
            if form.existing { "existing" } else { "new" },
            cx.listener(|model, value: &gpui::SharedString, _, cx| {
                model.set_worktree_branch_mode(value.as_ref() == "existing", cx);
            }),
        ));
    }
    if form.worktree && form.existing {
        view = view.child(branch_picker(form, false, model, cx));
    } else {
        view = view.child(field(
            "Branch name",
            controls::text_field(style, "git-branch", &form.branch, None),
            model,
        ));
        if form.worktree {
            view = view.child(branch_picker(form, true, model, cx));
        }
    }
    if form.worktree {
        view = view.child(field(
            "Location",
            controls::text_field(style, "git-directory", &form.directory, None),
            model,
        ));
    }
    let error = form
        .error
        .as_ref()
        .or_else(|| repository.and_then(|r| r.error.as_ref()))
        .or_else(|| repository.and_then(|r| r.load_error(&muxy_protocol::GitAction::Branches)));
    if let Some(error) = error {
        view = view.child(
            div()
                .debug_selector(|| "git-form-error".into())
                .text_size(m.font_footnote())
                .text_color(theme.danger)
                .child(error.clone()),
        );
    } else if !ready {
        view = view.child(
            div()
                .text_size(m.font_footnote())
                .text_color(theme.fg_muted)
                .child("Connect to the server to create."),
        );
    }
    view.child(
        div()
            .flex()
            .justify_end()
            .gap(m.spacing3())
            .pt(m.spacing3())
            .child(
                controls::button(
                    style,
                    "git-cancel",
                    "Cancel",
                    !busy,
                    cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
                )
                .debug_selector(|| "git-cancel".into()),
            )
            .child(
                div()
                    .id("git-submit")
                    .debug_selector(|| "git-submit".into())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .h(m.control_medium())
                    .px(m.spacing5())
                    .rounded(m.radius_sm())
                    .bg(theme.accent)
                    .text_color(theme.accent_foreground)
                    .text_size(m.font_footnote())
                    .font_weight(FontWeight::MEDIUM)
                    .when(!valid || !ready || busy, |button| button.opacity(0.4))
                    .when(valid && ready && !busy, |button| {
                        button
                            .cursor_pointer()
                            .hover(|style| style.opacity(0.85))
                            .button_interaction(
                                cx.listener(|model, _, _, cx| model.submit_git_form(cx)),
                            )
                    })
                    .child(if busy { "Creating…" } else { "Create" }),
            ),
    )
    .into_any_element()
}

fn field(label: &str, input: AnyElement, model: &AppModel) -> gpui::Div {
    let selector = format!("git-field-{label}");
    div()
        .debug_selector(move || selector.clone())
        .flex()
        .flex_col()
        .gap(model.metrics.spacing3())
        .child(
            div()
                .text_size(model.metrics.font_footnote())
                .text_color(model.theme.fg_muted)
                .child(label.to_owned()),
        )
        .child(input)
}

fn branch_picker(
    form: &Form,
    base: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let repository = model.git.projects.get(&form.project);
    let loading = model.session_listing_ready()
        && repository.is_none_or(|r| {
            !r.has_loaded(&muxy_protocol::GitAction::Branches)
                && r.load_error(&muxy_protocol::GitAction::Branches).is_none()
        });
    let value = if base {
        form.base.read(cx).text()
    } else {
        form.branch.read(cx).text()
    };
    let label = if loading {
        "Loading branches…"
    } else if value.is_empty() {
        "Choose a branch…"
    } else {
        value
    };
    let control = controls::picker_trigger(
        Style {
            theme: &model.theme,
            metrics: &m,
        },
        if base { "git-base" } else { "git-existing" },
        label,
        None,
        form.chooser.is_some(),
        cx.listener(move |model, _, _, cx| model.choose_worktree_branch(base, cx)),
    );
    let mut field = field(if base { "Base branch" } else { "Branch" }, control, model).relative();
    if let Some(chooser) = &form.chooser {
        field = field.child(
            deferred(
                anchored()
                    .anchor(Corner::TopLeft)
                    .offset(point(px(0.0), m.control_medium() + m.spacing6()))
                    .snap_to_window_with_margin(px(8.0))
                    .child(chooser.clone()),
            )
            .with_priority(2),
        );
    }
    field.into_any_element()
}
