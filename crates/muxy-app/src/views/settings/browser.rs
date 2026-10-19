//! Settings → Browser: the built-in browser, search, home page, profiles,
//! and importing from other browsers.

use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use muxy_app_core::browser::SearchEngine;
use muxy_core::capability::Feature;
use muxy_ui::controls::{self, Choice};
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

use super::{Category, Change, SettingsEvent, SettingsView};

/// A browser profile as Settings lists it.
#[derive(Clone, Debug)]
pub(crate) struct Profile {
    pub id: String,
    pub name: String,
    pub default: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum BrowserChange {
    Enabled(bool),
    TerminalLinks(bool),
    SearchEngine(SearchEngine),
    DefaultProfile(String),
    AddProfile(String),
    RenameProfile(String, String),
    ClearProfile(String),
    DeleteProfile(String),
    /// Imports from the listed browser at `source` into the default profile.
    Import {
        source: usize,
        cookies: bool,
    },
}

pub(super) fn rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let settings = &view.snapshot.settings.browser;
    let mut rows = Vec::new();
    let browser = |change| Change::Browser(change);
    if let Some(reason) = view.snapshot.capabilities.reason(Feature::BrowserPanes) {
        if view.matches(Category::Browser, tr_key!("Enable built-in browser")) {
            rows.push(view.note(&reason, false).into_any_element());
        }
        return rows;
    }
    if view.matches(Category::Browser, tr_key!("Enable built-in browser")) {
        rows.push(view.row(
            "browser-enabled",
            tr_key!("Enable built-in browser"),
            view.toggle(
                "browser-enabled",
                settings.enabled,
                browser(BrowserChange::Enabled(!settings.enabled)),
                cx,
            ),
        ));
        if !settings.enabled {
            rows.push(
                view.note(
                    &tr!("The built-in browser is off. Browser tabs and browser commands are disabled, and terminal links open in your system browser."),
                    false,
                )
                .into_any_element(),
            );
        }
    }
    if view.matches(
        Category::Browser,
        tr_key!("Open terminal links in the browser"),
    ) {
        rows.push(view.row(
            "browser-terminal-links",
            tr_key!("Open terminal links in the browser"),
            view.toggle(
                "browser-terminal-links",
                settings.open_terminal_links,
                browser(BrowserChange::TerminalLinks(!settings.open_terminal_links)),
                cx,
            ),
        ));
    }
    if view.matches(Category::Browser, tr_key!("Search engine")) {
        let choices: Vec<_> = SearchEngine::ALL
            .iter()
            .map(|engine| Choice::new(engine_id(*engine), engine.name()))
            .collect();
        rows.push(view.row_with(
            "browser-search-engine",
            tr_key!("Search engine"),
            controls::segmented(
                view.style(),
                "browser-search-engine",
                &choices,
                engine_id(settings.search_engine),
                cx.listener(|_, selected: &gpui::SharedString, _, cx| {
                    if let Some(engine) = SearchEngine::ALL
                        .into_iter()
                        .find(|engine| engine_id(*engine) == selected.as_ref())
                    {
                        cx.emit(SettingsEvent::Change(Change::Browser(
                            BrowserChange::SearchEngine(engine),
                        )));
                    }
                }),
            ),
            true,
        ));
    }
    if view.matches(Category::Browser, tr_key!("Home page")) {
        rows.push(view.row(
            "browser-home-page",
            tr_key!("Home page"),
            view.field("browser-home-page"),
        ));
    }
    if view.matches(Category::Browser, tr_key!("Profiles")) {
        rows.push(view.row_with(
            "browser-profiles",
            tr_key!("Profiles"),
            profiles(view, cx),
            true,
        ));
    }
    if view.matches(Category::Browser, tr_key!("Import from another browser")) {
        rows.push(view.row_with(
            "browser-import",
            tr_key!("Import from another browser"),
            sources(view, cx),
            true,
        ));
    }
    rows
}

fn engine_id(engine: SearchEngine) -> &'static str {
    match engine {
        SearchEngine::Google => "google",
        SearchEngine::DuckDuckGo => "duckduckgo",
        SearchEngine::Bing => "bing",
        SearchEngine::Brave => "brave",
        SearchEngine::Startpage => "startpage",
    }
}

fn profiles(view: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let name = view.fields["browser-profile-name"]
        .read(cx)
        .text()
        .trim()
        .to_owned();
    let mut list = div().flex().flex_col().gap(px(6.0)).w_full();
    for profile in &view.snapshot.browser_profiles {
        list = list.child(profile_row(view, profile, &name, cx));
    }
    let added = name.clone();
    list.child(
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .pt(px(4.0))
            .child(view.field("browser-profile-name"))
            .child(controls::button(
                view.style(),
                "browser-add-profile",
                &tr!("Add Profile"),
                true,
                cx.listener(move |view, _, _, cx| {
                    view.clear_browser_profile_name(cx);
                    cx.emit(SettingsEvent::Change(Change::Browser(
                        BrowserChange::AddProfile(added.clone()),
                    )));
                }),
            )),
    )
    .child(
        view.note(
            &tr!("Each profile keeps its own cookies, cache, history, and logins. Pick a profile per tab from the browser toolbar."),
            false,
        ),
    )
    .into_any_element()
}

