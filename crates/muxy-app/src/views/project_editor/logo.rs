use crate::model::AppModel;
use crate::views::overlays::{Mark, Overlay, OverlayView};
use gpui::{
    AnyElement, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Point, RenderImage, Styled, Window, canvas, div, img, point, px,
};
use image::ImageDecoder;
use muxy_app_core::{ProjectId, ProjectStatus};
use muxy_ui::tr;
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

pub(crate) type Cache = HashMap<ProjectId, (Arc<[u8]>, Arc<gpui::Image>)>;

pub(crate) struct Cropper {
    project: ProjectId,
    source: image::RgbaImage,
    preview: Entity<PreviewImage>,
    crop: Crop,
    drag: Option<(Point<gpui::Pixels>, Point<f32>)>,
    error: Option<String>,
}

struct PreviewImage {
    image: Arc<RenderImage>,
}

#[derive(Clone, Copy)]
struct Crop {
    width: f32,
    height: f32,
    zoom: f32,
    offset: Point<f32>,
}

#[allow(
    clippy::cast_precision_loss,
    reason = "Crop dimensions are bounded to 1024 pixels"
)]
impl Crop {
    fn scale(self) -> f32 {
        self.zoom / self.width.min(self.height)
    }
    fn clamp(&mut self) {
        let scale = self.scale();
        let x = (self.width * scale - 1.0) / 2.0;
        let y = (self.height * scale - 1.0) / 2.0;
        self.offset.x = self.offset.x.clamp(-x, x);
        self.offset.y = self.offset.y.clamp(-y, y);
    }
    fn zoom(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(1.0, 5.0);
        self.clamp();
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn rectangle(self) -> (u32, u32, u32) {
        let side = (1.0 / self.scale()).round().max(1.0) as u32;
        let x = ((self.width - side as f32) / 2.0 - self.offset.x / self.scale())
            .round()
            .max(0.0) as u32;
        let y = ((self.height - side as f32) / 2.0 - self.offset.y / self.scale())
            .round()
            .max(0.0) as u32;
        (
            x.min(self.width as u32 - side),
            y.min(self.height as u32 - side),
            side,
        )
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "Source dimensions are bounded to 1024 pixels"
)]
impl Cropper {
    pub(crate) fn new(project: ProjectId, source: image::RgbaImage, cx: &mut gpui::App) -> Self {
        let crop = Crop {
            width: source.width() as f32,
            height: source.height() as f32,
            zoom: 1.0,
            offset: point(0.0, 0.0),
        };
        let mut bgra = source.clone();
        for pixel in bgra.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        Self {
            project,
            source,
            preview: cx.new(|cx| {
                cx.on_release(|preview: &mut PreviewImage, cx| {
                    let image = preview.image.clone();
                    cx.defer(move |cx| cx.drop_image(image, None));
                })
                .detach();
                PreviewImage {
                    image: Arc::new(RenderImage::new([image::Frame::new(bgra)])),
                }
            }),
            crop,
            drag: None,
            error: None,
        }
    }

    fn encode(&self) -> Result<Arc<[u8]>, String> {
        let (x, y, side) = self.crop.rectangle();
        let cropped = image::imageops::crop_imm(&self.source, x, y, side, side).to_image();
        let resized =
            image::imageops::resize(&cropped, 256, 256, image::imageops::FilterType::Lanczos3);
        let mut bytes = Cursor::new(Vec::new());
        resized
            .write_to(&mut bytes, image::ImageFormat::Png)
            .map_err(|error| error.to_string())?;
        Ok(bytes.into_inner().into())
    }
}

const MAX_SIDE: u32 = 8192;
const MAX_DECODED: u64 = 64 * 1024 * 1024;

/// Reads a logo file of at most 20 MB and returns it at most 1024 pixels on
/// its longer side. Raster images are decoded within `MAX_SIDE` pixels and
/// `MAX_DECODED` bytes, and anything the image crate doesn't recognise is
/// tried as an SVG.
fn load(path: &Path) -> Result<image::RgbaImage, String> {
    const MAX_FILE: u64 = 20 * 1024 * 1024;
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(tr!("Choose an image smaller than 20 MB.").to_string());
    }
    decode(&bytes)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    if image::guess_format(bytes).is_err() {
        return rasterize_svg(bytes);
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODED);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| unsupported())?;
    let orientation = decoder.orientation().map_err(|error| error.to_string())?;
    let mut image =
        image::DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    image.apply_orientation(orientation);
    if image.width() == 0 || image.height() == 0 {
        return Err(tr!("The image is empty.").to_string());
    }
    if image.width() <= 1024 && image.height() <= 1024 {
        return Ok(image.into_rgba8());
    }
    Ok(image.thumbnail(1024, 1024).to_rgba8())
}

