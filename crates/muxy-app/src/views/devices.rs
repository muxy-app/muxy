//! The Devices tab's This Computer section: whether phones and other
//! computers may reach this computer's server, the code that pairs one, and
//! the devices paired with it.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Bounds, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, Styled, canvas, div, point, px, size,
};
use muxy_app_core::qr::{QUIET_ZONE, QrCode};
use muxy_protocol::{DeviceKind, ListenerStatus, PairedDevice, PairingOffer, RemoteAccessState};
use muxy_ui::controls;
use muxy_ui::tr;

use super::servers_tab::{card, style};
use crate::model::AppModel;

const CODE_SIZE: f32 = 196.0;

/// A pairing offer as shown: its link, and the QR code encoded once.
#[derive(Clone)]
pub(crate) struct Pairing {
    link: String,
    code: Option<Arc<QrCode>>,
}

impl Pairing {
    pub(crate) fn new(offer: &PairingOffer) -> Self {
        let link = offer.invite.to_link();
        Self {
            code: QrCode::encode(&link).map(Arc::new),
            link,
        }
    }

    pub(crate) fn link(&self) -> &str {
        &self.link
    }
}

/// The section's title and its card, or why it can't show yet.
pub(crate) fn this_computer(model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let access = model.device_access();
    let body = if !model.mobile_available() {
        note(
            &tr!("Connect to this computer's server to manage device access."),
            false,
            model,
        )
    } else if let Some(state) = &access.state {
        card(model)
            .child(row(
                "devices-access",
                tr!("Allow other devices"),
                Some(tr!(
                    "Let paired phones and computers reach this computer's server over the local network or a VPN. A paired phone gets full terminal access; a paired computer can also run commands and change or stop this server."
                )),
                controls::toggle(
                    style(model),
                    "devices-access",
                    state.settings.enabled,
                    cx.listener({
                        let enabled = state.settings.enabled;
                        move |model, _, _, cx| model.set_device_access(!enabled, cx)
                    }),
                ),
                true,
                model,
            ))
            .child(row(
                "devices-port",
                tr!("Port"),
                Some(tr!("From 1024 to 65535. Press Return to save.")),
                model.servers_tab.port_input().map_or_else(
                    || div().into_any_element(),
                    |input| controls::text_field(style(model), "devices-port", input, Some(96.0)),
                ),
                false,
                model,
            ))
            .child(row(
                "devices-status",
                tr!("Status"),
                None,
                status(state, model),
                false,
                model,
            ))
            .child(stacked(
                "devices-pair",
                tr!("Pair a device"),
                tr!("Show a one-time code to scan with the Muxy phone app, or a link to paste on another computer."),
                pairing(state, model, cx),
                model,
            ))
            .child(stacked(
                "devices-paired",
                tr!("Paired with this computer"),
                tr!("Phones and computers that can connect. Revoking one disconnects it immediately."),
                devices(state, model, cx),
                model,
            ))
            .into_any_element()
    } else {
        note(&tr!("Loading device access…"), false, model)
    };
    div()
        .debug_selector(|| "devices-this-computer".into())
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(m.spacing4())
        .child(
            div()
                .text_color(theme.fg_muted)
                .font_weight(FontWeight::SEMIBOLD)
                .child(tr!("This Computer")),
        )
        .children(
            access
                .error
                .as_deref()
                .map(|error| note(error, true, model)),
        )
        .child(body)
        .into_any_element()
}

fn note(text: &str, error: bool, model: &AppModel) -> AnyElement {
    muxy_ui::form::note(style(model), text, error).into_any_element()
}

/// A label and its description beside the control.
fn row(
    id: &'static str,
    label: SharedString,
    description: Option<SharedString>,
    control: AnyElement,
    first: bool,
    model: &AppModel,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .debug_selector(move || format!("{id}-row"))
        .min_w(px(0.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(m.spacing6())
        .px(m.spacing6())
        .py(m.spacing5())
        .when(!first, |row| row.border_t_1().border_color(theme.border))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.0))
                .gap(m.spacing1())
                .child(label)
                .children(description.map(|description| {
                    div()
                        .text_size(m.font_footnote())
                        .text_color(theme.fg_muted)
                        .child(description)
                })),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

/// A label and its description over the content.
fn stacked(
    id: &'static str,
    label: SharedString,
    description: SharedString,
    content: AnyElement,
    model: &AppModel,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .debug_selector(move || format!("{id}-row"))
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(m.spacing4())
        .px(m.spacing6())
        .py(m.spacing5())
        .border_t_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(m.spacing1())
                .child(label)
                .child(
                    div()
                        .text_size(m.font_footnote())
                        .text_color(theme.fg_muted)
                        .child(description),
                ),
        )
        .child(div().min_w(px(0.0)).child(content))
        .into_any_element()
}

