//! The Installed and Browse lists, as cards in a grid or as rows, and the
//! controls that search, filter, sort, and lay them out.

use gpui::prelude::FluentBuilder;
use gpui::{
    Context, FontWeight, InteractiveElement, ParentElement, Pixels, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use muxy_app_core::extensions::Extension;
use muxy_app_core::settings::ExtensionsLayout;
use muxy_ui::{
    accessibility,
    components::{ButtonInteraction, SymbolGlyph},
    controls::{self, Choice},
};

use super::pictures::Art;
use super::{ExtensionsView, Loading, Tab};
use crate::extensions::{
    authoring::AgentTask,
    marketplace::{About, ExtensionDetails, Sort},
    pictures::Size,
};
use crate::model::AppModel;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

/// Browse's category filters: the marketplace category and its English label.
pub(super) const CATEGORIES: [(Option<&str>, &str); 6] = [
    (None, tr_key!("All")),
    (Some("productivity"), tr_key!("Productivity")),
    (Some("developer-tools"), tr_key!("Developer tools")),
    (Some("git"), tr_key!("Git")),
    (Some("ai"), tr_key!("AI")),
    (Some("localization"), tr_key!("Languages")),
];

/// The narrowest a grid card gets before the grid drops a column.
const CARD_WIDTH: f32 = 220.0;
const MAX_COLUMNS: u16 = 4;

/// What opening an entry shows.
#[derive(Clone)]
enum Open {
    /// An installed extension, shown at once from what is on disk.
    Installed(Box<ExtensionDetails>),
    /// A marketplace listing, fetched first.
    Marketplace(String),
}

/// One extension in a list, whichever tab it is on.
pub(super) struct Entry {
    pub(super) name: String,
    version: Option<String>,
    description: String,
    about: About,
    art: Art,
    /// Short labels beside the name; the accented ones call for attention.
    pub(super) badges: Vec<(SharedString, bool)>,
    open: Open,
    /// The installed copy and whether it is an unpacked folder, for rows
    /// that offer the agent.
    installed: Option<(Extension, bool)>,
}

/// How many grid columns fit `width`. Before the first layout there is no
/// width yet, so two are assumed.
pub(super) fn columns(width: Pixels, card: Pixels, gap: Pixels) -> u16 {
    if width <= px(0.0) {
        return 2;
    }
    let fit = ((width + gap) / (card + gap)).floor();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Clamped to 1..=MAX_COLUMNS before the cast"
    )]
    let fit = fit.clamp(1.0, f32::from(MAX_COLUMNS)) as u16;
    fit
}

/// A download count as people say it: 457, 2.5K, 12K, 1.2M.
pub(super) fn compact(count: u64) -> String {
    #[allow(clippy::cast_precision_loss, reason = "One decimal place is shown")]
    let scaled = |unit: u64, suffix: &str| {
        let tenths = (count as f64 / unit as f64 * 10.0).round() / 10.0;
        if tenths < 10.0 && tenths.fract() != 0.0 {
            format!("{tenths:.1}{suffix}")
        } else {
            format!("{:.0}{suffix}", tenths.round())
        }
    };
    match count {
        0..1_000 => count.to_string(),
        1_000..999_500 => scaled(1_000, "K"),
        _ => scaled(1_000_000, "M"),
    }
}