fn unsupported() -> String {
    tr!("Choose a PNG, JPEG, GIF, WebP, or SVG image up to 8192 pixels and 64 MB decoded.")
        .to_string()
}

fn system_fonts() -> Arc<resvg::usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut fonts = resvg::usvg::fontdb::Database::new();
            fonts.load_system_fonts();
            Arc::new(fonts)
        })
        .clone()
}

/// Tells whether resvg may decode an image embedded in an SVG logo. resvg
/// decodes without limits, so a raster image is drawn only when its header
/// declares a size within the limits `load` applies to other images.
fn fits_decode_limits(kind: &resvg::usvg::ImageKind) -> bool {
    use resvg::usvg::ImageKind;
    let (ImageKind::JPEG(data)
    | ImageKind::PNG(data)
    | ImageKind::GIF(data)
    | ImageKind::WEBP(data)) = kind
    else {
        return true;
    };
    let dimensions = image::ImageReader::new(Cursor::new(data.as_slice()))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok());
    let Some((width, height)) = dimensions else {
        return false;
    };
    width <= MAX_SIDE
        && height <= MAX_SIDE
        && u64::from(width) * u64::from(height) * 4 <= MAX_DECODED
}

fn is_gzip(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1f, 0x8b])
}

/// Draws an SVG at most 1024 pixels on its longer side, like `load` does for
/// other images. Text is set in the system fonts, which are scanned once, the
/// first time an SVG asks for a font. Embedded images are drawn when they fit
/// `fits_decode_limits`, linked images are ignored so a logo can't read other
/// files, and compressed SVGs, embedded or not, are refused because usvg
/// inflates them without a limit.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Both sides are clamped to 1..=1024 before the cast"
)]
fn rasterize_svg(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    use resvg::{tiny_skia, usvg};
    const SIDE: f32 = 1024.0;
    if is_gzip(bytes) {
        return Err(unsupported());
    }
    let mut options = usvg::Options::default();
    let select_font = usvg::FontResolver::default_font_selector();
    options.font_resolver.select_font = Box::new(move |font, fonts| {
        if fonts.is_empty() {
            *fonts = system_fonts();
        }
        select_font(font, fonts)
    });
    let resolve_data = usvg::ImageHrefResolver::default_data_resolver();
    options.image_href_resolver.resolve_data = Box::new(move |mime, data, svg_options| {
        if is_gzip(&data) {
            return None;
        }
        resolve_data(mime, data, svg_options).filter(fits_decode_limits)
    });
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|_| unsupported())?;
    let size = tree.size();
    let scale = SIDE / size.width().max(size.height());
    let width = (size.width() * scale).round().clamp(1.0, SIDE) as u32;
    let height = (size.height() * scale).round().clamp(1.0, SIDE) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| tr!("The image is empty.").to_string())?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(width, height, pixels).ok_or_else(unsupported)
}

