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

pub(super) const MAX_SCREENSHOTS: usize = 6;

enum State {
    Loading(u64),
    Ready(Arc<RenderImage>),
    Missing,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Art {
    pub(super) icon: Option<Source>,
    pub(super) screenshots: Vec<Source>,
}

impl Art {
    pub(super) fn marketplace(name: &str, screenshots: usize) -> Self {
        Self {
            icon: marketplace::icon_url(name).map(Source::Marketplace),
            screenshots: (0..screenshots.min(MAX_SCREENSHOTS))
                .filter_map(|index| marketplace::screenshot_url(name, index))
                .map(Source::Marketplace)
                .collect(),
        }
    }

    fn installed(extension: &Extension) -> Self {
        let listing = &extension.manifest.marketplace;
        let file = |path: &String| extension.resource(path).ok().map(Source::File);
        Self {
            icon: listing.icon.as_ref().and_then(file),
            screenshots: listing
                .screenshots
                .iter()
                .take(MAX_SCREENSHOTS)
                .filter_map(file)
                .collect(),
        }
    }
}

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
    requests: std::cell::Cell<u64>,
    installed: RefCell<HashMap<(PathBuf, String), Art>>,
}

impl Pictures {
    pub(super) fn release(&self, cx: &mut gpui::App) {
        self.installed.borrow_mut().clear();
        self.forget(|_, _| true, cx);
    }

    pub(super) fn forget_files(&self, cx: &mut gpui::App) {
        self.installed.borrow_mut().clear();
        self.forget(|source, _| matches!(source, Source::File(_)), cx);
    }

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
    pub(super) fn picture(
        &self,
        source: &Source,
        size: Size,
        cx: &Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let key = (source.clone(), size);
        let request = self.pictures.requests.get() + 1;
        {
            let mut loaded = self.pictures.loaded.borrow_mut();
            match loaded.get(&key) {
                Some(State::Ready(image)) => return Some(image.clone()),
                Some(State::Loading(_) | State::Missing) => return None,
                None => {
                    self.pictures.requests.set(request);
                    loaded.insert(key.clone(), State::Loading(request));
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
                if let Some(slot) = loaded
                    .get_mut(&key)
                    .filter(|slot| matches!(slot, State::Loading(owner) if *owner == request))
                {
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