fn status(state: &RemoteAccessState, model: &AppModel) -> AnyElement {
    let (text, failed) = match (&state.status, state.settings.enabled) {
        (ListenerStatus::Listening, _) => {
            (tr!("Listening on port %lld", state.settings.port), false)
        }
        (ListenerStatus::Failed(message), _) => (tr!("Not listening: %@", message), true),
        (ListenerStatus::Disabled, true) => (tr!("Starting"), false),
        (ListenerStatus::Disabled, false) => (tr!("Off"), false),
        (ListenerStatus::Unrecognized(_), _) => (tr!("Update Muxy to see this status"), false),
    };
    div()
        .debug_selector(|| "devices-status".into())
        .text_color(if failed {
            model.theme.danger
        } else {
            model.theme.fg_muted
        })
        .child(text)
        .into_any_element()
}

fn pairing(state: &RemoteAccessState, model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let access = model.device_access();
    let Some(pairing) = &access.pairing else {
        return div()
            .flex()
            .child(controls::button(
                style(model),
                "devices-show-code",
                &tr!("Show Pairing Code"),
                state.status == ListenerStatus::Listening && !access.busy,
                cx.listener(|model, _, _, cx| model.show_pairing_code(cx)),
            ))
            .into_any_element();
    };
    div()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(code(pairing.code.clone()))
        .child(div().min_w(px(0.0)).text_color(model.theme.fg_muted).child(tr!(
            "Scan it with the Muxy phone app, or on another computer choose Pair with Link… in the Devices section and paste the link. The code works once and expires in 5 minutes."
        )))
        .child(
            div()
                .flex()
                .gap(px(8.0))
                .child(controls::button(
                    style(model),
                    "devices-copy-link",
                    &tr!("Copy Link"),
                    true,
                    cx.listener(|model, _, _, cx| model.copy_pairing_link(cx)),
                ))
                .child(controls::button(
                    style(model),
                    "devices-cancel-code",
                    &tr!("Cancel"),
                    true,
                    cx.listener(|model, _, _, cx| model.cancel_pairing(cx)),
                )),
        )
        .into_any_element()
}

/// Dark modules on a light square in every theme, merged into one quad per run.
fn code(code: Option<Arc<QrCode>>) -> AnyElement {
    let Some(code) = code else {
        return div()
            .child(tr!("The pairing code is too long to show."))
            .into_any_element();
    };
    let painted = canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            window.paint_quad(gpui::fill(bounds, gpui::white()));
            let span = code.size() + 2 * QUIET_ZONE;
            let module = (bounds.size.width / units(span)).floor();
            let inset = (bounds.size.width - module * units(span)) / 2.0;
            let at = |x: usize, y: usize| {
                point(
                    bounds.origin.x + inset + module * units(x + QUIET_ZONE),
                    bounds.origin.y + inset + module * units(y + QUIET_ZONE),
                )
            };
            for y in 0..code.size() {
                let mut x = 0;
                while x < code.size() {
                    if !code.dark(x, y) {
                        x += 1;
                        continue;
                    }
                    let start = x;
                    while code.dark(x, y) {
                        x += 1;
                    }
                    let run = size(module * units(x - start), module);
                    window.paint_quad(gpui::fill(Bounds::new(at(start, y), run), gpui::black()));
                }
            }
        },
    )
    .size(px(CODE_SIZE));
    div()
        .debug_selector(|| "devices-code".into())
        .child(painted)
        .into_any_element()
}

fn devices(state: &RemoteAccessState, model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    if state.devices.is_empty() {
        return div()
            .text_color(model.theme.fg_muted)
            .child(tr!("No paired devices yet."))
            .into_any_element();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let busy = model.device_access().busy;
    div()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .children(state.devices.iter().map(|device| {
            let id = device.id;
            div()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .debug_selector(move || format!("devices-paired-{id}"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(div().truncate().child(device.name.clone()))
                        .child(
                            div()
                                .truncate()
                                .text_color(model.theme.fg_muted)
                                .child(format!(
                                    "{} · {}",
                                    kind(device.kind),
                                    presence(device, now)
                                )),
                        ),
                )
                .child(div().flex_none().child(controls::button(
                    style(model),
                    &format!("devices-revoke-{id}"),
                    &tr!("Revoke"),
                    !busy,
                    cx.listener(move |model, _, window, cx| {
                        model.confirm_revoke(id, window.window_handle(), cx);
                    }),
                )))
        }))
        .into_any_element()
}

fn kind(kind: DeviceKind) -> SharedString {
    match kind {
        DeviceKind::Computer => tr!("Computer"),
        DeviceKind::Phone | DeviceKind::Unrecognized(_) => tr!("Phone"),
    }
}

fn presence(device: &PairedDevice, now: u64) -> SharedString {
    if device.connected {
        return tr!("Connected");
    }
    let Some(seen) = device.last_seen else {
        return tr!("Never connected");
    };
    match now.saturating_sub(seen) {
        0..60 => tr!("Last seen just now"),
        seconds @ 60..3_600 => tr!("Last seen %lld min ago", seconds / 60),
        seconds @ 3_600..86_400 => tr!("Last seen %lld h ago", seconds / 3_600),
        seconds => tr!("Last seen %lld d ago", seconds / 86_400),
    }
}

fn units(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}
