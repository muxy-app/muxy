//! Extension topbar, rail, and status-bar items: their effective values,
//! icons, and clicks. Items whose command opens a popover anchor it to
//! themselves.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, Window, canvas, div,
};
use muxy_app_core::extensions::{Action, Extension, Icon, Side};
use muxy_ui::components::{ButtonInteraction, SymbolGlyph, TemplateGlyph};

use super::AppModel;
use crate::views::titlebar;

const FALLBACK: &str = "puzzlepiece.extension";
const MAX_ICON_BYTES: u64 = 256 * 1024;

/// Where an item shows, which also decides where its popover opens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Placement {
    Topbar,
    Rail,
    StatusBar,
}

/// One visible item after runtime overrides are applied.
#[derive(Clone, Debug)]
pub(crate) struct Item {
    /// Names the item across the manifest list it was declared in.
    pub key: String,
    pub placement: Placement,
    pub owner: String,
    pub id: String,
    pub icon: Icon,
    pub text: Option<String>,
    pub tooltip: String,
    pub command: String,
    pub root: PathBuf,
}

/// An item's tooltip, with the key of the command it runs when that has one.
fn item_tooltip(item: &Item, cx: &gpui::App) -> gpui::SharedString {
    let name = gpui::SharedString::from(item.tooltip.clone());
    match super::RunCommand::key(&item.owner, &item.command, cx) {
        Some(key) => muxy_ui::shortcuts::with_key(&name, &key),
        None => name,
    }
}

/// The cache key an icon renders under and the validated file it loads from.
fn svg_path(extension: &Extension, icon: &Icon) -> Option<(PathBuf, PathBuf)> {
    let Icon::Svg(svg) = icon else {
        return None;
    };
    extension
        .resource(svg)
        .ok()
        .filter(|_| svg.to_ascii_lowercase().ends_with(".svg"))
        .map(|file| (extension.directory.join(svg), file))
}

impl AppModel {
    /// Topbar items, followed by each extension's rail items while the rail
    /// is turned off.
    pub(crate) fn topbar_items(&self) -> Vec<Item> {
        let mut items = Vec::new();
        for extension in self.extensions.registry.active() {
            self.bar_items(extension, "top", Placement::Topbar, &mut items);
            if !self.appearance.rail_enabled {
                self.bar_items(extension, "rail", Placement::Topbar, &mut items);
            }
        }
        items
    }

    /// Rail items; none while the rail is turned off.
    pub(crate) fn rail_items(&self) -> Vec<Item> {
        let mut items = Vec::new();
        if self.appearance.rail_enabled {
            for extension in self.extensions.registry.active() {
                self.bar_items(extension, "rail", Placement::Rail, &mut items);
            }
        }
        items
    }

    /// The rail is open and has something to show.
    pub(crate) fn rail_shown(&self) -> bool {
        self.appearance.rail_expanded && !self.rail_items().is_empty()
    }

    pub(crate) fn toggle_rail(&mut self, cx: &mut Context<Self>) {
        self.appearance.rail_expanded = !self.appearance.rail_expanded;
        self.save_appearance(cx);
        cx.notify();
    }

    /// The item named `key` is on screen at `placement`.
    pub(crate) fn extension_item_shown(&self, key: &str, placement: Placement) -> bool {
        let items = match placement {
            Placement::Topbar => self.topbar_items(),
            Placement::Rail if self.rail_shown() => self.rail_items(),
            Placement::StatusBar if self.appearance.status_bar_visible => [Side::Left, Side::Right]
                .into_iter()
                .flat_map(|side| self.status_items(side))
                .collect(),
            Placement::Rail | Placement::StatusBar => Vec::new(),
        };
        items.iter().any(|item| item.key == key)
    }

    fn bar_items(
        &self,
        extension: &Extension,
        list: &str,
        placement: Placement,
        items: &mut Vec<Item>,
    ) {
        let (declared, changes) = if list == "rail" {
            (&extension.manifest.rail_items, &self.extensions.items.rail)
        } else {
            (
                &extension.manifest.topbar_items,
                &self.extensions.items.topbar,
            )
        };
        for item in declared {
            let change = changes.get(&(extension.name.clone(), item.id.clone()));
            if change
                .and_then(|change| change.visible)
                .unwrap_or(item.visible)
            {
                items.push(Item {
                    key: format!("{list}:{}:{}", extension.name, item.id),
                    placement,
                    owner: extension.name.clone(),
                    id: item.id.clone(),
                    icon: change
                        .and_then(|change| change.icon.clone())
                        .unwrap_or_else(|| item.icon.clone()),
                    text: None,
                    tooltip: item.tooltip.clone().unwrap_or_else(|| item.id.clone()),
                    command: item.command.clone(),
                    root: extension.directory.clone(),
                });
            }
        }
    }

