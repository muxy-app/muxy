//! One extension's page: a fixed header with its name, main actions, and
//! links to each section, then its screenshots, facts, and settings.

use std::borrow::Cow;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels,
    SharedString, StatefulInteractiveElement, Styled, div, px,
};
use muxy_app_core::extensions::Extension;
use muxy_ui::{
    accessibility,
    components::{ButtonInteraction, SymbolGlyph},
};

use super::pictures::Art;
use super::{ExtensionsView, Mutation, permissions_note};
use crate::extensions::{
    marketplace::{self, ExtensionDetails},
    pictures::Size,
};
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::tr;

/// The parts of an extension's page the header links to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Section {
    Overview,
    Updates,
    Permissions,
    Settings,
    Logs,
    Manage,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => tr_key!("Overview"),
            Self::Updates => tr_key!("Updates"),
            Self::Permissions => tr_key!("Permissions"),
            Self::Settings => tr_key!("Settings"),
            Self::Logs => tr_key!("Logs"),
            Self::Manage => tr_key!("Manage"),
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Updates => "updates",
            Self::Permissions => "permissions",
            Self::Settings => "settings",
            Self::Logs => "logs",
            Self::Manage => "manage",
        }
    }
}

/// The page is laid out in two columns once it is at least this wide.
const TWO_COLUMNS: f32 = 640.0;
const INFO_WIDTH: f32 = 220.0;

/// What the page shows for `details`: the installed copy's own description
/// when there is one, with the marketplace's counts when they're known.
fn shown<'a>(
    details: &'a ExtensionDetails,
    local: Option<&(Extension, bool, bool)>,
) -> Cow<'a, ExtensionDetails> {
    let Some((extension, _, _)) = local else {
        return Cow::Borrowed(details);
    };
    let mut shown = ExtensionDetails::from(extension);
    shown.about.downloads = details.about.downloads;
    shown.about.official = details.about.official;
    Cow::Owned(shown)
}

