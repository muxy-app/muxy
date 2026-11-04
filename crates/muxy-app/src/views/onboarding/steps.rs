//! Each step's words and controls.

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_app_core::qr::{QUIET_ZONE, QrCode};
use muxy_core::shortcuts::ShortcutId;
use muxy_ui::accessibility;
use muxy_ui::components::{ButtonInteraction, IconGlyph};
use muxy_ui::controls::{self, Style};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::art::kit::units;
use super::art::{STORE_QR_MODULE, chord_text, key_caps};
use crate::model::AppModel;
use crate::model::onboarding::{APP_STORE_URL, Offer, OfferState, PLAY_STORE_URL, Step};

pub(super) fn body(
    step: Step,
    art_shown: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> Div {
    match step {
        Step::Welcome => welcome(model),
        Step::Server => server(model),
        Step::Projects => projects(model, cx),
        Step::Extensions => extensions(model, cx),
        Step::Agents => agents(model),
        Step::Composer => composer(model),
        Step::Remote => remote(model, cx),
        Step::Mobile => mobile(art_shown, model, cx),
        Step::Shortcuts => shortcuts(model),
    }
}

/// The step's heading and its one or two sentences.
fn intro(title: SharedString, text: SharedString, model: &AppModel) -> Div {
    let m = model.metrics;
    div()
        .flex()
        .flex_col()
        .gap(m.spacing4())
        .child(
            div()
                .debug_selector(|| "onboarding-title".into())
                .text_size(m.font_hero())
                .line_height(m.scaled(30.0))
                .font_weight(FontWeight::BOLD)
                .text_color(model.theme.fg)
                .child(accessibility::label("onboarding-title", title)),
        )
        .child(
            div()
                .text_size(m.font_emphasis())
                .line_height(m.scaled(19.0))
                .text_color(model.theme.fg_muted)
                .child(accessibility::label("onboarding-text", text)),
        )
}

fn column(model: &AppModel) -> Div {
    div().flex().flex_col().gap(model.metrics.spacing8())
}

/// One thing worth knowing: an icon, a few words, and a sentence.
fn point(icon: Icon, title: SharedString, text: SharedString, model: &AppModel) -> Div {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .items_start()
        .gap(m.spacing6())
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(m.scaled(28.0))
                .rounded(m.radius_md())
                .bg(theme.accent_soft)
                .child(IconGlyph::new(icon, m.icon_md(), theme.accent)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.0))
                .gap(m.spacing1())
                .child(
                    div()
                        .text_size(m.font_body())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(m.font_footnote())
                        .line_height(m.scaled(15.0))
                        .text_color(theme.fg_muted)
                        .child(text),
                ),
        )
}

fn points(items: impl IntoIterator<Item = Div>, model: &AppModel) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(model.metrics.spacing7())
        .children(items)
}

/// What a click just did, as a line beside the button that did it.
fn confirmation(text: SharedString, model: &AppModel) -> Div {
    let m = model.metrics;
    div()
        .flex()
        .items_center()
        .gap(m.spacing3())
        .min_w(px(0.0))
        .child(IconGlyph::new(
            Icon::CircleCheck,
            m.icon_md(),
            model.theme.diff_add,
        ))
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_size(m.font_footnote())
                .text_color(model.theme.fg)
                .child(text),
        )
}

fn problem(text: SharedString, model: &AppModel) -> AnyElement {
    let m = model.metrics;
    accessibility::announced(
        div()
            .id("onboarding-problem")
            .debug_selector(|| "onboarding-problem".into())
            .text_size(m.font_footnote())
            .line_height(m.scaled(15.0))
            .text_color(model.theme.danger),
        text.clone(),
        true,
    )
    .child(text)
    .into_any_element()
}

fn action(
    id: &'static str,
    button: controls::Button,
    model: &AppModel,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let style = Style {
        theme: &model.theme,
        metrics: &model.metrics,
    };
    div()
        .flex_none()
        .debug_selector(move || id.into())
        .child(controls::button(style, id, button, on_click))
}