    pub(crate) fn status_items(&self, side: Side) -> Vec<Item> {
        let mut items = Vec::new();
        for extension in self.extensions.registry.active() {
            for item in extension
                .manifest
                .status_bar_items
                .iter()
                .filter(|item| item.side == side)
            {
                let change = self
                    .extensions
                    .items
                    .status
                    .get(&(extension.name.clone(), item.id.clone()));
                if change
                    .and_then(|change| change.visible)
                    .unwrap_or(item.visible)
                {
                    items.push(Item {
                        key: format!("status:{}:{}", extension.name, item.id),
                        placement: Placement::StatusBar,
                        owner: extension.name.clone(),
                        id: item.id.clone(),
                        icon: change
                            .and_then(|change| change.icon.clone())
                            .unwrap_or_else(|| item.icon.clone()),
                        text: change
                            .and_then(|change| change.text.clone())
                            .or_else(|| item.text.clone())
                            .filter(|text| !text.is_empty()),
                        tooltip: item.tooltip.clone().unwrap_or_else(|| item.id.clone()),
                        command: item.command.clone(),
                        root: extension.directory.clone(),
                    });
                }
            }
        }
        items
    }

    /// An SF Symbol, or a loaded template SVG tinted with `color`. An icon
    /// that cannot be drawn shows the extension glyph in its place.
    pub(crate) fn extension_icon(
        &self,
        root: &Path,
        icon: &Icon,
        size: Pixels,
        color: Hsla,
    ) -> AnyElement {
        match icon {
            Icon::Symbol(symbol) => SymbolGlyph::new(symbol.clone(), size, color)
                .or(FALLBACK)
                .into_any_element(),
            Icon::Svg(svg) => {
                let path = root.join(svg);
                match self.extensions.items.icons.get(&path) {
                    Some(bytes) => {
                        use std::hash::{Hash, Hasher};
                        let mut hash = std::collections::hash_map::DefaultHasher::new();
                        bytes.hash(&mut hash);
                        TemplateGlyph::new(
                            format!("extension-svg:{}:{:x}", path.display(), hash.finish()),
                            bytes.clone(),
                            size,
                            color,
                            FALLBACK,
                        )
                        .into_any_element()
                    }
                    None => SymbolGlyph::new(FALLBACK, size, color).into_any_element(),
                }
            }
        }
    }