impl ExtensionsView {
    fn local<'a>(
        details: &ExtensionDetails,
        installed: &'a [(Extension, bool, bool)],
    ) -> Option<&'a (Extension, bool, bool)> {
        installed
            .iter()
            .find(|(extension, _, _)| extension.name == details.name)
    }

    fn art(&self, details: &ExtensionDetails, local: Option<&(Extension, bool, bool)>) -> Art {
        local.map_or_else(
            || Art::marketplace(&details.name, details.screenshots),
            |(extension, _, _)| self.pictures.installed(extension),
        )
    }

    /// The section the page is scrolled to: the one a header link just
    /// jumped to, or else the last one whose top has reached the top of the
    /// page, or the last of all at the very end.
    pub(super) fn current_section(&self) -> Option<Section> {
        if let Some(section) = self
            .jumped
            .filter(|jumped| self.sections.iter().any(|(section, _)| section == jumped))
        {
            return Some(section);
        }
        let scroll = &self.detail_scroll;
        let at_end = scroll.max_offset().height > px(0.0)
            && -scroll.offset().y >= scroll.max_offset().height - px(1.0);
        if at_end {
            return self.sections.last().map(|(section, _)| *section);
        }
        let top = scroll.top_item();
        self.sections
            .iter()
            .take_while(|(_, index)| *index <= top)
            .last()
            .or(self.sections.first())
            .map(|(section, _)| *section)
    }

    /// The fixed header of an extension's page.
    pub(super) fn detail_bar(
        &self,
        details: &ExtensionDetails,
        installed: &[(Extension, bool, bool)],
        cx: &Context<Self>,
    ) -> gpui::Div {
        let local = Self::local(details, installed);
        let shown = shown(details, local);
        let art = self.art(details, local);
        div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing6())
            .child(
                div().flex().child(
                    self.button(
                        "extension-back",
                        tr_key!("Extensions"),
                        |view, _, cx| {
                            view.selected = None;
                            view.error = None;
                            cx.notify();
                        },
                        cx,
                    )
                    .aria_label(tr!("Back to Extensions"))
                    .flex_row_reverse()
                    .gap(self.metrics.spacing3())
                    .child(SymbolGlyph::new(
                        "chevron.left",
                        self.metrics.icon_sm(),
                        self.theme.fg_muted,
                    )),
                ),
            )
            .child(self.detail_hero(&details.name, &shown, &art, local, cx))
            .when(self.sections.len() > 1, |bar| {
                bar.child(self.section_links(cx))
            })
    }

    #[allow(clippy::too_many_lines, reason = "One header's declarative layout")]
    fn detail_hero(
        &self,
        name: &str,
        details: &ExtensionDetails,
        art: &Art,
        local: Option<&(Extension, bool, bool)>,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let version = details
            .version
            .as_ref()
            .or(details.current_version.as_ref());
        let mut badges: Vec<(SharedString, bool)> = Vec::new();
        if details.about.official {
            badges.push((tr!("Official"), true));
        }
        if let Some((_, enabled, unpacked)) = local {
            badges.push(if *enabled {
                (tr!("Enabled"), true)
            } else {
                (tr!("Disabled"), false)
            });
            if *unpacked {
                badges.push((tr!("Unpacked"), false));
            }
        }
        let action = if let Some((_, enabled, _)) = local {
            let enabled = *enabled;
            let owner = name.to_owned();
            self.styled_button(
                "extension-enable",
                if enabled {
                    tr_key!("Disable")
                } else {
                    tr_key!("Enable")
                },
                !enabled,
                move |view, _, cx| view.enable(&owner, !enabled, cx),
                cx,
            )
        } else {
            self.styled_button(
                "extension-install",
                tr_key!("Install"),
                true,
                |view, _, cx| view.install(cx),
                cx,
            )
        };
        let byline = Self::byline(&details.about);
        div()
            .flex()
            .items_start()
            .gap(self.metrics.spacing7())
            .child(self.icon_tile(art.icon.as_ref(), self.metrics.scaled(56.0), cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(self.metrics.spacing2())
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(self.metrics.spacing4())
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(self.metrics.font_display())
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .min_w_0()
                                    .truncate()
                                    .child(accessibility::label(
                                        "extension-detail-name",
                                        name.to_owned(),
                                    )),
                            )
                            .when_some(version, |heading, version| {
                                heading.child(
                                    div().flex_none().text_color(self.theme.fg_muted).child(
                                        accessibility::label(
                                            "extension-detail-version",
                                            format!("v{version}"),
                                        ),
                                    ),
                                )
                            }),
                    )
                    .when_some(byline, |column, byline| {
                        column.child(
                            div()
                                .text_size(self.metrics.font_footnote())
                                .text_color(self.theme.fg_muted)
                                .child(byline),
                        )
                    })
                    .child(div().text_color(self.theme.fg_muted).line_clamp(2).child(
                        accessibility::label(
                            "extension-detail-description",
                            details.description.clone(),
                        ),
                    ))
                    .when(!badges.is_empty(), |column| {
                        column.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(self.metrics.spacing3())
                                .pt(self.metrics.spacing2())
                                .children(
                                    badges
                                        .into_iter()
                                        .map(|(text, accent)| self.badge(text, accent)),
                                ),
                        )
                    }),
            )
            .child(action)
    }

    /// Links that scroll the page to each of its sections.
    fn section_links(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let current = self.current_section();
        let mut links = div()
            .id("extension-sections")
            .role(gpui::Role::TabList)
            .aria_label(tr!("Sections"))
            .flex()
            .flex_wrap()
            .gap(self.metrics.spacing2());
        for (section, index) in self.sections.iter().copied() {
            let selected = current == Some(section);
            let id = format!("extension-section-link-{}", section.id());
            let selector = id.clone();
            links = links.child(
                div()
                    .id(SharedString::from(id.clone()))
                    .role(gpui::Role::Tab)
                    .accessibility_id(id)
                    .debug_selector(move || selector.clone())
                    .aria_label(translate(section.label()))
                    .aria_selected(selected)
                    .flex()
                    .items_center()
                    .h(self.metrics.control_medium())
                    .px(self.metrics.spacing5())
                    .rounded(self.metrics.radius_sm())
                    .text_size(self.metrics.font_footnote())
                    .cursor_pointer()
                    .map(|link| {
                        if selected {
                            link.bg(self.theme.accent_soft)
                                .text_color(self.theme.accent)
                                .font_weight(FontWeight::MEDIUM)
                        } else {
                            link.text_color(self.theme.fg_muted)
                                .hover(|link| link.bg(self.theme.hover).text_color(self.theme.fg))
                        }
                    })
                    .focus(|link| link.bg(self.theme.hover))
                    .button_interaction(cx.listener(move |view, _, _, cx| {
                        view.detail_scroll.scroll_to_top_of_item(index);
                        view.jumped = Some(section);
                        cx.notify();
                    }))
                    .child(translate(section.label())),
            );
        }
        links
    }

    /// The scrolled parts of an extension's page, each with the section it
    /// starts, if any.
    pub(super) fn detail_children(
        &self,
        details: &ExtensionDetails,
        installed: &[(Extension, bool, bool)],
        cx: &Context<Self>,
    ) -> Vec<(Option<Section>, AnyElement)> {
        let name = details.name.clone();
        let local = Self::local(details, installed);
        let shown = shown(details, local);
        let art = self.art(details, local);
        let mut children = vec![(
            Some(Section::Overview),
            self.overview(&shown, &art, local, cx).into_any_element(),
        )];
        if let Some((extension, _, false)) = local {
            children.push((
                Some(Section::Updates),
                self.update_section(extension, details, cx)
                    .into_any_element(),
            ));
        }
        children.push((
            Some(Section::Permissions),
            self.section(&tr!("Requested permissions"), self.permissions_body(&shown))
                .into_any_element(),
        ));
        let Some((extension, _, unpacked)) = local else {
            return children;
        };
        if let Some(model) = self.model.upgrade() {
            let model = model.read(cx);
            let stored = model
                .extensions
                .settings
                .get(&extension.name)
                .cloned()
                .unwrap_or_default();
            let lines: Vec<String> = model
                .extensions
                .logs
                .tail(&extension.name)
                .map(str::to_owned)
                .collect();
            let background = extension
                .manifest
                .background
                .is_some()
                .then(|| model.background_running(&extension.name));
            children.push((
                None,
                self.rules_section(&extension.name, model, cx)
                    .into_any_element(),
            ));
            if let Some(settings) = self.settings_section(extension, &stored, cx) {
                children.push((Some(Section::Settings), settings.into_any_element()));
            }
            children.push((
                Some(Section::Logs),
                self.logs_section(extension, &lines, background, cx)
                    .into_any_element(),
            ));
        }
        children.push((
            Some(Section::Manage),
            self.manage_section(&name, extension, *unpacked, cx)
                .into_any_element(),
        ));
        children
    }

    fn manage_section(
        &self,
        name: &str,
        extension: &Extension,
        unpacked: bool,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let agent = self.agent_button("extension-agent", extension, unpacked, cx);
        let owner = name.to_owned();
        let removed = name.to_owned();
        let published = self
            .published
            .as_ref()
            .filter(|(extension, _)| extension == name)
            .map(|(_, url)| url.clone());
        self.section(
            &tr!("Manage extension"),
            div()
                .flex()
                .flex_wrap()
                .gap(self.metrics.spacing4())
                .children(agent)
                .when(unpacked, |actions| {
                    actions.child(self.button(
                        "extension-publish",
                        tr_key!("Publish…"),
                        move |view, _, cx| view.publish(&owner, cx),
                        cx,
                    ))
                })
                .when_some(published, |actions, url| {
                    actions.child(self.button(
                        "extension-open-pull-request",
                        tr_key!("Open Pull Request"),
                        move |view, _, _| {
                            if let Err(error) = crate::opener::open_url(&url, None) {
                                view.error = Some(error.to_string());
                            }
                        },
                        cx,
                    ))
                })
                .child(
                    self.button(
                        "extension-remove",
                        if unpacked {
                            tr_key!("Unload folder")
                        } else {
                            tr_key!("Uninstall")
                        },
                        move |view, _, cx| view.remove(&removed, cx),
                        cx,
                    )
                    .text_color(self.theme.danger),
                ),
        )
    }

    /// Screenshots beside the extension's facts, or above them when the page
    /// is narrow.
    fn overview(
        &self,
        details: &ExtensionDetails,
        art: &Art,
        local: Option<&(Extension, bool, bool)>,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let width = self.width.get();
        let wide = width >= self.metrics.scaled(TWO_COLUMNS);
        let gap = self.metrics.spacing7();
        let info_width = self.metrics.scaled(INFO_WIDTH);
        let gallery_width = match (width > px(0.0), wide) {
            (false, _) => self.metrics.scaled(480.0),
            (true, true) => width - info_width - gap,
            (true, false) => width,
        };
        let review = local.is_some_and(|(_, enabled, _)| !enabled);
        let info = self.info(details, cx);
        div()
            .flex()
            .flex_col()
            .gap(gap)
            .when(review, |overview| {
                overview.child(
                    div()
                        .text_color(self.theme.fg_muted)
                        .child(accessibility::label(
                            "extension-review-permissions",
                            tr!("Review the permissions below, then enable this extension to use it."),
                        )),
                )
            })
            .child(
                div()
                    .flex()
                    .when(wide, |row| row.flex_row().items_start())
                    .when(!wide, Styled::flex_col)
                    .gap(gap)
                    .when(!art.screenshots.is_empty(), |row| {
                        row.child(self.gallery(art, gallery_width, cx))
                    })
                    .child(if wide && !art.screenshots.is_empty() {
                        info.w(info_width).flex_none()
                    } else {
                        info.w_full()
                    }),
            )
    }

    /// The selected screenshot large, arrows to step through them, and a
    /// strip of all of them.
    #[allow(clippy::too_many_lines, reason = "One gallery's declarative layout")]
    fn gallery(&self, art: &Art, width: Pixels, cx: &Context<Self>) -> gpui::Div {
        let count = art.screenshots.len();
        let index = self.screenshot.min(count.saturating_sub(1));
        let radius = self.metrics.radius_lg();
        // Drawn over any picture, so never see-through: `surface` is only a
        // tint, so it is laid over the page background first.
        let solid = gpui::Hsla {
            a: 1.0,
            ..self.theme.raised()
        };
        let step = |id: &'static str, symbol: &'static str, name: SharedString, to: usize| {
            div()
                .id(id)
                .role(gpui::Role::Button)
                .accessibility_id(id)
                .debug_selector(move || id.into())
                .aria_label(name)
                .absolute()
                .top(width * 0.3125 - self.metrics.scaled(14.0))
                .flex()
                .items_center()
                .justify_center()
                .size(self.metrics.scaled(28.0))
                .rounded_full()
                .bg(solid)
                .border_1()
                .border_color(self.theme.border)
                .cursor_pointer()
                .hover(|button| button.bg(self.theme.hover))
                .focus(|button| button.border_color(self.theme.accent))
                .button_interaction(cx.listener(move |view, _, _, cx| {
                    view.screenshot = to;
                    cx.notify();
                }))
                .child(SymbolGlyph::new(
                    symbol,
                    self.metrics.icon_sm(),
                    self.theme.fg,
                ))
        };
        let main = div()
            .id("extension-screenshot")
            .relative()
            .w_full()
            .h(width * 0.625)
            .rounded_t(radius)
            .when(count < 2, |main| main.rounded_b(radius))
            .bg(self.theme.hover)
            .child(self.screenshot_fill(
                art.screenshots.get(index),
                Size::Full,
                |image| {
                    let image = image.rounded_t(radius);
                    if count < 2 {
                        image.rounded_b(radius)
                    } else {
                        image
                    }
                },
                cx,
            ))
            .when(count > 1, |main| {
                main.child(
                    step(
                        "extension-screenshot-previous",
                        "chevron.left",
                        tr!("Previous screenshot"),
                        (index + count - 1) % count,
                    )
                    .left(self.metrics.spacing5()),
                )
                .child(
                    step(
                        "extension-screenshot-next",
                        "chevron.right",
                        tr!("Next screenshot"),
                        (index + 1) % count,
                    )
                    .right(self.metrics.spacing5()),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(self.metrics.spacing5())
                        .right(self.metrics.spacing5())
                        .px(self.metrics.spacing4())
                        .py(self.metrics.spacing1())
                        .rounded(self.metrics.radius_sm())
                        .bg(solid)
                        .text_size(self.metrics.font_footnote())
                        .text_color(self.theme.fg)
                        .child(accessibility::label(
                            "extension-screenshot-position",
                            tr!("%@ of %@", (index + 1).to_string(), count.to_string()),
                        )),
                )
            });
        let thumb_width = self.metrics.scaled(88.0);
        let strip = (count > 1).then(|| {
            div()
                .id("extension-screenshot-strip")
                .flex()
                .gap(self.metrics.spacing4())
                .p(self.metrics.spacing5())
                .overflow_x_scroll()
                .children(art.screenshots.iter().enumerate().map(|(at, source)| {
                    let selected = at == index;
                    let id = format!("extension-screenshot-{at}");
                    div()
                        .id(SharedString::from(id.clone()))
                        .role(gpui::Role::Button)
                        .accessibility_id(id)
                        .aria_label(tr!("Screenshot %@", (at + 1).to_string()))
                        .aria_selected(selected)
                        .flex_none()
                        .w(thumb_width)
                        .h(thumb_width * 0.625)
                        .rounded(self.metrics.radius_sm())
                        .bg(self.theme.hover)
                        .border_2()
                        .border_color(if selected {
                            self.theme.accent
                        } else {
                            gpui::transparent_black()
                        })
                        .cursor_pointer()
                        .when(!selected, |thumb| {
                            thumb.opacity(0.75).hover(|thumb| thumb.opacity(1.0))
                        })
                        .focus(|thumb| thumb.border_color(self.theme.accent))
                        .button_interaction(cx.listener(move |view, _, _, cx| {
                            view.screenshot = at;
                            cx.notify();
                        }))
                        .child(self.screenshot_fill(
                            Some(source),
                            Size::Card,
                            |image| image.rounded(self.metrics.radius_sm()),
                            cx,
                        ))
                }))
        });
        self.card()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(main)
            .children(strip)
    }

    /// Version, installs, author, categories, and links.
    fn info(&self, details: &ExtensionDetails, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let about = &details.about;
        let version = details
            .version
            .as_ref()
            .or(details.current_version.as_ref());
        let fact = |label: SharedString, value: AnyElement| {
            div()
                .flex()
                .flex_col()
                .gap(self.metrics.spacing1())
                .child(
                    div()
                        .text_size(self.metrics.font_footnote())
                        .text_color(self.theme.fg_muted)
                        .child(label),
                )
                .child(value)
        };
        let text = |value: String| div().min_w_0().truncate().child(value).into_any_element();
        let link = |id: &'static str, label: &'static str, url: &String| {
            let url = url.clone();
            self.button(
                id,
                label,
                move |view, _, _| {
                    if let Err(error) = crate::opener::open_url(&url, None) {
                        view.error = Some(error.to_string());
                    }
                },
                cx,
            )
            .child(
                SymbolGlyph::new(
                    "arrow.up.right",
                    self.metrics.icon_xs(),
                    self.theme.fg_muted,
                )
                .or("arrow.up.right.square"),
            )
        };
        let links: Vec<_> = [
            about
                .homepage
                .as_ref()
                .map(|url| link("extension-homepage", tr_key!("Homepage"), url)),
            about
                .repository
                .as_ref()
                .map(|url| link("extension-repository", tr_key!("Repository"), url)),
        ]
        .into_iter()
        .flatten()
        .collect();
        self.card()
            .id("extension-info")
            .flex()
            .flex_col()
            .gap(self.metrics.spacing6())
            .p(self.metrics.spacing7())
            .when_some(version, |info, version| {
                info.child(fact(tr!("Version"), text(version.clone())))
            })
            .when_some(about.downloads, |info, downloads| {
                info.child(fact(
                    tr!("Installs"),
                    text(super::cards::grouped(downloads)),
                ))
            })
            .when_some(about.author.clone(), |info, author| {
                info.child(fact(tr!("Author"), text(author)))
            })
            .when(!about.categories.is_empty(), |info| {
                info.child(fact(
                    tr!("Categories"),
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(self.metrics.spacing3())
                        .children(
                            about
                                .categories
                                .iter()
                                .map(|category| self.badge(category.replace('-', " "), false)),
                        )
                        .into_any_element(),
                ))
            })
            .when(!links.is_empty(), |info| {
                info.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(self.metrics.spacing4())
                        .children(links),
                )
            })
    }

    fn update_section(
        &self,
        extension: &Extension,
        details: &ExtensionDetails,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let available = marketplace::available_update(extension, details);
        let owner = extension.name.clone();
        let actions = div()
            .flex()
            .flex_wrap()
            .gap(self.metrics.spacing4())
            .child(self.button(
                "extension-check-update",
                tr_key!("Check for updates"),
                move |view, _, cx| view.details(owner.clone(), cx),
                cx,
            ))
            .when(available.is_some(), |actions| {
                actions.child(self.button(
                    "extension-update",
                    tr_key!("Update"),
                    |view, _, cx| view.install(cx),
                    cx,
                ))
            });
        let message = if let Some(version) = available {
            tr!("Version %@ is available.", version)
        } else if self.completed == Some(Mutation::Update) {
            tr!("Extension updated.")
        } else if details.current_version.is_some() {
            tr!("This extension is up to date.")
        } else {
            tr!("Check the marketplace for a newer version.")
        };
        let mut content = div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing4())
            .text_color(self.theme.fg_muted)
            .child(accessibility::label("extension-update-status", message));
        if available.is_some() {
            let added: Vec<_> = details
                .permissions
                .iter()
                .flatten()
                .filter(|permission| !extension.manifest.permissions.contains(*permission))
                .map(String::as_str)
                .collect();
            if !added.is_empty() {
                content = content.child(accessibility::label(
                    "extension-update-permissions",
                    tr!(
                        "New permissions: %@. After updating, review the permissions and enable the extension again.",
                        added.join(", ")
                    ),
                ));
            }
        }
        self.section_with_actions(&tr!("Updates"), actions, content)
    }

    fn permissions_body(&self, details: &ExtensionDetails) -> gpui::Div {
        let permissions = details.permissions.as_ref();
        let mut permission_tags = div().flex().flex_wrap().gap(self.metrics.spacing4());
        for permission in permissions.into_iter().flatten() {
            permission_tags = permission_tags.child(
                self.badge(permission.clone(), false)
                    .id(SharedString::from(format!(
                        "extension-permission-{permission}"
                    )))
                    .role(gpui::Role::Label)
                    .aria_value(permission.clone()),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing6())
            .child(permission_tags)
            .when_some(permissions_note(permissions), |body, note| {
                body.child(
                    div()
                        .text_color(self.theme.fg_muted)
                        .child(accessibility::label("extension-permissions-note", note)),
                )
            })
            .child(
                div()
                    .text_size(self.metrics.font_footnote())
                    .text_color(self.theme.fg_muted)
                    .child(tr!(
                        "Commands and file or Git changes ask for permission when used."
                    )),
            )
    }
}
