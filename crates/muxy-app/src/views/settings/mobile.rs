use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    AnyElement, Bounds, ClipboardItem, Context, InteractiveElement, IntoElement, ParentElement,
    SharedString, Styled, canvas, div, point, px, size,
};
use muxy_app_core::qr::{QUIET_ZONE, QrCode};
use muxy_protocol::{ListenerStatus, PairedDevice, PairingOffer, RemoteAccessState};
use muxy_ui::controls;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

use super::{Category, Change, SettingsEvent, SettingsView};

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
}

pub(super) fn rows(pane: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if !pane.snapshot.connected {
        if pane.matches(Category::Mobile, tr_key!("Server disconnected")) {
            rows.push(
                pane.note(
                    &tr!("Connect to the server to manage mobile access."),
                    false,
                )
                .into_any_element(),
            );
        }
        return rows;
    }
    if let Some(error) = &pane.snapshot.mobile_error
        && pane.matches(Category::Mobile, tr_key!("Mobile access error"))
    {
        rows.push(pane.note(error, true).into_any_element());
    }
    let Some(state) = &pane.snapshot.mobile else {
        if pane.matches(Category::Mobile, tr_key!("Loading mobile access")) {
            rows.push(
                pane.note(&tr!("Loading mobile access…"), false)
                    .into_any_element(),
            );
        }
        return rows;
    };
    if pane.matches(Category::Mobile, tr_key!("Allow mobile devices")) {
        rows.push(pane.row(
            "mobile-access",
            tr_key!("Allow mobile devices"),
            pane.toggle(
                "mobile-access",
                state.settings.enabled,
                Change::MobileAccess(!state.settings.enabled),
                cx,
            ),
        ));
    }
    if pane.matches(Category::Mobile, tr_key!("Port")) {
        rows.push(pane.row("mobile-port", tr_key!("Port"), pane.field("mobile-port")));
    }
    if pane.matches(Category::Mobile, tr_key!("Status")) {
        rows.push(pane.row("mobile-status", tr_key!("Status"), status(pane, state)));
    }
    if pane.matches(Category::Mobile, tr_key!("Pair a phone")) {
        rows.push(pane.row_with(
            "mobile-pair",
            tr_key!("Pair a phone"),
            pairing(pane, state, cx),
            true,
        ));
    }
    if pane.matches(Category::Mobile, tr_key!("Paired devices")) {
        rows.push(pane.row_with(
            "mobile-devices",
            tr_key!("Paired devices"),
            devices(pane, state, cx),
            true,
        ));
    }
    rows
}

fn status(pane: &SettingsView, state: &RemoteAccessState) -> AnyElement {
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
        .debug_selector(|| "settings-mobile-status".into())
        .text_color(if failed {
            pane.theme.danger
        } else {
            pane.theme.fg_muted
        })
        .child(text)
        .into_any_element()
}

fn pairing(
    pane: &SettingsView,
    state: &RemoteAccessState,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let listening = state.status == ListenerStatus::Listening;
    let Some(pairing) = &pane.snapshot.mobile_pairing else {
        return controls::button(
            pane.style(),
            "mobile-pair",
            &tr!("Show Pairing Code"),
            listening && !pane.snapshot.mobile_busy,
            cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::PairPhone)),
        )
        .into_any_element();
    };
    let link = pairing.link.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(code(pairing.code.clone()))
        .child(div().text_color(pane.theme.fg_muted).child(tr!(
            "Scan with the Muxy app. The code works once and expires in 5 minutes."
        )))
        .child(
            div()
                .flex()
                .gap(px(8.0))
                .child(controls::button(
                    pane.style(),
                    "mobile-copy-link",
                    &tr!("Copy Link"),
                    true,
                    move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(link.clone())),
                ))
                .child(controls::button(
                    pane.style(),
                    "mobile-cancel-pairing",
                    &tr!("Cancel"),
                    true,
                    cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::CancelPairing)),
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
        .debug_selector(|| "settings-mobile-code".into())
        .child(painted)
        .into_any_element()
}

fn devices(
    pane: &SettingsView,
    state: &RemoteAccessState,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    if state.devices.is_empty() {
        return div()
            .text_color(pane.theme.fg_muted)
            .child(tr!("No paired devices yet."))
            .into_any_element();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .children(state.devices.iter().map(|device| {
            let id = device.id;
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .debug_selector(move || format!("settings-mobile-device-{id}"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w(px(0.0))
                        .child(device.name.clone())
                        .child(
                            div()
                                .text_color(pane.theme.fg_muted)
                                .child(presence(device, now)),
                        ),
                )
                .child(controls::button(
                    pane.style(),
                    &format!("mobile-revoke-{id}"),
                    &tr!("Revoke"),
                    !pane.snapshot.mobile_busy,
                    cx.listener(move |_, _, _, cx| cx.emit(SettingsEvent::RevokeDevice(id))),
                ))
        }))
        .into_any_element()
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