    /// Reads every template SVG the enabled extensions show, off the main thread.
    pub(super) fn load_extension_icons(&mut self, cx: &mut Context<Self>) {
        let mut paths: Vec<(PathBuf, PathBuf)> = Vec::new();
        for extension in self.extensions.registry.active() {
            let manifest = &extension.manifest;
            let icons = manifest
                .topbar_items
                .iter()
                .chain(&manifest.rail_items)
                .map(|item| &item.icon)
                .chain(manifest.status_bar_items.iter().map(|item| &item.icon))
                .chain(
                    manifest
                        .panels
                        .iter()
                        .filter_map(|panel| panel.icon.as_ref()),
                )
                .chain(
                    manifest
                        .panels
                        .iter()
                        .flat_map(|panel| panel.header_buttons.iter().map(|button| &button.icon)),
                )
                .chain(
                    manifest
                        .sidebar
                        .iter()
                        .filter_map(|sidebar| sidebar.icon.as_ref()),
                );
            paths.extend(icons.filter_map(|icon| svg_path(extension, icon)));
            for ((owner, _), change) in self
                .extensions
                .items
                .topbar
                .iter()
                .chain(&self.extensions.items.rail)
                .chain(&self.extensions.items.status)
            {
                if *owner == extension.name {
                    paths.extend(
                        change
                            .icon
                            .as_ref()
                            .and_then(|icon| svg_path(extension, icon)),
                    );
                }
            }
        }
        paths.retain(|(key, _)| !self.extensions.items.icons.contains_key(key));
        paths.sort();
        paths.dedup();
        if paths.is_empty() {
            return;
        }
        let load = crate::extensions::io::run(move || {
            Ok(paths
                .into_iter()
                .filter_map(|(key, file)| {
                    use std::io::Read;
                    let mut bytes = Vec::new();
                    std::fs::File::open(&file)
                        .and_then(|file| file.take(MAX_ICON_BYTES + 1).read_to_end(&mut bytes))
                        .ok()
                        .filter(|size| *size as u64 <= MAX_ICON_BYTES)
                        .map(|_| (key, Arc::<[u8]>::from(bytes)))
                })
                .collect::<Vec<_>>())
        });
        cx.spawn(async move |model, cx| {
            let Ok(icons) = load.await else {
                return;
            };
            let _ = model.update(cx, |model, cx| {
                model.extensions.items.icons.extend(icons);
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens the command's popover anchored to the item, or runs the command.
    pub(crate) fn click_extension_item(
        &mut self,
        item: &Item,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let popover = self
            .extensions
            .registry
            .enabled(&item.owner)
            .filter(|extension| extension.allows("panels:write"))
            .and_then(|extension| extension.manifest.command(&item.command))
            .and_then(|command| match &command.action {
                Action::OpenPopover { popover } => Some(popover.clone()),
                _ => None,
            });
        match popover {
            Some(popover) => {
                self.toggle_extension_popover(
                    &item.owner,
                    &popover,
                    &item.key,
                    item.placement,
                    window,
                    cx,
                );
            }
            None => self.run_extension_command(&item.owner, &item.command, window, cx),
        }
    }

    pub(crate) fn extension_toolbar_width(&self) -> f32 {
        f32::from(u16::try_from(self.topbar_items().len()).unwrap_or(u16::MAX))
            * f32::from(self.metrics.control_medium() + self.metrics.spacing2())
    }

    pub(crate) fn extension_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_none()
            .items_center()
            .children(
                self.topbar_items()
                    .into_iter()
                    .map(|item| self.item_button(item, cx)),
            )
            .into_any_element()
    }

    /// The rail's buttons, top to bottom.
    pub(crate) fn rail_buttons(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.rail_items()
            .into_iter()
            .map(|item| self.item_button(item, cx))
            .collect()
    }

    fn item_button(&self, item: Item, cx: &mut Context<Self>) -> AnyElement {
        let anchor = self.extensions.items.anchor(&item.key);
        let icon = self.extension_icon(
            &item.root,
            &item.icon,
            self.metrics.icon_md(),
            self.theme.fg_muted,
        );
        let rail = item.placement == Placement::Rail;
        let element_id = gpui::SharedString::from(format!("extension-item-{}", item.key));
        let tooltip = item_tooltip(&item, cx);
        titlebar::action_button(element_id, tooltip, self)
            .aria_label(item.tooltip.clone())
            .debug_selector(move || {
                if rail {
                    "rail-item".into()
                } else {
                    "extension-toolbar-item".into()
                }
            })
            .when(rail, Styled::mr_0)
            .button_interaction(cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.click_extension_item(&item, window, cx);
            }))
            .child(
                canvas(
                    move |bounds, _, _| anchor.set(Some(bounds)),
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(icon)
            .into_any_element()
    }

    /// Status-bar items for one side, each with the separator main draws.
    pub(crate) fn extension_status_items(
        &self,
        side: Side,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let metrics = self.metrics;
        let mut elements = Vec::new();
        for item in self.status_items(side) {
            let anchor = self.extensions.items.anchor(&item.key);
            let tooltip = item_tooltip(&item, cx);
            let icon = self.extension_icon(
                &item.root,
                &item.icon,
                metrics.font_caption(),
                self.theme.fg_muted,
            );
            let text = item.text.clone();
            let element_id =
                gpui::SharedString::from(format!("extension-status-{}-{}", item.owner, item.id));
            elements.push(
                div()
                    .debug_selector(|| "extension-status-item".into())
                    .id(element_id)
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(metrics.scaled(4.0))
                    .h_full()
                    .px(metrics.scaled(4.0))
                    .text_color(self.theme.fg_muted)
                    .cursor_pointer()
                    .hover(|style| style.text_color(self.theme.fg))
                    .tooltip(muxy_ui::components::tooltip(tooltip, &self.theme))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .button_interaction(cx.listener(move |model, _, window, cx| {
                        model.click_extension_item(&item, window, cx);
                    }))
                    .child(
                        canvas(
                            move |bounds, _, _| anchor.set(Some(bounds)),
                            |_, (), _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(icon)
                    .children(text.map(|text| {
                        div()
                            .text_size(metrics.font_footnote())
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(text)
                    }))
                    .into_any_element(),
            );
        }
        elements
    }
}