impl AppModel {
    pub(crate) fn sync_project_logos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut retired = Vec::new();
        self.project_logos.retain(|id, (_, image)| {
            let keep = self
                .state
                .project(*id)
                .is_some_and(|project| project.logo.is_some());
            if !keep {
                retired.push(image.clone());
            }
            keep
        });
        for project in self.state.projects() {
            let Some(logo) = &project.logo else {
                continue;
            };
            if let Some((cached, _)) = self.project_logos.get_mut(&project.id)
                && (Arc::ptr_eq(cached, logo) || cached == logo)
            {
                *cached = logo.clone();
                continue;
            }
            if let Some((_, previous)) = self.project_logos.insert(
                project.id,
                (
                    logo.clone(),
                    Arc::new(gpui::Image::from_bytes(
                        gpui::ImageFormat::Png,
                        logo.to_vec(),
                    )),
                ),
            ) {
                retired.push(previous);
            }
        }
        for image in retired {
            if self
                .project_logos
                .values()
                .any(|(_, active)| active.id() == image.id())
            {
                continue;
            }
            if let Some(decoded) = image.clone().get_render_image(window, cx) {
                cx.drop_image(decoded, Some(window));
            }
            image.remove_asset(cx);
        }
    }

    pub(crate) fn choose_project_logo(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.choose_project_logo_with(project, prompt, cx);
    }

    pub(crate) fn choose_project_logo_with<F>(
        &mut self,
        project: ProjectId,
        prompt: impl FnOnce(Option<&Path>, &mut Context<Self>) -> F,
        cx: &mut Context<Self>,
    ) where
        F: Future<Output = Result<Option<PathBuf>, String>> + 'static,
    {
        let Some(target) = self
            .state
            .project(project)
            .filter(|project| project.status() == ProjectStatus::Available)
        else {
            return;
        };
        let directory = target
            .server_id
            .is_local()
            .then_some(target.directory.as_path());
        let result = prompt(directory, cx);
        self.project_logo_task = Some(cx.spawn(async move |model, cx| {
            let path = match result.await {
                Ok(path) => path,
                Err(error) => {
                    let _ = model.update(cx, |model, cx| {
                        model.fail(tr!("Could not choose logo: %@", error).to_string(), cx);
                    });
                    None
                }
            };
            let Some(path) = path else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move { load(&path) })
                .await;
            let _ = model.update(cx, |model, cx| {
                if model.overlay.is_some()
                    || !model
                        .state
                        .project(project)
                        .is_some_and(|project| project.status() == ProjectStatus::Available)
                {
                    return;
                }
                match result {
                    Ok(source) => {
                        model.overlay_subscription = None;
                        let cropper = Cropper::new(project, source, cx);
                        model.open_overlay(Overlay::ProjectLogo(cropper), cx);
                        model.focus_owner.request(&model.overlay_focus);
                        cx.notify();
                    }
                    Err(error) => {
                        model.fail(tr!("Could not load logo: %@", error).to_string(), cx);
                    }
                }
            });
        }));
    }

    fn zoom_project_logo(&mut self, factor: f32, cx: &mut Context<Self>) {
        if let Some(Overlay::ProjectLogo(cropper)) = &mut self.overlay {
            cropper.crop.zoom(factor);
            cx.notify();
        }
    }

    fn apply_project_logo(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::ProjectLogo(cropper)) = &self.overlay else {
            return;
        };
        let project = cropper.project;
        let result = cropper.encode();
        match result {
            Ok(logo)
                if self.edit_project(
                    |state| state.set_project_logo(project, Some(logo.clone())),
                    cx,
                ) =>
            {
                self.dismiss_overlay(cx);
            }
            Ok(_) => {
                if let Some(Overlay::ProjectLogo(cropper)) = &mut self.overlay {
                    cropper.error.clone_from(&self.error);
                }
            }
            Err(error) => {
                if let Some(Overlay::ProjectLogo(cropper)) = &mut self.overlay {
                    cropper.error = Some(error);
                }
            }
        }
    }
}

fn prompt(
    directory: Option<&Path>,
    cx: &mut Context<AppModel>,
) -> gpui::Task<Result<Option<PathBuf>, String>> {
    if let Some(directory) = directory {
        let (sender, receiver) = async_channel::bounded(1);
        let dialog =
            muxy_ui::dialog::choose_file(cx, &tr!("Choose Logo"), directory, move |path| {
                let _ = sender.try_send(path);
            });
        return cx.spawn(async move |_, _| {
            let _dialog = dialog.map_err(|error| error.to_string())?;
            receiver
                .recv()
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())
        });
    }
    let result = cx.prompt_for_paths(gpui::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(tr!("Choose Logo")),
    });
    cx.spawn(async move |_, _| {
        result
            .await
            .map_err(|error| error.to_string())?
            .map(|paths| paths.and_then(|paths| paths.into_iter().next()))
            .map_err(|error| error.to_string())
    })
}

fn preview(
    cropper: &Cropper,
    side: gpui::Pixels,
    radius: gpui::Pixels,
    background: gpui::Hsla,
    cx: &gpui::App,
) -> gpui::Div {
    let scale = f32::from(side) * cropper.crop.scale();
    let width = px(cropper.crop.width * scale);
    let height = px(cropper.crop.height * scale);
    div()
        .relative()
        .flex_none()
        .size(side)
        .overflow_hidden()
        .child(
            img(cropper.preview.read(cx).image.clone())
                .absolute()
                .w(width)
                .h(height)
                .left((side - width) / 2.0 + side * cropper.crop.offset.x)
                .top((side - height) / 2.0 + side * cropper.crop.offset.y),
        )
        .child(corner_mask(radius, background))
}