/// A profile with what can be done to it.
fn profile_row(
    view: &SettingsView,
    profile: &Profile,
    name: &str,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let theme = &view.theme;
    let metrics = view.style().metrics;
    let id = profile.id.clone();
    let mut buttons = div().flex().flex_wrap().gap(px(6.0));
    if !profile.default {
        let chosen = id.clone();
        buttons = buttons.child(controls::button(
            view.style(),
            &format!("browser-default-{id}"),
            &tr!("Make Default"),
            true,
            cx.listener(move |_, _, _, cx| {
                cx.emit(SettingsEvent::Change(Change::Browser(
                    BrowserChange::DefaultProfile(chosen.clone()),
                )));
            }),
        ));
    }
    let (renamed, new_name) = (id.clone(), name.to_owned());
    buttons = buttons.child(controls::button(
        view.style(),
        &format!("browser-rename-{id}"),
        &tr!("Rename to Name Below"),
        !name.is_empty(),
        cx.listener(move |view, _, _, cx| {
            view.clear_browser_profile_name(cx);
            cx.emit(SettingsEvent::Change(Change::Browser(
                BrowserChange::RenameProfile(renamed.clone(), new_name.clone()),
            )));
        }),
    ));
    let (cleared, label) = (id.clone(), profile.name.clone());
    buttons = buttons.child(controls::button(
        view.style(),
        &format!("browser-clear-{id}"),
        &tr!("Clear Data"),
        true,
        cx.listener(move |_, _, window, cx| {
            confirm(
                BrowserChange::ClearProfile(cleared.clone()),
                &tr!("Clear data for “%@”?", &label),
                &tr!("This signs out and removes all cookies, cache, history, and logins for this profile, including imported ones."),
                &tr!("Clear Data"),
                window,
                cx,
            );
        }),
    ));
    if !profile.default {
        let (deleted, label) = (id.clone(), profile.name.clone());
        buttons = buttons.child(controls::button(
            view.style(),
            &format!("browser-delete-{id}"),
            &tr!("Delete"),
            true,
            cx.listener(move |_, _, window, cx| {
                confirm(
                    BrowserChange::DeleteProfile(deleted.clone()),
                    &tr!("Delete “%@”?", &label),
                    &tr!("This permanently deletes the profile's cookies, history, and browsing data. Its tabs use the default profile."),
                    &tr!("Delete"),
                    window,
                    cx,
                );
            }),
        ));
    }
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(metrics.font_body())
                .child(profile.name.clone())
                .when(profile.default, |row| {
                    row.child(
                        div()
                            .px(px(6.0))
                            .rounded(metrics.radius_sm())
                            .bg(theme.accent_soft)
                            .text_size(metrics.font_caption())
                            .text_color(theme.fg)
                            .child(tr!("Default")),
                    )
                }),
        )
        .child(buttons)
        .into_any_element()
}

fn sources(view: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let metrics = view.style().metrics;
    let mut list = div().flex().flex_col().gap(px(6.0)).w_full();
    if view.snapshot.browser_sources.is_empty() {
        return list
            .child(view.note(
                &tr!("No other browsers were found on this computer."),
                false,
            ))
            .into_any_element();
    }
    for (index, (source, has_cookies)) in view.snapshot.browser_sources.iter().enumerate() {
        let mut buttons = div().flex().flex_wrap().gap(px(6.0));
        let kinds = [(false, tr!("History & Bookmarks")), (true, tr!("Cookies"))];
        for (cookies, label) in kinds
            .into_iter()
            .filter(|(cookies, _)| !cookies || *has_cookies)
        {
            buttons = buttons.child(controls::button(
                view.style(),
                &format!("browser-import-{index}-{cookies}"),
                &label,
                true,
                cx.listener(move |_, _, _, cx| {
                    cx.emit(SettingsEvent::Change(Change::Browser(
                        BrowserChange::Import {
                            source: index,
                            cookies,
                        },
                    )));
                }),
            ));
        }
        list = list.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(div().text_size(metrics.font_body()).child(source.clone()))
                .child(buttons),
        );
    }
    list.child(view.note(
        &tr!("Imports go into the default profile. Cookies bring your sign-ins; macOS may ask to allow access to the browser's key."),
        false,
    ))
    .into_any_element()
}

impl SettingsView {
    fn clear_browser_profile_name(&mut self, cx: &mut Context<Self>) {
        self.dirty.remove("browser-profile-name");
        self.fields["browser-profile-name"].update(cx, |input, cx| input.set_text("", cx));
    }
}

/// Asks before a change that deletes data, and makes it once confirmed.
fn confirm(
    change: BrowserChange,
    title: &str,
    message: &str,
    label: &str,
    window: &mut Window,
    cx: &mut Context<SettingsView>,
) {
    let (sender, answer) = async_channel::bounded(1);
    let Ok(dialog) =
        muxy_ui::dialog::confirm(window, cx, title, message, label, None, move |response| {
            let _ = sender.try_send(matches!(
                response,
                muxy_ui::dialog::ConfirmationResponse::Confirmed { .. }
            ));
        })
    else {
        return;
    };
    cx.spawn(async move |view, cx| {
        let _dialog = dialog;
        if answer.recv().await.unwrap_or(false) {
            let _ = view.update(cx, |_, cx| {
                cx.emit(SettingsEvent::Change(Change::Browser(change)));
            });
        }
    })
    .detach();
}