fn welcome(model: &AppModel) -> Div {
    column(model)
        .child(intro(
            tr!("Welcome to Muxy"),
            tr!(
                "Terminals that keep running, organized around your projects. This short tour shows you around and sets up the essentials."
            ),
            model,
        ))
        .child(points(
            [
                point(
                    Icon::Puzzle,
                    tr!("Set up the essentials"),
                    tr!("Add a project and the extensions most people start with."),
                    model,
                ),
                point(
                    Icon::Sparkles,
                    tr!("See what makes it different"),
                    tr!("Background terminals, AI agents, remote servers, and your phone."),
                    model,
                ),
                point(
                    Icon::Clock,
                    tr!("About a minute"),
                    tr!("Skip whenever you like. You can replay it from the command palette."),
                    model,
                ),
            ],
            model,
        ))
}

fn server(model: &AppModel) -> Div {
    column(model)
        .child(intro(
            tr!("Your terminals live on a server"),
            tr!(
                "A small background program, muxy-server, runs every terminal. This app, the muxy command, and your phone are all clients of it."
            ),
            model,
        ))
        .child(points(
            [
                point(
                    Icon::Restore,
                    tr!("Quit whenever you like"),
                    tr!("Close the window or quit the app, and your terminals keep running."),
                    model,
                ),
                point(
                    Icon::Overlap,
                    tr!("One terminal, any client"),
                    tr!("Open the same terminal from several apps at once."),
                    model,
                ),
                point(
                    Icon::Sparkles,
                    tr!("Nothing to manage"),
                    tr!("The server starts by itself when an app needs it."),
                    model,
                ),
            ],
            model,
        ))
}

fn projects(model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let added = model
        .onboarding
        .tour
        .as_ref()
        .and_then(|tour| tour.added_project.clone());
    column(model)
        .child(intro(
            tr!("Everything starts with a project"),
            tr!(
                "A project is a folder with its own tabs, splits, and worktrees. Switch with a click, and everything is where you left it."
            ),
            model,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap(model.metrics.spacing6())
                .child(action(
                    "onboarding-add-project",
                    controls::Button::secondary(&tr!("Add a Project…")),
                    model,
                    cx.listener(|model, _, window, cx| {
                        model.add_project_from_onboarding(window, cx);
                    }),
                ))
                .children(added.map(|name| confirmation(tr!("%@ added", name), model))),
        )
        .child(points(
            [
                point(
                    Icon::Palette,
                    tr!("Make it yours"),
                    tr!("Right-click a project to change its icon, color, or logo."),
                    model,
                ),
                point(
                    Icon::GitBranch,
                    tr!("Worktrees built in"),
                    tr!(
                        "Press %@ to give a branch its own tabs and terminals.",
                        chord_text(ShortcutId::NewWorktree, model).as_str()
                    ),
                    model,
                ),
                point(
                    Icon::LayoutSplit,
                    tr!("Layouts you can share"),
                    tr!("Keep a layout in .muxy/layouts to reopen the same workspace."),
                    model,
                ),
            ],
            model,
        ))
}

fn extensions(model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let m = model.metrics;
    let unavailable = model
        .onboarding
        .offers
        .iter()
        .any(|offer| matches!(offer.state, OfferState::Unavailable(_)));
    let all_enabled = model
        .onboarding
        .offers
        .iter()
        .all(|offer| model.extensions.registry.enabled(offer.name).is_some());
    let to_install = model.offers_to_install();
    let installing = model.onboarding.installing();
    let label = if installing {
        tr!("Installing…")
    } else {
        tr!("Install & Enable")
    };
    let button = controls::Button::primary(&label).enabled(to_install > 0);
    column(model)
        .gap(m.spacing7())
        .child(intro(
            tr!("Add the essentials"),
            tr!("Most people start with these two. Find more anytime in Settings → Extensions."),
            model,
        ))
        .child(
            div().flex().flex_col().gap(m.spacing4()).children(
                (0..model.onboarding.offers.len()).map(|index| offer_card(index, model, cx)),
            ),
        )
        .child(
            div()
                .text_size(m.font_caption())
                .line_height(m.scaled(14.0))
                .text_color(model.theme.fg_muted)
                .child(tr!(
                    "Commands and file or Git changes ask for permission when used."
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.spacing4())
                .when(!all_enabled, |row| {
                    row.child(action(
                        "onboarding-install",
                        button,
                        model,
                        cx.listener(|model, _, _, cx| model.install_offers(cx)),
                    ))
                })
                .when(unavailable, |row| {
                    row.child(action(
                        "onboarding-retry",
                        controls::Button::secondary(&tr!("Try Again")),
                        model,
                        cx.listener(|model, _, _, cx| model.retry_offers(cx)),
                    ))
                }),
        )
}

fn offer_card(index: usize, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let Some(offer) = model.onboarding.offers.get(index) else {
        return div().into_any_element();
    };
    let name = offer.name;
    let (icon, title, summary) = if name == "files" {
        (
            Icon::Folder,
            tr!("Files"),
            tr!("Browse and open your project's files beside your terminals."),
        )
    } else {
        (
            Icon::GitBranch,
            tr!("Git"),
            tr!("Stage, commit, and manage pull requests in a Source Control panel."),
        )
    };
    let enabled = model.extensions.registry.enabled(name).is_some();
    let permissions = offer.details.as_ref().map(|details| {
        details.permissions.as_ref().map_or_else(
            || tr!("The marketplace did not list this extension's permissions."),
            |permissions| {
                if permissions.is_empty() {
                    tr!("No permissions requested.")
                } else {
                    let list = permissions
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(" · ");
                    tr!("Asks for: %@", list)
                }
            },
        )
    });
    let trailing = offer_control(offer, &title, enabled, model, cx);
    let error = match &offer.state {
        OfferState::Failed(error) | OfferState::Unavailable(error) => Some(error.clone()),
        _ => None,
    };
    div()
        .debug_selector(move || format!("onboarding-offer-card-{name}"))
        .flex()
        .flex_col()
        .gap(m.spacing3())
        .p(m.spacing6())
        .rounded(m.radius_lg())
        .border_1()
        .border_color(theme.border)
        .bg(theme.sunken())
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.spacing5())
                .child(IconGlyph::new(icon, m.icon_lg(), theme.accent))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(m.font_emphasis())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg)
                        .child(title),
                )
                .child(div().flex_none().child(trailing)),
        )
        .child(
            div()
                .text_size(m.font_footnote())
                .line_height(m.scaled(15.0))
                .text_color(theme.fg_muted)
                .child(summary),
        )
        .children(permissions.map(|permissions| {
            div()
                .text_size(m.font_caption())
                .line_height(m.scaled(14.0))
                .text_color(theme.fg_muted)
                .child(permissions)
        }))
        .children(error.map(|error| problem(error.into(), model)))
        .into_any_element()
}

