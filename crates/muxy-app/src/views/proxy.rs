//! The status bar's Proxy control and its popover, where the app's one proxy
//! is set up and applied to the whole app.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, InteractiveElement,
    IntoElement, MouseButton, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Window, canvas, div, px,
};
use muxy_app_core::settings::{DEFAULT_PROXY_BYPASS, ProxyKind, ProxySettings};
use muxy_ui::components::{ButtonInteraction, IconGlyph};
use muxy_ui::controls::{self, Choice, Style};
use muxy_ui::icon::Icon;
use muxy_ui::l10n::tr_key;
use muxy_ui::popover;
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use muxy_ui::tr;

use super::overlays::Overlay;
use crate::model::AppModel;

pub(crate) struct ProxyForm {
    kind: ProxyKind,
    host: Entity<TextInput>,
    port: Entity<TextInput>,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    bypass: Entity<TextInput>,
    global: bool,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl ProxyForm {
    fn submitted(&self, cx: &gpui::App) -> Result<ProxySettings, String> {
        let text = |input: &Entity<TextInput>| input.read(cx).text().trim().to_owned();
        let port = text(&self.port);
        let settings = ProxySettings {
            kind: self.kind,
            host: text(&self.host),
            port: port
                .parse()
                .map_err(|_| tr!("Enter a port from 1 to 65535").to_string())?,
            username: text(&self.username),
            password: self.password.read(cx).text().to_owned(),
            bypass: text(&self.bypass),
            global: self.global,
        };
        settings.validate().map_err(|_| {
            if settings.host.is_empty() || settings.host.contains(['/', '@', ' ']) {
                tr!("Enter the proxy's host name or address, without http://").to_string()
            } else if settings.port == 0 {
                tr!("Enter a port from 1 to 65535").to_string()
            } else if settings.username.is_empty() {
                tr!("Enter the user name for this password").to_string()
            } else {
                tr!("The proxy settings contain characters that can't be used").to_string()
            }
        })?;
        Ok(settings)
    }
}

pub(crate) fn control(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    let anchor = model.proxy.anchor.clone();
    let global = model.proxy.global();
    div()
        .id("proxy-status")
        .debug_selector(|| "proxy-status".into())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .h_full()
        .gap(px(5.0))
        .pl(px(8.0))
        .border_l_1()
        .border_color(theme.border)
        .text_color(if global { theme.fg } else { theme.fg_muted })
        .cursor_pointer()
        .hover(|style| style.text_color(theme.fg))
        .focus(|style| style.bg(theme.hover))
        .button_interaction(cx.listener(|model, _, window, cx| {
            model.toggle_proxy_popover(window, cx);
        }))
        .child(
            canvas(
                move |bounds, _, _| anchor.set(Some(bounds)),
                |_, (), _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(IconGlyph::new(
            Icon::Network,
            m.scaled(12.0),
            if global { theme.accent } else { theme.fg_muted },
        ))
        .child(
            div()
                .text_size(m.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .child(if global {
                    tr!("Proxy · On")
                } else {
                    tr!("Proxy")
                }),
        )
}

pub(crate) fn render(
    form: &ProxyForm,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    let width = m
        .scaled(380.0)
        .min((window.viewport_size().width - px(16.0)).max(px(0.0)));
    popover::surface(theme, m)
        .id("proxy-popover")
        .debug_selector(|| "proxy-popover".into())
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(
            cx.listener(|model, _: &super::menu::DismissMenu, _, cx| model.dismiss_overlay(cx)),
        )
        .on_key_down(
            cx.listener(|model, event: &gpui::KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if event.keystroke.key == "tab"
                    && !modifiers.control
                    && !modifiers.alt
                    && !modifiers.platform
                    && !modifiers.function
                {
                    let advance = |window: &mut Window| {
                        if modifiers.shift {
                            window.focus_prev();
                        } else {
                            window.focus_next();
                        }
                    };
                    advance(window);
                    let first = window.focused(cx);
                    while !model.overlay_focus.contains_focused(window, cx) {
                        advance(window);
                        if window.focused(cx) == first {
                            model.overlay_focus.focus(window);
                            break;
                        }
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }),
        )
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .w(width)
        .max_h((window.viewport_size().height - m.status_bar_height() - px(16.0)).max(px(0.0)))
        .overflow_y_scroll()
        .text_color(theme.fg)
        .child(
            popover::header(theme, m)
                .justify_between()
                .child(tr!("Proxy"))
                .child(controls::button(
                    style,
                    "close-proxy",
                    &tr!("Close"),
                    true,
                    cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
                )),
        )
        .child(fields(form, model, cx))
        .child(actions(model, cx))
        .into_any_element()
}

fn fields(form: &ProxyForm, model: &AppModel, cx: &mut Context<AppModel>) -> gpui::Div {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    let text = |id: &str, input: &Entity<TextInput>| controls::text_field(style, id, input, None);
    let field = super::git::form_field;
    popover::body(m)
        .gap(m.scaled(12.0))
        .child(field(
            &tr!("Type"),
            controls::segmented(
                style,
                "proxy-kind",
                &[Choice::new("http", "HTTP"), Choice::new("socks5", "SOCKS5")],
                match form.kind {
                    ProxyKind::Http => "http",
                    ProxyKind::Socks5 => "socks5",
                },
                cx.listener(|model, selected: &SharedString, _, cx| {
                    model.set_proxy_form_kind(
                        if selected.as_ref() == "socks5" {
                            ProxyKind::Socks5
                        } else {
                            ProxyKind::Http
                        },
                        cx,
                    );
                }),
            ),
            model,
        ))
        .child(
            div()
                .flex()
                .gap(m.spacing4())
                .child(field(&tr!("Host"), text("proxy-host", &form.host), model).flex_1())
                .child(
                    field(&tr!("Port"), text("proxy-port", &form.port), model).w(m.scaled(96.0)),
                ),
        )
        .child(
            div()
                .flex()
                .gap(m.spacing4())
                .child(field(&tr!("User"), text("proxy-username", &form.username), model).flex_1())
                .child(
                    field(
                        &tr!("Password"),
                        text("proxy-password", &form.password),
                        model,
                    )
                    .flex_1(),
                ),
        )
        .child(field(
            &tr!("No proxy for"),
            text("proxy-bypass", &form.bypass),
            model,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.spacing3())
                .child(controls::toggle(
                    style,
                    "proxy-global",
                    form.global,
                    cx.listener(|model, _, _, cx| model.toggle_proxy_form_global(cx)),
                ))
                .child(
                    div()
                        .text_size(m.font_footnote())
                        .child(tr!("Use for the whole app")),
                ),
        )
        .children(notes(form, model))
}

/// What the whole-app choice covers, and anything that stops the proxy.
fn notes(form: &ProxyForm, model: &AppModel) -> Vec<gpui::Div> {
    let m = model.metrics;
    let theme = &model.theme;
    let note = |color| div().text_size(m.font_footnote()).text_color(color);
    let mut notes = vec![note(theme.fg_muted).child(if form.global {
                    tr!(
                        "New terminals on this computer, Muxy's own downloads, and SSH to remote servers use the proxy. Terminals already running keep their settings."
                    )
                } else {
                    tr!(
                        "Right-click a terminal and choose Apply Proxy to use it there. The terminal restarts in the same folder."
                    )
                })];
    if model.proxy_needs_newer_server() {
        notes.push(
            note(theme.warning)
                .debug_selector(|| "proxy-server-outdated".into())
                .child(tr!(
                    "This computer's server is from an older Muxy and starts terminals without the proxy. Restart the server to update it."
                )),
        );
    }
    if let Some(error) = form.error.clone().or_else(|| model.proxy.error.clone()) {
        notes.push(
            note(theme.danger)
                .debug_selector(|| "proxy-error".into())
                .child(error),
        );
    }
    notes
}

/// Remove Proxy and Save.
fn actions(model: &AppModel, cx: &mut Context<AppModel>) -> gpui::Div {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    popover::footer(theme, m)
        .gap(m.spacing3())
        .when(model.proxy.settings.is_some(), |footer| {
            footer.child(
                controls::button(
                    style,
                    "proxy-remove",
                    &tr!("Remove Proxy"),
                    true,
                    cx.listener(|model, _, _, cx| model.remove_proxy(cx)),
                )
                .debug_selector(|| "proxy-remove".into()),
            )
        })
        .child(div().flex_1())
        .child(super::remote_servers::primary(
            "proxy-save",
            tr!("Save"),
            true,
            model,
            cx.listener(|model, _, _, cx| model.submit_proxy_form(cx)),
        ))
}

fn input(
    model: &AppModel,
    cx: &mut Context<AppModel>,
    placeholder: &'static str,
    text: &str,
    secure: bool,
) -> Entity<TextInput> {
    let style = InputStyle::field(&model.theme, &model.metrics);
    let text = text.to_owned();
    cx.new(|cx| {
        let input = TextInput::new(style, cx);
        if secure { input.secure() } else { input }
            .with_placeholder_key(placeholder)
            .with_text(text)
    })
}

impl AppModel {
    pub(crate) fn toggle_proxy_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if matches!(self.overlay, Some(Overlay::Proxy(_))) {
            self.dismiss_overlay(cx);
            return;
        }
        self.dismiss_overlay(cx);
        self.open_proxy_popover(window, cx);
    }

    pub(crate) fn open_proxy_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = self.proxy.settings.clone();
        let current = saved.clone().unwrap_or_else(|| ProxySettings {
            bypass: DEFAULT_PROXY_BYPASS.into(),
            ..ProxySettings::default()
        });
        let port = if current.port == 0 {
            String::new()
        } else {
            current.port.to_string()
        };
        let host = input(self, cx, "127.0.0.1", &current.host, false);
        let port = input(self, cx, "8080", &port, false);
        let username = input(self, cx, tr_key!("optional"), &current.username, false);
        let password = input(self, cx, tr_key!("optional"), &current.password, true);
        let bypass = input(self, cx, DEFAULT_PROXY_BYPASS, &current.bypass, false);
        let subscriptions = [&host, &port, &username, &password, &bypass]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |model, _, event, cx| match event {
                    InputEvent::Submitted => model.submit_proxy_form(cx),
                    InputEvent::Cancelled => model.dismiss_overlay(cx),
                    InputEvent::Changed => {
                        if let Some(Overlay::Proxy(form)) = &mut model.overlay {
                            form.error = None;
                            cx.notify();
                        }
                    }
                })
            })
            .collect();
        self.focus_later(host.focus_handle(cx), cx);
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Proxy(Box::new(ProxyForm {
            kind: current.kind,
            host,
            port,
            username,
            password,
            bypass,
            global: current.global,
            error: None,
            _subscriptions: subscriptions,
        })));
        self.overlay_focus.focus(window);
        cx.notify();
    }

    fn set_proxy_form_kind(&mut self, kind: ProxyKind, cx: &mut Context<Self>) {
        if let Some(Overlay::Proxy(form)) = &mut self.overlay {
            form.kind = kind;
            form.error = None;
            cx.notify();
        }
    }

    fn toggle_proxy_form_global(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::Proxy(form)) = &mut self.overlay {
            form.global = !form.global;
            form.error = None;
            cx.notify();
        }
    }

    fn submit_proxy_form(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::Proxy(form)) = &self.overlay else {
            return;
        };
        let result = form
            .submitted(cx)
            .and_then(|settings| self.set_proxy(Some(settings), cx));
        match result {
            Ok(()) => {
                self.dismiss_overlay(cx);
                self.show_notice(tr!("Proxy saved").to_string(), cx);
            }
            Err(error) => {
                if let Some(Overlay::Proxy(form)) = &mut self.overlay {
                    form.error = Some(error);
                }
                cx.notify();
            }
        }
    }

    fn remove_proxy(&mut self, cx: &mut Context<Self>) {
        match self.set_proxy(None, cx) {
            Ok(()) => {
                self.dismiss_overlay(cx);
                self.show_notice(tr!("Proxy removed").to_string(), cx);
            }
            Err(error) => {
                if let Some(Overlay::Proxy(form)) = &mut self.overlay {
                    form.error = Some(error);
                }
                cx.notify();
            }
        }
    }
}