// GPUI's content mask clips rectangles. Cover the four corners without
// decoding or uploading another image as the user drags or zooms.
fn corner_mask(radius: gpui::Pixels, color: gpui::Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            for (origin, sx, sy) in [
                (bounds.origin, 1.0, 1.0),
                (point(bounds.right(), bounds.top()), -1.0, 1.0),
                (point(bounds.right(), bounds.bottom()), -1.0, -1.0),
                (point(bounds.left(), bounds.bottom()), 1.0, -1.0),
            ] {
                let p = |x: f32, y: f32| origin + point(radius * x * sx, radius * y * sy);
                let mut path = gpui::PathBuilder::fill();
                path.move_to(p(0.0, 0.0));
                path.line_to(p(1.0, 0.0));
                path.cubic_bezier_to(p(0.0, 1.0), p(0.447_715_25, 0.0), p(0.0, 0.447_715_25));
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .absolute()
    .inset_0()
}

impl OverlayView for Cropper {
    fn render(
        &self,
        _: Option<Mark>,
        model: &AppModel,
        window: &Window,
        cx: &mut Context<AppModel>,
    ) -> AnyElement {
        render(self, model, window, cx)
    }
}

fn render(
    cropper: &Cropper,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let width = m
        .scaled(300.0)
        .min((window.viewport_size().width - px(16.0)).max(px(1.0)));
    let side = m
        .scaled(240.0)
        .min((width - m.spacing8() * 2.0).max(px(1.0)))
        .min((window.viewport_size().height - m.scaled(180.0)).max(px(1.0)));
    let style = muxy_ui::controls::Style { theme, metrics: &m };
    muxy_ui::popover::modal(
        theme,
        m,
        muxy_ui::popover::modal_card(
            theme,
            m,
            window.viewport_size(),
            "project-logo-dialog",
            tr!("Project Logo"),
            width,
        )
        .debug_selector(|| "project-logo-cropper".into())
        .gap(m.spacing6())
        .track_focus(&model.overlay_focus)
        .on_key_down(cx.listener(|model, event: &gpui::KeyDownEvent, _, cx| {
            match event.keystroke.key.as_str() {
                "escape" => model.dismiss_overlay(cx),
                "enter" => model.apply_project_logo(cx),
                _ => {}
            }
        }))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(tr!("Crop Logo")),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .justify_center()
                .child(crop_surface(cropper, side, model, cx)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.spacing4())
                .child(preview(
                    cropper,
                    m.icon_xxl(),
                    m.radius_md(),
                    theme.raised(),
                    cx,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(m.font_caption())
                        .text_color(theme.fg_muted)
                        .child(tr!("Drag to reposition, scroll to zoom")),
                )
                .child(muxy_ui::controls::button(
                    style,
                    "logo-zoom-out",
                    muxy_ui::controls::Button::secondary("−")
                        .named(&tr!("Zoom Out"))
                        .enabled(cropper.crop.zoom > 1.0),
                    cx.listener(|model, _, _, cx| model.zoom_project_logo(1.0 / 1.2, cx)),
                ))
                .child(muxy_ui::controls::button(
                    style,
                    "logo-zoom-in",
                    muxy_ui::controls::Button::secondary("+")
                        .named(&tr!("Zoom In"))
                        .enabled(cropper.crop.zoom < 5.0),
                    cx.listener(|model, _, _, cx| model.zoom_project_logo(1.2, cx)),
                )),
        )
        .children(cropper.error.as_ref().map(|error| {
            muxy_ui::accessibility::announced(
                div().id("project-logo-error").text_color(theme.danger),
                error.clone(),
                true,
            )
            .child(error.clone())
        }))
        .child(footer(model, cx)),
    )
}

fn crop_surface(
    cropper: &Cropper,
    side: gpui::Pixels,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    let radius = side * (f32::from(m.radius_md()) / f32::from(m.icon_xxl()));
    preview(cropper, side, radius, theme.raised(), cx)
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(radius)
                .border_1()
                .border_color(theme.border),
        )
        .id("project-logo-crop")
        .bg(theme.bg)
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|model, event: &gpui::MouseDownEvent, _, cx| {
                if let Some(Overlay::ProjectLogo(cropper)) = &mut model.overlay {
                    cropper.drag = Some((event.position, cropper.crop.offset));
                }
                cx.stop_propagation();
            }),
        )
        .on_mouse_move(
            cx.listener(move |model, event: &gpui::MouseMoveEvent, _, cx| {
                if let Some(Overlay::ProjectLogo(cropper)) = &mut model.overlay
                    && let Some((start, offset)) = cropper.drag
                {
                    if event.pressed_button != Some(MouseButton::Left) {
                        cropper.drag = None;
                        return;
                    }
                    cropper.crop.offset = point(
                        offset.x + f32::from(event.position.x - start.x) / f32::from(side),
                        offset.y + f32::from(event.position.y - start.y) / f32::from(side),
                    );
                    cropper.crop.clamp();
                    cx.notify();
                }
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|model, _, _, _| {
                if let Some(Overlay::ProjectLogo(cropper)) = &mut model.overlay {
                    cropper.drag = None;
                }
            }),
        )
        .on_scroll_wheel(cx.listener(|model, event: &gpui::ScrollWheelEvent, _, cx| {
            let delta = f32::from(event.delta.pixel_delta(px(20.0)).y);
            model.zoom_project_logo((1.0 + delta * 0.01).clamp(0.5, 2.0), cx);
            cx.stop_propagation();
        }))
}