/// A count with thousands separated: 5,564.
pub(super) fn grouped(count: u64) -> String {
    let digits = count.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

impl ExtensionsView {
    /// The layout the current tab is shown in.
    pub(super) fn layout(&self, cx: &gpui::App) -> ExtensionsLayout {
        self.model
            .upgrade()
            .map_or(ExtensionsLayout::List, |model| {
                let appearance = &model.read(cx).appearance;
                if self.tab == Tab::Installed {
                    appearance.extensions_installed_layout
                } else {
                    appearance.extensions_browse_layout
                }
            })
    }

    /// Shows the current tab in `layout`, remembered for next time. Saving
    /// refreshes the Settings tab, which updates this page, so it waits
    /// until this update ends.
    pub(super) fn set_layout(&mut self, layout: ExtensionsLayout, cx: &mut Context<Self>) {
        let tab = self.tab;
        let changed = self.model.update(cx, |model, _| {
            let current = if tab == Tab::Installed {
                &mut model.appearance.extensions_installed_layout
            } else {
                &mut model.appearance.extensions_browse_layout
            };
            let changed = *current != layout;
            *current = layout;
            changed
        });
        if changed.unwrap_or(false) {
            let model = self.model.clone();
            cx.defer(move |cx| {
                let _ = model.update(cx, AppModel::save_appearance);
            });
        }
        cx.notify();
    }

    pub(super) fn set_category(&mut self, category: Option<&'static str>, cx: &mut Context<Self>) {
        if self.category != category {
            self.category = category;
            self.fetch(true, cx);
        }
    }

    pub(super) fn set_sort(&mut self, sort: Sort, cx: &mut Context<Self>) {
        if self.sort != sort {
            self.sort = sort;
            self.fetch(true, cx);
        }
    }

    /// The search field, with Browse's order and both tabs' layout beside it,
    /// and Browse's categories below.
    pub(super) fn toolbar(&self, cx: &Context<Self>) -> gpui::Div {
        let browsing = self.tab == Tab::Marketplace;
        div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing5())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(self.metrics.spacing4())
                    .child(div().flex_1().min_w(self.metrics.scaled(180.0)).child(
                        controls::search_field(self.style(), "extension-search", &self.search),
                    ))
                    .when(browsing, |row| row.child(self.sort_control(cx)))
                    .child(self.layout_toggle(cx)),
            )
            .when(browsing, |toolbar| toolbar.child(self.category_chips(cx)))
    }

    fn sort_control(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let enabled = !self.mutating(cx);
        let choices = [
            Choice::new("popular", tr!("Popular")),
            Choice::new("recent", tr!("Recent")),
            Choice::new("name", tr!("A–Z")),
        ]
        .map(|mut choice| {
            choice.enabled = enabled;
            choice
        });
        controls::segmented(
            self.style(),
            "extension-sort",
            &tr!("Sort extensions"),
            &choices,
            match self.sort {
                Sort::Popular => "popular",
                Sort::Recent => "recent",
                Sort::Name => "name",
            },
            cx.listener(|view, value: &SharedString, _, cx| {
                let sort = match value.as_ref() {
                    "recent" => Sort::Recent,
                    "name" => Sort::Name,
                    _ => Sort::Popular,
                };
                view.set_sort(sort, cx);
            }),
        )
    }

    /// Grid and list, as two icon buttons joined like a segmented control.
    fn layout_toggle(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let current = self.layout(cx);
        let option = |layout: ExtensionsLayout,
                      id: &'static str,
                      (symbol, fallback): (&'static str, &'static str),
                      name| {
            let selected = layout == current;
            div()
                .id(id)
                .role(gpui::Role::RadioButton)
                .accessibility_id(id)
                .debug_selector(move || id.into())
                .aria_label(name)
                .aria_toggled(if selected {
                    gpui::Toggled::True
                } else {
                    gpui::Toggled::False
                })
                .flex()
                .items_center()
                .justify_center()
                .size(self.metrics.control_medium() - px(4.0))
                .rounded(self.metrics.radius_sm())
                .cursor_pointer()
                .when(selected, |button| button.bg(self.theme.accent_soft))
                .when(!selected, |button| {
                    button.hover(|button| button.bg(self.theme.hover))
                })
                .focus(|button| button.bg(self.theme.hover))
                .button_interaction(cx.listener(move |view, _, _, cx| {
                    view.set_layout(layout, cx);
                }))
                .child(
                    SymbolGlyph::new(
                        symbol,
                        self.metrics.icon_md(),
                        if selected {
                            self.theme.accent
                        } else {
                            self.theme.fg_muted
                        },
                    )
                    .or(fallback),
                )
        };
        div()
            .id("extension-layout")
            .role(gpui::Role::RadioGroup)
            .aria_label(tr!("Layout"))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(1.0))
            .p(px(1.0))
            .rounded(self.metrics.radius_sm())
            .bg(self.theme.surface)
            .border_1()
            .border_color(self.theme.border)
            .child(option(
                ExtensionsLayout::Grid,
                "extension-layout-grid",
                ("square.grid.2x2", "square.grid.2x2"),
                tr!("Grid"),
            ))
            .child(option(
                ExtensionsLayout::List,
                "extension-layout-list",
                ("list.bullet", "square.split.1x2"),
                tr!("List"),
            ))
    }

    fn category_chips(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let enabled = !self.mutating(cx);
        let mut chips = div()
            .id("extension-categories")
            .role(gpui::Role::RadioGroup)
            .aria_label(tr!("Categories"))
            .flex()
            .flex_wrap()
            .gap(self.metrics.spacing3());
        for (category, label) in CATEGORIES {
            let selected = self.category == category;
            let id =
                SharedString::from(format!("extension-category-{}", category.unwrap_or("all")));
            let selector = id.clone();
            chips = chips.child(
                div()
                    .id(id.clone())
                    .role(gpui::Role::RadioButton)
                    .accessibility_id(id)
                    .debug_selector(move || selector.to_string())
                    .aria_label(muxy_ui::l10n::translate(label))
                    .aria_toggled(if selected {
                        gpui::Toggled::True
                    } else {
                        gpui::Toggled::False
                    })
                    .flex()
                    .items_center()
                    .h(self.metrics.control_small())
                    .px(self.metrics.spacing5())
                    .rounded_full()
                    .border_1()
                    .text_size(self.metrics.font_footnote())
                    .map(|chip| {
                        if selected {
                            chip.bg(self.theme.accent_soft)
                                .border_color(self.theme.accent)
                                .text_color(self.theme.accent)
                                .font_weight(FontWeight::MEDIUM)
                        } else {
                            chip.bg(self.theme.surface)
                                .border_color(self.theme.border)
                                .text_color(self.theme.fg_muted)
                        }
                    })
                    .when(!enabled, |chip| chip.opacity(0.4))
                    .when(enabled, |chip| {
                        chip.cursor_pointer()
                            .hover(|chip| chip.text_color(self.theme.fg))
                            .focus(|chip| chip.border_color(self.theme.accent))
                            .button_interaction(cx.listener(move |view, _, _, cx| {
                                if !view.mutating(cx) {
                                    view.set_category(category, cx);
                                }
                            }))
                    })
                    .child(muxy_ui::l10n::translate(label)),
            );
        }
        chips
    }

    pub(super) fn installed_entries(
        &self,
        installed: &[(Extension, bool, bool)],
        cx: &Context<Self>,
    ) -> Vec<Entry> {
        let query = self.query.to_lowercase();
        let model = self.model.upgrade();
        installed
            .iter()
            .filter(|(extension, _, _)| {
                format!("{} {}", extension.name, extension.manifest.description)
                    .to_lowercase()
                    .contains(&query)
            })
            .map(|(extension, enabled, unpacked)| {
                let details = ExtensionDetails::from(extension);
                let mut badges = Vec::new();
                if *unpacked {
                    badges.push((tr!("Unpacked"), false));
                } else if model
                    .as_ref()
                    .and_then(|model| model.read(cx).extension_update_version(&extension.name))
                    .is_some()
                {
                    badges.push((tr!("Update available"), true));
                }
                if !enabled {
                    badges.push((tr!("Disabled"), false));
                }
                Entry {
                    name: extension.name.clone(),
                    version: Some(extension.version.clone()),
                    description: extension.manifest.description.clone(),
                    about: details.about.clone(),
                    art: self.pictures.installed(extension),
                    badges,
                    open: Open::Installed(Box::new(details)),
                    installed: Some((extension.clone(), *unpacked)),
                }
            })
            .collect()
    }

    pub(super) fn browse_entries(&self, cx: &Context<Self>) -> Vec<Entry> {
        let model = self.model.upgrade();
        self.items
            .iter()
            .map(|item| {
                let mut badges = Vec::new();
                if item.about.official {
                    badges.push((tr!("Official"), true));
                }
                if let Some(model) = &model {
                    let model = model.read(cx);
                    if model
                        .extensions
                        .registry
                        .extensions
                        .contains_key(&item.name)
                    {
                        badges.push(if model.extension_update_version(&item.name).is_some() {
                            (tr!("Update available"), true)
                        } else {
                            (tr!("Installed"), false)
                        });
                    }
                }
                Entry {
                    name: item.name.clone(),
                    version: item.version.clone(),
                    description: item.description.clone(),
                    about: item.about.clone(),
                    art: Art::marketplace(&item.name, 1),
                    badges,
                    open: Open::Marketplace(item.name.clone()),
                    installed: None,
                }
            })
            .collect()
    }

    pub(super) fn installed_body(
        &self,
        installed: &[(Extension, bool, bool)],
        cx: &Context<Self>,
    ) -> gpui::Div {
        let entries = self.installed_entries(installed, cx);
        if entries.is_empty() {
            return if installed.is_empty() {
                self.empty_state(
                    &tr!("No extensions installed"),
                    &tr!("Browse extensions to add tools to Muxy, or load an unpacked extension folder."),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(self.metrics.spacing4())
                        .child(self.button(
                            "extension-browse",
                            tr_key!("Browse extensions"),
                            |view, _, cx| view.switch_tab(Tab::Marketplace, cx),
                            cx,
                        ))
                        .child(self.button(
                            "extension-create-empty",
                            tr_key!("Create Extension…"),
                            |view, _, cx| view.agent_task(AgentTask::Create, cx),
                            cx,
                        )),
                )
            } else {
                self.empty_state(
                    &tr!("No matching extensions"),
                    &tr!("Try a different name or clear your search."),
                )
            };
        }
        self.entries(entries, cx)
    }

    pub(super) fn marketplace_body(&self, cx: &Context<Self>) -> gpui::Div {
        let mut body = div().flex().flex_col().gap(self.metrics.spacing7());
        if self.loading == Some(Loading::Search) {
            body = body.child(
                accessibility::announced(
                    div()
                        .text_color(self.theme.fg_muted)
                        .id("extension-loading"),
                    tr!("Loading extensions…"),
                    false,
                )
                .child(tr!("Loading extensions…")),
            );
        }
        let entries = self.browse_entries(cx);
        if !entries.is_empty() {
            body = body.child(self.entries(entries, cx));
        } else if self.loading.is_none() && self.error.is_none() {
            body = body.child(self.empty_state(
                &tr!("No extensions found"),
                &if self.category.is_some() {
                    tr!("Try another category or a different search.")
                } else {
                    tr!("Try a different search to find extensions.")
                },
            ));
        }
        if self.has_next {
            body = body.child(div().flex().justify_center().child(self.button(
                "extension-more",
                tr_key!("Load more"),
                |view, _, cx| {
                    view.fetch(false, cx);
                },
                cx,
            )));
        }
        body
    }

    fn entries(&self, entries: Vec<Entry>, cx: &Context<Self>) -> gpui::Div {
        match self.layout(cx) {
            ExtensionsLayout::Grid => {
                let gap = self.metrics.spacing6();
                let width = self.width.get();
                let columns = columns(width, self.metrics.scaled(CARD_WIDTH), gap);
                let card = if width > px(0.0) {
                    (width - gap * f32::from(columns - 1)) / f32::from(columns)
                } else {
                    self.metrics.scaled(CARD_WIDTH)
                };
                let mut grid = div()
                    .id("extension-grid")
                    .debug_selector(move || format!("extension-grid-{columns}"))
                    .grid()
                    .grid_cols(columns)
                    .gap(gap)
                    .w_full();
                for entry in entries {
                    grid = grid.child(self.card_entry(&entry, card * 0.625, cx));
                }
                div().child(grid)
            }
            ExtensionsLayout::List => {
                let mut rows = self
                    .card()
                    .overflow_hidden()
                    .debug_selector(|| "extension-list".into());
                for (index, entry) in entries.into_iter().enumerate() {
                    rows = rows.child(self.row_entry(&entry, index > 0, cx));
                }
                rows
            }
        }
    }

    /// The clickable shell every entry shares, which opens it.
    fn entry_shell(&self, entry: &Entry, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let id = match &entry.open {
            Open::Installed(_) => format!("extension-row-{}", entry.name),
            Open::Marketplace(_) => format!("extension-card-{}", entry.name),
        };
        let open = entry.open.clone();
        let loading = matches!(&self.loading, Some(Loading::Details(name)) if *name == entry.name);
        let description = entry.description.clone();
        div()
            .id(SharedString::from(id.clone()))
            .role(gpui::Role::Button)
            .accessibility_id(id.clone())
            .debug_selector(move || id.clone())
            .aria_label(entry.name.clone())
            .aria_value(
                entry
                    .badges
                    .iter()
                    .map(|(badge, _)| badge.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .aria_description(description)
            .cursor_pointer()
            .when(loading, |shell| shell.opacity(0.6))
            .button_interaction(cx.listener(move |view, _, _, cx| {
                if view.busy(cx) {
                    return;
                }
                view.completed = None;
                view.install_status = None;
                match &open {
                    Open::Installed(details) => view.open_details(details.as_ref().clone(), cx),
                    Open::Marketplace(name) => view.details(name.clone(), cx),
                }
                cx.notify();
            }))
    }

    /// Author and downloads, as one muted line.
    pub(super) fn byline(about: &About) -> Option<String> {
        let parts: Vec<String> = about
            .author
            .iter()
            .map(|author| tr!("by %@", author).to_string())
            .chain(
                about
                    .downloads
                    .map(|downloads| tr!("%@ installs", compact(downloads)).to_string()),
            )
            .collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    fn badges(&self, badges: &[(SharedString, bool)]) -> impl Iterator<Item = gpui::Div> {
        badges
            .iter()
            .map(|(text, accent)| self.badge(text.clone(), *accent))
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn card_entry(
        &self,
        entry: &Entry,
        shot: Pixels,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let radius = self.metrics.radius_lg();
        let byline = Self::byline(&entry.about);
        let screenshot = self.screenshot_fill(
            entry.art.screenshots.first(),
            Size::Card,
            |image| image.rounded_t(radius),
            cx,
        );
        self.entry_shell(entry, cx)
            .flex()
            .flex_col()
            .min_w_0()
            .rounded(radius)
            .bg(self.theme.surface)
            .border_1()
            .border_color(self.theme.border)
            .hover(|card| card.border_color(self.theme.accent))
            .focus(|card| card.border_color(self.theme.accent))
            .child(
                div()
                    .flex_none()
                    .w_full()
                    .h(shot)
                    .rounded_t(radius)
                    .bg(self.theme.hover)
                    .border_b_1()
                    .border_color(self.theme.border)
                    .child(screenshot),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(self.metrics.spacing4())
                    .p(self.metrics.spacing6())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(self.metrics.spacing5())
                            .min_w_0()
                            .child(self.icon_tile(
                                entry.art.icon.as_ref(),
                                self.metrics.scaled(32.0),
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .truncate()
                                            .child(entry.name.clone()),
                                    )
                                    .when_some(byline, |names, byline| {
                                        names.child(
                                            div()
                                                .text_size(self.metrics.font_footnote())
                                                .text_color(self.theme.fg_muted)
                                                .truncate()
                                                .child(byline),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(self.metrics.font_footnote())
                            .text_color(self.theme.fg_muted)
                            .line_clamp(2)
                            .child(entry.description.clone()),
                    )
                    .when(!entry.badges.is_empty(), |body| {
                        body.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(self.metrics.spacing3())
                                .children(self.badges(&entry.badges)),
                        )
                    }),
            )
    }

    fn row_entry(
        &self,
        entry: &Entry,
        separator: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let byline = Self::byline(&entry.about);
        let agent = entry.installed.as_ref().and_then(|(extension, unpacked)| {
            self.agent_button("extension-row-agent", extension, *unpacked, cx)
        });
        self.entry_shell(entry, cx)
            .flex()
            .items_center()
            .gap(self.metrics.spacing6())
            .px(self.metrics.spacing7())
            .py(self.metrics.spacing6())
            .when(separator, |row| {
                row.border_t_1().border_color(self.theme.border)
            })
            .hover(|row| row.bg(self.theme.hover))
            .focus(|row| row.bg(self.theme.accent_soft))
            .child(self.icon_tile(entry.art.icon.as_ref(), self.metrics.scaled(40.0), cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(self.metrics.spacing2())
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(self.metrics.spacing4())
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .min_w_0()
                                    .max_w_full()
                                    .truncate()
                                    .child(entry.name.clone()),
                            )
                            .when_some(entry.version.clone(), |title, version| {
                                title.child(
                                    div()
                                        .text_size(self.metrics.font_footnote())
                                        .text_color(self.theme.fg_muted)
                                        .child(format!("v{version}")),
                                )
                            })
                            .children(self.badges(&entry.badges)),
                    )
                    .when_some(byline, |text, byline| {
                        text.child(
                            div()
                                .text_size(self.metrics.font_footnote())
                                .text_color(self.theme.fg_muted)
                                .truncate()
                                .child(byline),
                        )
                    })
                    .child(
                        div()
                            .text_size(self.metrics.font_footnote())
                            .text_color(self.theme.fg_muted)
                            .line_clamp(2)
                            .child(entry.description.clone()),
                    ),
            )
            .children(agent.map(|button| {
                button
                    .flex_none()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
            }))
            .child(SymbolGlyph::new(
                "chevron.right",
                self.metrics.icon_sm(),
                self.theme.fg_muted,
            ))
    }
}