/// A card's checkbox, or how far its extension has come.
fn offer_control(
    offer: &Offer,
    title: &SharedString,
    enabled: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let name = offer.name;
    if enabled {
        return confirmation(tr!("Enabled"), model).into_any_element();
    }
    let label = match offer.state {
        OfferState::Loading => tr!("Loading…"),
        OfferState::Queued => tr!("Waiting…"),
        OfferState::Installing => tr!("Installing…"),
        OfferState::Enabling => tr!("Enabling…"),
        _ => {
            let style = Style {
                theme: &model.theme,
                metrics: &m,
            };
            return controls::checkbox(
                style,
                &format!("onboarding-offer-{name}"),
                &tr!("Install %@", title.as_str()),
                offer.selected,
                false,
                cx.listener(move |model, _, _, cx| model.toggle_offer(name, cx)),
            );
        }
    };
    div()
        .text_size(m.font_footnote())
        .text_color(model.theme.fg_muted)
        .child(label)
        .into_any_element()
}

fn agents(model: &AppModel) -> Div {
    column(model)
        .child(intro(
            tr!("Made for AI coding agents"),
            tr!(
                "Run Claude Code, Codex, Cursor, OpenCode, and more in any terminal. Muxy sees what each one is doing, with no setup."
            ),
            model,
        ))
        .child(points(
            [
                point(
                    Icon::Eye,
                    tr!("Working, waiting, or done"),
                    tr!("Tabs and projects show each agent's state, and waiting comes first."),
                    model,
                ),
                point(
                    Icon::Bell,
                    tr!("Know when you're needed"),
                    tr!("Get a notification when an agent needs you or finishes. Click it to jump right there."),
                    model,
                ),
                point(
                    Icon::Dashboard,
                    tr!("All of them at a glance"),
                    tr!(
                        "Press %@ for the agents panel, with every running agent in one list.",
                        chord_text(ShortcutId::ToggleAgentsPanel, model).as_str()
                    ),
                    model,
                ),
            ],
            model,
        ))
}