fn footer(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let style = muxy_ui::controls::Style {
        theme: &model.theme,
        metrics: &model.metrics,
    };
    div()
        .flex()
        .justify_between()
        .child(
            muxy_ui::controls::button(
                style,
                "logo-cancel",
                muxy_ui::controls::Button::secondary(&tr!("Cancel")),
                cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
            )
            .debug_selector(|| "logo-cancel".into()),
        )
        .child(apply_button(model, cx))
}

fn apply_button(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    muxy_ui::controls::button(
        muxy_ui::controls::Style {
            theme: &model.theme,
            metrics: &model.metrics,
        },
        "logo-apply",
        muxy_ui::controls::Button::primary(&tr!("Apply")),
        cx.listener(|model, _, _, cx| model.apply_project_logo(cx)),
    )
    .debug_selector(|| "logo-apply".into())
}

#[cfg(test)]
mod tests {
    use super::{load, rasterize_svg};

    #[test]
    fn load_draws_svg_bounded_whatever_size_it_declares() {
        let file = tempfile::Builder::new()
            .suffix(".svg")
            .tempfile()
            .expect("file");
        std::fs::write(
            file.path(),
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="1000000" height="1000"><rect width="1000000" height="1000" fill="red"/></svg>"#,
        )
        .expect("write");
        let image = load(file.path()).expect("svg");
        assert_eq!((image.width(), image.height()), (1024, 1));
        assert!(image.pixels().any(|pixel| pixel.0[3] != 0));
    }

    #[test]
    fn svg_draws_embedded_images_within_limits_but_not_linked_or_gzipped_ones() {
        let red = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="red"/></svg>"#;
        let file = tempfile::Builder::new()
            .suffix(".svg")
            .tempfile()
            .expect("file");
        std::fs::write(file.path(), red).expect("write");
        let svg = |href: &str| {
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><image href="{href}" width="4" height="4" preserveAspectRatio="none"/></svg>"#
            )
        };
        let opaque = |href: &str| {
            rasterize_svg(svg(href).as_bytes())
                .expect("svg")
                .pixels()
                .any(|pixel| pixel.0[3] != 0)
        };
        let embedded = format!(
            "data:image/svg+xml;base64,{}",
            muxy_core::base64::encode(red)
        );
        let gzipped = "data:image/svg+xml;base64,H4sIAAAAAAAC/22MSQ6AIAwAv9L0AZaoJwN8RpCS4BJorM9X7t4mmcnYdid49nI0hyxyLUSqOug0nDXRaIyhr0DQHIQdzggcc2Lp6G2Nq/wq2HIpDmsMSN72hX8BNPb9iWoAAAA=";
        let raster = |format: image::ImageFormat, width, height| {
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::RgbImage::from_pixel(width, height, image::Rgb([255, 0, 0]))
                .write_to(&mut bytes, format)
                .expect("encode");
            format!(
                "data:{};base64,{}",
                format.to_mime_type(),
                muxy_core::base64::encode(bytes.get_ref())
            )
        };
        assert!(opaque(&embedded));
        assert!(!opaque(gzipped));
        assert!(!opaque(&file.path().to_string_lossy()));
        assert!(opaque(&raster(image::ImageFormat::Png, 4, 1)));
        for format in [
            image::ImageFormat::Png,
            image::ImageFormat::Jpeg,
            image::ImageFormat::Gif,
            image::ImageFormat::WebP,
        ] {
            assert!(
                !opaque(&raster(format, super::MAX_SIDE + 1, 1)),
                "{format:?}"
            );
        }
        assert!(!opaque(&raster(image::ImageFormat::Png, 4096, 4097)));
    }
}
