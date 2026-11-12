//! The listing icons and screenshots this page has loaded, kept until it
//! closes so scrolling and switching tabs never fetch one twice.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{Context, ObjectFit, ParentElement, Pixels, RenderImage, Styled, StyledImage, div, img};
use muxy_app_core::extensions::Extension;
use muxy_ui::components::SymbolGlyph;

use super::ExtensionsView;
use crate::extensions::{
    marketplace,
    pictures::{self, Size, Source},
};

enum State {
    Loading,
    Ready(Arc<RenderImage>),
    Missing,
}

/// Where one extension's listing pictures come from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Art {
    pub(super) icon: Option<Source>,
    pub(super) screenshots: Vec<Source>,
}

impl Art {
    /// The marketplace's pictures for `name`, which has `screenshots` of them.
    pub(super) fn marketplace(name: &str, screenshots: usize) -> Self {
        Self {
            icon: marketplace::icon_url(name).map(Source::Marketplace),
            screenshots: (0..screenshots)
                .filter_map(|index| marketplace::screenshot_url(name, index))
                .map(Source::Marketplace)
                .collect(),
        }
    }

    /// The pictures an installed extension ships, which need no network.
    fn installed(extension: &Extension) -> Self {
        let listing = &extension.manifest.marketplace;
        let file = |path: &String| extension.resource(path).ok().map(Source::File);
        Self {
            icon: listing.icon.as_ref().and_then(file),
            screenshots: listing.screenshots.iter().filter_map(file).collect(),
        }
    }
}

/// Frees textures once the current update ends: while a window is being
/// updated it is out of the app's list, so freeing at once would miss it.
fn drop_later(images: Vec<Arc<RenderImage>>, cx: &mut gpui::App) {
    if !images.is_empty() {
        cx.defer(move |cx| {
            for image in images {
                cx.drop_image(image, None);
            }
        });
    }
}

#[derive(Default)]
pub(super) struct Pictures {
    loaded: RefCell<HashMap<(Source, Size), State>>,
    /// Installed extensions' pictures, by folder and version, so drawing a
    /// row never touches the disk.
    installed: RefCell<HashMap<(PathBuf, String), Art>>,
}

impl Pictures {
    /// Frees every loaded picture's texture.
    pub(super) fn release(&self, cx: &mut gpui::App) {
        self.installed.borrow_mut().clear();
        self.forget(|_, _| true, cx);
    }

    /// Forgets pictures read from extension folders, which a reload may change.
    pub(super) fn forget_files(&self, cx: &mut gpui::App) {
        self.installed.borrow_mut().clear();
        self.forget(|source, _| matches!(source, Source::File(_)), cx);
    }

    /// Forgets marketplace screenshots, the large pictures, when Browse starts
    /// a new list, so browsing many lists doesn't keep every one. Icons are
    /// small and stay.
    pub(super) fn forget_listed_screenshots(&self, cx: &mut gpui::App) {
        self.forget(
            |source, size| matches!(source, Source::Marketplace(_)) && *size != Size::Icon,
            cx,
        );
    }

    fn forget(&self, matches: impl Fn(&Source, &Size) -> bool, cx: &mut gpui::App) {
        let mut images = Vec::new();
        self.loaded.borrow_mut().retain(|(source, size), state| {
            if !matches(source, size) {
                return true;
            }
            if let State::Ready(image) = state {
                images.push(image.clone());
            }
            false
        });
        drop_later(images, cx);
    }

    pub(super) fn installed(&self, extension: &Extension) -> Art {
        self.installed
            .borrow_mut()
            .entry((extension.directory.clone(), extension.version.clone()))
            .or_insert_with(|| Art::installed(extension))
            .clone()
    }
}

impl ExtensionsView {
    /// The picture at `source`, once loaded. Asking for one starts loading it.
    pub(super) fn picture(
        &self,
        source: &Source,
        size: Size,
        cx: &Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let key = (source.clone(), size);
        {
            let mut loaded = self.pictures.loaded.borrow_mut();
            match loaded.get(&key) {
                Some(State::Ready(image)) => return Some(image.clone()),
                Some(State::Loading | State::Missing) => return None,
                None => {
                    loaded.insert(key.clone(), State::Loading);
                }
            }
        }
        let source = source.clone();
        cx.spawn(async move |view, cx| {
            let result =
                crate::extensions::io::pictures(move || pictures::load(&source, size)).await;
            let _ = view.update(cx, |view, cx| {
                let state = match result {
                    Ok(image) => State::Ready(Arc::new(image)),
                    Err(error) => {
                        crate::diagnostics::event(
                            "extension-picture-skipped",
                            format_args!("error={error}"),
                        );
                        State::Missing
                    }
                };
                let mut loaded = view.pictures.loaded.borrow_mut();
                if let Some(slot) = loaded.get_mut(&key) {
                    *slot = state;
                } else if let State::Ready(image) = state {
                    drop_later(vec![image], cx);
                }
                cx.notify();
            });
        })
        .detach();
        None
    }

    /// An extension's icon on a square tile, or the extension glyph until it
    /// loads or when it has none.
    pub(super) fn icon_tile(
        &self,
        source: Option<&Source>,
        side: Pixels,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let radius = side * 0.22;
        let tile = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(side)
            .rounded(radius);
        match source.and_then(|source| self.picture(source, Size::Icon, cx)) {
            Some(image) => tile.child(
                img(image)
                    .size_full()
                    .rounded(radius)
                    .object_fit(ObjectFit::Contain),
            ),
            None => tile
                .bg(self.theme.hover)
                .border_1()
                .border_color(self.theme.border)
                .child(SymbolGlyph::new(
                    "puzzlepiece.extension.fill",
                    side * 0.5,
                    self.theme.fg_muted,
                )),
        }
    }

    /// A screenshot filling its parent, or a quiet placeholder until it loads
    /// or when there is none.
    pub(super) fn screenshot_fill(
        &self,
        source: Option<&Source>,
        size: Size,
        rounding: impl Fn(gpui::Img) -> gpui::Img,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        use gpui::IntoElement;
        match source.and_then(|source| self.picture(source, size, cx)) {
            Some(image) => {
                rounding(img(image).size_full().object_fit(ObjectFit::Cover)).into_any_element()
            }
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    SymbolGlyph::new("photo", self.metrics.icon_xxl(), self.theme.fg_dim)
                        .or("puzzlepiece.extension"),
                )
                .into_any_element(),
        }
    }
}
