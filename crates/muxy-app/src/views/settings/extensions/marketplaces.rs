use gpui::prelude::FluentBuilder;
use gpui::{Context, InteractiveElement, ParentElement, SharedString, Styled, Window, div};
use muxy_app_core::extensions::Marketplace;
use muxy_ui::l10n::tr_key;
use muxy_ui::{accessibility, controls, form, tr};

use super::{ExtensionsView, Mutation, Tab};

impl ExtensionsView {
    pub(super) fn marketplaces(&self, cx: &gpui::App) -> Vec<Marketplace> {
        self.model.upgrade().map_or_else(
            || vec![Marketplace::official()],
            |model| model.read(cx).extensions.registry.marketplaces(),
        )
    }

    pub(super) fn sync_marketplaces(&mut self, cx: &mut Context<Self>) {
        let current = self.marketplaces(cx);
        if current != self.sources && !self.mutating(cx) {
            self.sources = current;
            if self.tab == Tab::Marketplace && self.selected.is_none() {
                self.fetch(true, cx);
            }
        }
    }

    pub(super) fn add_marketplace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mutating(cx) {
            return;
        }
        let text = self.marketplace_input.read(cx).text().to_owned();
        let Ok(marketplace) = Marketplace::parse(&text) else {
            self.error = Some(
                tr!("Enter a GitHub repository like owner/repo, such as muxy-app/extensions.")
                    .to_string(),
            );
            cx.notify();
            return;
        };
        let window = window.window_handle();
        let task = self.model.update(cx, |model, cx| {
            model.add_extension_marketplace(marketplace, window, cx)
        });
        self.mutation = Some(Mutation::AddMarketplace);
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = match task {
                Ok(task) => task.await,
                Err(error) => Err(error.to_string()),
            };
            let _ = view.update(cx, |view, cx| {
                view.mutation = None;
                match result {
                    Ok(true) => {
                        view.marketplace_input
                            .update(cx, |input, cx| input.set_text("", cx));
                        view.sync_marketplaces(cx);
                    }
                    Ok(false) => view.sync_marketplaces(cx),
                    Err(error) => {
                        view.error = Some(error);
                        view.sync_marketplaces(cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn remove_marketplace(&mut self, marketplace: Marketplace, cx: &mut Context<Self>) {
        if self.mutating(cx) {
            return;
        }
        let task = self.model.update(cx, |model, cx| {
            model.remove_extension_marketplace(marketplace, cx)
        });
        self.mutation = Some(Mutation::RemoveMarketplace);
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = match task {
                Ok(task) => task.await,
                Err(error) => Err(error.to_string()),
            };
            let _ = view.update(cx, |view, cx| {
                view.mutation = None;
                view.error = result.err();
                view.sync_marketplaces(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn marketplaces_section(&self, cx: &Context<Self>) -> gpui::Div {
        let mut rows = div().flex().flex_col().gap(self.metrics.spacing4());
        for (index, marketplace) in self.marketplaces(cx).into_iter().enumerate() {
            let unavailable = self.unavailable.contains(&marketplace);
            let name = marketplace.to_string();
            let mut row = div()
                .id(SharedString::from(format!("extension-marketplace-{index}")))
                .flex()
                .items_center()
                .gap(self.metrics.spacing5())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(accessibility::label(
                            SharedString::from(format!("extension-marketplace-name-{index}")),
                            name.clone(),
                        )),
                );
            if marketplace.is_official() {
                row = row.child(self.badge(tr!("Official"), true));
            } else {
                if unavailable {
                    row = row.child(self.badge(tr!("Unavailable"), false));
                }
                let removed = marketplace.clone();
                row = row.child(self.button(
                    format!("extension-marketplace-remove-{index}"),
                    tr_key!("Remove"),
                    move |view, _, cx| view.remove_marketplace(removed.clone(), cx),
                    cx,
                ));
            }
            rows = rows.child(row);
        }
        let add =
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(self.metrics.spacing4())
                .child(div().flex_1().min_w(self.metrics.scaled(180.0)).child(
                    controls::text_field(
                        self.style(),
                        "extension-marketplace-input",
                        &self.marketplace_input,
                        None,
                    ),
                ))
                .child(self.button(
                    "extension-marketplace-add",
                    tr_key!("Add Marketplace"),
                    Self::add_marketplace,
                    cx,
                ));
        let content = div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing6())
            .child(div().text_color(self.theme.fg_muted).child(tr!(
                "Browse lists extensions from these GitHub repositories. Muxy reviews only the official marketplace. Extensions from others can ask for any permission they list, so add only repositories you trust."
            )))
            .when(!self.unavailable.is_empty(), |content| {
                let names: Vec<String> = self.unavailable.iter().map(ToString::to_string).collect();
                content.child(form::note(
                    self.style(),
                    &tr!("Couldn't load %@. Showing the other marketplaces.", names.join(", ")),
                    false,
                ))
            })
            .child(rows)
            .child(add);
        self.section_with_actions(&tr!("Marketplaces"), div(), content)
    }
}
