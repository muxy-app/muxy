//! The form that pairs with another computer from the link its Settings →
//! Devices shows, and renames or regroups a paired server.

use gpui::{
    AnyElement, Context, Entity, Focusable, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Subscription, Window, div,
};
use muxy_app_core::ServerId;
use muxy_ui::controls::{self, Style};
use muxy_ui::form::{NextControl, PreviousControl};
use muxy_ui::l10n::tr_key;
use muxy_ui::text_input::{InputEvent, TextInput};
use muxy_ui::tr;

use super::overlays::{Mark, Overlay, OverlayView, Placement};
use super::remote_servers::{field, input, modal, title};
use crate::model::{AppModel, PairingForm};

pub(crate) struct PairForm {
    /// `None` pairs a new computer.
    device: Option<ServerId>,
    link: Entity<TextInput>,
    name: Entity<TextInput>,
    group: Entity<TextInput>,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl PairForm {
    fn submitted(&self, cx: &gpui::App) -> PairingForm {
        let text = |input: &Entity<TextInput>| input.read(cx).text().to_owned();
        PairingForm {
            link: text(&self.link),
            name: text(&self.name),
            group: text(&self.group),
        }
    }

    fn advance_focus(&self, backwards: bool, window: &mut Window, cx: &gpui::App) {
        let fields = [&self.link, &self.name, &self.group].map(|input| input.focus_handle(cx));
        muxy_ui::form::advance_focus(&fields, backwards, window);
    }
}

impl AppModel {
    /// Opens the pairing form, or with `device` the form that renames,
    /// regroups, or pairs again with a paired server.
    pub(crate) fn open_pair_form(&mut self, device: Option<ServerId>, cx: &mut Context<Self>) {
        let entry = device.and_then(|id| self.settings.server(id).cloned());
        let link = input(
            self,
            cx,
            tr_key!("Pairing Link"),
            "pair-link",
            if device.is_some() {
                tr_key!("Paste a new link to pair again")
            } else {
                "muxy://pair?…"
            },
            "",
        );
        let name = input(
            self,
            cx,
            tr_key!("Name"),
            "pair-name",
            tr_key!("Shown in the sidebar"),
            entry.as_ref().map_or("", |entry| &entry.name),
        );
        let group = input(
            self,
            cx,
            tr_key!("Group"),
            "pair-group",
            tr_key!("optional"),
            entry
                .as_ref()
                .and_then(|entry| entry.group.as_deref())
                .unwrap_or_default(),
        );
        let subscriptions = [&link, &name, &group]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |model, _, event, cx| match event {
                    InputEvent::Submitted => model.submit_pair_form(cx),
                    InputEvent::Cancelled => model.dismiss_overlay(cx),
                    InputEvent::Changed => {
                        if let Some(Overlay::PairForm(form)) = &mut model.overlay {
                            form.error = None;
                            cx.notify();
                        }
                    }
                })
            })
            .collect();
        self.focus_owner.request(&link.focus_handle(cx));
        self.overlay_subscription = None;
        self.open_overlay(
            Overlay::PairForm(Box::new(PairForm {
                device,
                link,
                name,
                group,
                error: None,
                _subscriptions: subscriptions,
            })),
            cx,
        );
        cx.notify();
    }

    fn submit_pair_form(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::PairForm(form)) = &mut self.overlay else {
            return;
        };
        let (device, submitted) = (form.device, form.submitted(cx));
        form.error = None;
        if let Some(device) = device
            && submitted.link.trim().is_empty()
        {
            let result = self.edit_paired_server(device, &submitted, cx);
            self.finish_pair_form(result, cx);
            return;
        }
        if let Err(error) = self.start_pairing(device, submitted, cx) {
            self.finish_pair_form(Err(error), cx);
        }
        cx.notify();
    }

    /// How pairing or saving went. With the form closed, a notice says so.
    pub(crate) fn finish_pair_form(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        let open = matches!(self.overlay, Some(Overlay::PairForm(_)));
        match result {
            Ok(()) if open => self.dismiss_overlay(cx),
            Ok(()) => self.show_notice(tr!("Paired with the other computer.").to_string(), cx),
            Err(error) => {
                if let Some(Overlay::PairForm(form)) = &mut self.overlay {
                    form.error = Some(error);
                } else {
                    self.show_notice(error, cx);
                }
            }
        }
        cx.notify();
    }
}

impl OverlayView for PairForm {
    fn render(
        &self,
        _: Option<Mark>,
        model: &AppModel,
        window: &Window,
        cx: &mut Context<AppModel>,
    ) -> AnyElement {
        render(self, model, window, cx)
    }

    fn placement(&self, _: &AppModel) -> Placement {
        Placement::Modal
    }
}

fn render(
    form: &PairForm,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    let text = |id: &str, input: &Entity<TextInput>| controls::text_field(style, id, input, None);
    let editing = form.device.is_some();
    let pairing = model.pairing_running();
    let ready = editing || !form.link.read(cx).text().trim().is_empty();
    let (status, color) = match &form.error {
        Some(error) => (error.clone().into(), theme.danger),
        None if pairing => (
            tr!("Pairing… It is saved when it finishes, even if you close this form."),
            theme.fg_muted,
        ),
        None if editing => (
            tr!(
                "If it was revoked, moved, or reset, copy a new pairing link from its Settings → Devices to pair again."
            ),
            theme.fg_muted,
        ),
        None => (
            tr!(
                "On the other computer, open Settings → Devices, turn on Allow other devices, and copy its pairing link. Both computers must reach each other on the network or through a VPN."
            ),
            theme.fg_muted,
        ),
    };
    let heading = if editing {
        tr!("Edit Paired Server")
    } else {
        tr!("Pair with Another Computer")
    };
    modal(model, window, "pair-form", heading.clone())
        .key_context("Form")
        .on_action(cx.listener(|model, _: &NextControl, window, cx| {
            if let Some(Overlay::PairForm(form)) = &model.overlay {
                form.advance_focus(false, window, cx);
            }
        }))
        .on_action(cx.listener(|model, _: &PreviousControl, window, cx| {
            if let Some(Overlay::PairForm(form)) = &model.overlay {
                form.advance_focus(true, window, cx);
            }
        }))
        .child(title(&heading, model))
        .child(field(
            &tr!("Pairing Link"),
            text("pair-link", &form.link),
            model,
        ))
        .child(field(&tr!("Name"), text("pair-name", &form.name), model))
        .child(field(&tr!("Group"), text("pair-group", &form.group), model))
        .child(
            muxy_ui::accessibility::announced(
                div()
                    .id("pair-form-status")
                    .accessibility_id("pair-form-status")
                    .debug_selector(|| "pair-form-status".into())
                    .text_size(m.font_footnote())
                    .text_color(color),
                status.clone(),
                form.error.is_some(),
            )
            .child(status),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.spacing3())
                .pt(m.spacing3())
                .child(div().flex_1())
                .child(controls::button(
                    style,
                    "pair-cancel",
                    controls::Button::secondary(&tr!("Cancel")),
                    cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
                ))
                .child(
                    controls::button(
                        style,
                        "pair-submit",
                        controls::Button::primary(
                            &(if editing { tr!("Save") } else { tr!("Pair") }),
                        )
                        .enabled(ready && !pairing),
                        cx.listener(|model, _, _, cx| model.submit_pair_form(cx)),
                    )
                    .debug_selector(|| "pair-submit".into()),
                ),
        )
        .into_any_element()
}