fn composer(model: &AppModel) -> Div {
    column(model)
        .child(intro(
            tr!("Room to write"),
            tr!(
                "Press %@ to open the composer, a text box for long prompts and commands. It's made for talking to AI agents.",
                chord_text(ShortcutId::ToggleComposer, model).as_str()
            ),
            model,
        ))
        .child(points(
            [
                point(
                    Icon::File,
                    tr!("Files and images"),
                    tr!("Attach, drop, or paste them. Files go in as paths, and AI tools can read your images."),
                    model,
                ),
                point(
                    Icon::Columns,
                    tr!("Send it where it's needed"),
                    tr!("Send to the active terminal, or to every terminal you can see in the project."),
                    model,
                ),
                point(
                    Icon::Pencil,
                    tr!("Your draft stays"),
                    tr!("Each project keeps its own draft, even across restarts."),
                    model,
                ),
            ],
            model,
        ))
}

fn remote(model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let added = model
        .onboarding
        .tour
        .as_ref()
        .and_then(|tour| tour.added_server.clone());
    column(model)
        .child(intro(
            tr!("Bring your servers in"),
            tr!(
                "Add a computer over SSH and its projects sit beside your local ones. Its terminals keep running when you disconnect."
            ),
            model,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap(model.metrics.spacing6())
                .child(action(
                    "onboarding-add-server",
                    controls::Button::secondary(&tr!("Add a Server…")),
                    model,
                    cx.listener(|model, _, _, cx| model.add_server_from_onboarding(cx)),
                ))
                .children(added.map(|name| confirmation(tr!("%@ added", name), model))),
        )
        .child(points(
            [
                point(
                    Icon::Sparkles,
                    tr!("Little to set up"),
                    tr!("When a server lacks Muxy, the app offers to install it there."),
                    model,
                ),
                point(
                    Icon::Network,
                    tr!("No SSH? Pair instead"),
                    tr!("Another computer's Devices tab gives you a link to pair with."),
                    model,
                ),
                point(
                    Icon::Tower,
                    tr!("Find them later"),
                    tr!("The Devices section at the bottom of the sidebar manages every server."),
                    model,
                ),
            ],
            model,
        ))
}

fn mobile(art_shown: bool, model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let m = model.metrics;
    let tour = model.onboarding.tour.as_ref();
    let access = model.device_access();
    column(model)
        .gap(m.spacing7())
        .child(intro(
            tr!("Take your terminals with you"),
            tr!(
                "Watch agents, answer their questions, and type from your phone. It connects straight to your computer, with no cloud and no account."
            ),
            model,
        ))
        .child(numbered("1", tr!("Get the app"), model))
        .child(
            div()
                .flex()
                .gap(m.spacing6())
                .child(store_code(
                    "onboarding-app-store",
                    tr!("App Store"),
                    APP_STORE_URL,
                    tour.and_then(|tour| tour.store_codes[0].clone()),
                    model,
                    cx,
                ))
                .child(store_code(
                    "onboarding-google-play",
                    tr!("Google Play"),
                    PLAY_STORE_URL,
                    tour.and_then(|tour| tour.store_codes[1].clone()),
                    model,
                    cx,
                )),
        )
        .child(numbered("2", tr!("Pair your phone"), model))
        .child(pairing_controls(model, cx))
        .children(access.error.clone().map(|error| problem(error.into(), model)))
        .children(
            access
                .pairing
                .as_ref()
                .filter(|pairing| !art_shown || pairing.qr().is_none())
                .map(|pairing| crate::views::devices::code(pairing.qr(), m)),
        )
}

/// What pairing needs next: its button, or a way to withdraw the code on show.
fn pairing_controls(model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let m = model.metrics;
    let theme = &model.theme;
    let tour = model.onboarding.tour.as_ref();
    let access = model.device_access();
    let column = div().flex().flex_col().gap(m.spacing4());
    if access.pairing.is_some() {
        return column
            .child(
                div()
                    .text_size(m.font_footnote())
                    .line_height(m.scaled(15.0))
                    .text_color(theme.fg_muted)
                    .child(tr!(
                        "Scan the code with the Muxy app. It works once and expires in 5 minutes."
                    )),
            )
            .child(div().flex().child(action(
                "onboarding-cancel-pairing",
                controls::Button::secondary(&tr!("Cancel")),
                model,
                cx.listener(|model, _, _, cx| model.cancel_onboarding_pairing(cx)),
            )));
    }
    let paired = tour.and_then(|tour| tour.pair.paired.clone());
    let wanted = tour.is_some_and(|tour| tour.pair.wanted);
    let ready = model.mobile_available() && !access.busy && !wanted;
    let label = if paired.is_some() {
        tr!("Pair Another Phone")
    } else {
        tr!("Pair a Phone")
    };
    column
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.spacing6())
                .child(action(
                    "onboarding-pair",
                    controls::Button::secondary(&label).enabled(ready),
                    model,
                    cx.listener(|model, _, _, cx| model.pair_phone_in_onboarding(cx)),
                ))
                .children(paired.map(|name| confirmation(tr!("Paired with %@", name), model))),
        )
        .child(
            div()
                .text_size(m.font_caption())
                .line_height(m.scaled(14.0))
                .text_color(theme.fg_muted)
                .child(if model.mobile_available() {
                    tr!(
                        "This turns on Allow other devices. A paired phone gets full terminal access; revoke it anytime in the Devices tab."
                    )
                } else {
                    tr!("Connect to this computer's server to manage device access.")
                }),
        )
}

