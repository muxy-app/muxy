//! Extension topbar and status-bar items: their effective values, icons, and
//! clicks. Items whose command opens a popover anchor it to themselves.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    AnyElement, AppContext, Context, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, Window, canvas, div, px,
};
use muxy_app_core::extensions::{Action, Extension, Icon, Side};
use muxy_ui::components::{ButtonInteraction, SymbolGlyph, TemplateGlyph, Tooltip};

use super::AppModel;

const FALLBACK: &str = "puzzlepiece.extension";
const MAX_ICON_BYTES: u64 = 256 * 1024;

/// One visible item after runtime overrides are applied.
#[derive(Clone, Debug)]
pub(crate) struct Item {
    pub owner: String,
    pub id: String,
    pub icon: Icon,
    pub text: Option<String>,
    pub tooltip: String,
    pub command: String,
    pub root: PathBuf,
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
    pub(crate) fn topbar_items(&self) -> Vec<Item> {
        let mut items = Vec::new();
        for extension in self.extensions.registry.active() {
            for item in &extension.manifest.topbar_items {
                let change = self
                    .extensions
                    .items
                    .topbar
                    .get(&(extension.name.clone(), item.id.clone()));
                if change
                    .and_then(|change| change.visible)
                    .unwrap_or(item.visible)
                {
                    items.push(Item {
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
        items
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

    /// An SF Symbol, or a loaded template SVG tinted with `color`.
    pub(crate) fn extension_icon(
        &self,
        root: &Path,
        icon: &Icon,
        size: Pixels,
        color: Hsla,
    ) -> AnyElement {
        match icon {
            Icon::Symbol(symbol) => {
                SymbolGlyph::new(symbol.clone(), size, color).into_any_element()
            }
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
        anchor: &str,
        above: bool,
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
                self.toggle_extension_popover(&item.owner, &popover, anchor, above, window, cx);
            }
            None => self.run_extension_command(&item.owner, &item.command, window, cx),
        }
    }

    pub(crate) fn extension_toolbar_width(&self) -> f32 {
        f32::from(u16::try_from(self.topbar_items().len()).unwrap_or(u16::MAX)) * 28.0
    }

    pub(crate) fn extension_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut bar = div().flex().items_center().gap(px(2.0));
        for item in self.topbar_items() {
            let key = format!("top:{}:{}", item.owner, item.id);
            let anchor = self.extensions.items.anchor(&key);
            let tooltip = item.tooltip.clone();
            let theme = self.theme.clone();
            let icon = self.extension_icon(&item.root, &item.icon, px(14.0), self.theme.fg_muted);
            let element_id =
                gpui::SharedString::from(format!("extension-toolbar-{}-{}", item.owner, item.id));
            bar = bar.child(
                div()
                    .debug_selector(|| "extension-toolbar-item".into())
                    .id(element_id)
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(26.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|view| view.bg(self.theme.hover))
                    .tooltip(move |_, cx| {
                        cx.new(|_| {
                            Tooltip::new(
                                tooltip.clone(),
                                theme.raised(),
                                theme.fg,
                                theme.border,
                                theme.bg,
                            )
                        })
                        .into()
                    })
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .button_interaction(cx.listener(move |model, _, window, cx| {
                        model.click_extension_item(&item, &key, false, window, cx);
                    }))
                    .child(
                        canvas(
                            move |bounds, _, _| anchor.set(Some(bounds)),
                            |_, (), _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(icon),
            );
        }
        bar.into_any_element()
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
            let key = format!("status:{}:{}", item.owner, item.id);
            let anchor = self.extensions.items.anchor(&key);
            let tooltip = item.tooltip.clone();
            let theme = self.theme.clone();
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
                    .gap(px(4.0))
                    .h_full()
                    .px(px(4.0))
                    .text_color(self.theme.fg_muted)
                    .cursor_pointer()
                    .hover(|style| style.text_color(self.theme.fg))
                    .tooltip(move |_, cx| {
                        cx.new(|_| {
                            Tooltip::new(
                                tooltip.clone(),
                                theme.raised(),
                                theme.fg,
                                theme.border,
                                theme.bg,
                            )
                        })
                        .into()
                    })
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .button_interaction(cx.listener(move |model, _, window, cx| {
                        model.click_extension_item(&item, &key, true, window, cx);
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