fn numbered(number: &'static str, title: SharedString, model: &AppModel) -> Div {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .items_center()
        .gap(m.spacing4())
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(m.scaled(16.0))
                .rounded_full()
                .bg(theme.accent_soft)
                .text_size(m.font_caption())
                .font_weight(FontWeight::BOLD)
                .text_color(theme.accent)
                .child(number),
        )
        .child(
            div()
                .text_size(m.font_body())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg)
                .child(title),
        )
}

/// A store's QR code and name, which also opens the store when clicked.
fn store_code(
    id: &'static str,
    name: SharedString,
    url: &'static str,
    code: Option<Arc<QrCode>>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .id(id)
        .debug_selector(move || id.into())
        .accessibility_id(id)
        .role(gpui::Role::Link)
        .aria_label(name.clone())
        .flex()
        .flex_col()
        .items_center()
        .gap(m.spacing3())
        .p(m.spacing4())
        .rounded(m.radius_lg())
        .border_1()
        .border_color(theme.border)
        .cursor_pointer()
        .hover(|style| style.bg(theme.hover))
        .focus(|style| style.border_color(theme.accent))
        .button_interaction(cx.listener(move |_, _, _, cx| cx.open_url(url)))
        .children(code.map(|code| {
            let side = units(code.size() + 2 * QUIET_ZONE) * STORE_QR_MODULE;
            crate::views::devices::qr(code)
                .flex_none()
                .size(m.scaled(side))
        }))
        .child(
            div()
                .text_size(m.font_caption())
                .text_color(theme.fg_muted)
                .child(name),
        )
}

fn shortcuts(model: &AppModel) -> Div {
    let m = model.metrics;
    let rows = [
        (ShortcutId::ToggleCommandPalette, tr!("Command palette")),
        (ShortcutId::AddProject, tr!("Add a project")),
        (ShortcutId::NewTab, tr!("New tab")),
        (ShortcutId::SplitRight, tr!("Split right")),
        (ShortcutId::SplitDown, tr!("Split down")),
        (ShortcutId::ToggleSidebar, tr!("Show or hide the sidebar")),
        (ShortcutId::ToggleAgentsPanel, tr!("Agents panel")),
        (ShortcutId::ToggleComposer, tr!("Composer")),
        (ShortcutId::OpenSettings, tr!("Settings")),
    ];
    column(model)
        .gap(m.spacing7())
        .child(intro(
            tr!("A few keys to remember"),
            tr!(
                "Everything else is in the command palette. Change any shortcut in Settings → Keyboard."
            ),
            model,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(m.spacing2())
                .children(rows.into_iter().map(|(id, label)| {
                    shortcut_row(id, label, model)
                })),
        )
}

fn shortcut_row(id: ShortcutId, label: SharedString, model: &AppModel) -> Div {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(m.spacing6())
        .h(m.scaled(28.0))
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_size(m.font_body())
                .text_color(theme.fg)
                .child(label),
        )
        .child(div().flex().flex_none().gap(m.spacing1()).children(
            key_caps(id, model).into_iter().map(|cap| {
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .min_w(m.scaled(20.0))
                    .h(m.scaled(20.0))
                    .px(m.spacing2())
                    .rounded(m.radius_sm())
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.sunken())
                    .text_size(m.font_footnote())
                    .text_color(theme.fg_muted)
                    .child(cap)
            }),
        ))
}
