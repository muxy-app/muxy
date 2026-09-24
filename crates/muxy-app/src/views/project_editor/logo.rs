use crate::{model::AppModel, views::overlays::Overlay};
use gpui::{
    AnyElement, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Point, RenderImage, StatefulInteractiveElement, Styled, Window,
    canvas, div, img, point, px,
};
use image::ImageDecoder;
use muxy_app_core::{ProjectId, ProjectStatus};
use muxy_ui::components::ButtonInteraction;
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::Path,
    sync::Arc,
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

fn load(path: &Path) -> Result<image::RgbaImage, String> {
    const MAX_FILE: u64 = 20 * 1024 * 1024;
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err("Choose an image smaller than 20 MB.".into());
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| {
        "Choose a PNG, JPEG, GIF, or WebP image up to 8192 pixels and 64 MB decoded.".to_owned()
    })?;
    let orientation = decoder.orientation().map_err(|error| error.to_string())?;
    let mut image =
        image::DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    image.apply_orientation(orientation);
    if image.width() == 0 || image.height() == 0 {
        return Err("The image is empty.".into());
    }
    Ok(image.thumbnail(1024, 1024).to_rgba8())
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
        if !self
            .state
            .project(project)
            .is_some_and(|project| project.status() == ProjectStatus::Available)
        {
            return;
        }
        let result = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose Logo".into()),
        });
        self.project_logo_task = Some(cx.spawn(async move |model, cx| {
            let path = match result.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) => None,
                result => {
                    let _ = model.update(cx, |model, cx| {
                        model.fail(format!("Could not choose logo: {result:?}"), cx);
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
                        model.overlay =
                            Some(Overlay::ProjectLogo(Cropper::new(project, source, cx)));
                        model.focus_requested = false;
                        let focus = model.overlay_focus.clone();
                        let _ = model.window.update(cx, |_, window, _| focus.focus(window));
                        cx.notify();
                    }
                    Err(error) => model.fail(format!("Could not load logo: {error}"), cx),
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

pub(crate) fn render(
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
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            muxy_ui::popover::surface(theme, m)
                .shadow(muxy_ui::theme::Elevation::Modal.shadow(theme.bg))
                .id("project-logo-dialog")
                .debug_selector(|| "project-logo-cropper".into())
                .max_h((window.viewport_size().height - px(16.0)).max(px(1.0)))
                .overflow_y_scroll()
                .p(m.spacing8())
                .gap(m.spacing6())
                .w(width)
                .track_focus(&model.overlay_focus)
                .on_key_down(cx.listener(|model, event: &gpui::KeyDownEvent, _, cx| {
                    match event.keystroke.key.as_str() {
                        "escape" => model.dismiss_overlay(cx),
                        "enter" => model.apply_project_logo(cx),
                        _ => {}
                    }
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Crop Logo"))
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
                                .child("Drag to reposition, scroll to zoom"),
                        )
                        .child(muxy_ui::controls::button(
                            style,
                            "logo-zoom-out",
                            "−",
                            cropper.crop.zoom > 1.0,
                            cx.listener(|model, _, _, cx| model.zoom_project_logo(1.0 / 1.2, cx)),
                        ))
                        .child(muxy_ui::controls::button(
                            style,
                            "logo-zoom-in",
                            "+",
                            cropper.crop.zoom < 5.0,
                            cx.listener(|model, _, _, cx| model.zoom_project_logo(1.2, cx)),
                        )),
                )
                .children(
                    cropper
                        .error
                        .as_ref()
                        .map(|error| div().text_color(theme.danger).child(error.clone())),
                )
                .child(footer(model, cx)),
        )
        .into_any_element()
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
                "Cancel",
                true,
                cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
            )
            .debug_selector(|| "logo-cancel".into()),
        )
        .child(apply_button(model, cx))
}

fn apply_button(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .id("logo-apply")
        .debug_selector(|| "logo-apply".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(m.control_medium())
        .px(m.spacing5())
        .rounded(m.radius_sm())
        .bg(theme.accent)
        .text_color(theme.accent_foreground)
        .text_size(m.font_footnote())
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(|style| style.opacity(0.85))
        .button_interaction(cx.listener(|model, _, _, cx| model.apply_project_logo(cx)))
        .child("Apply")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_stays_square_and_inside_portrait_landscape_and_extreme_aspect_images() {
        for (width, height) in [(100.0, 300.0), (300.0, 100.0), (1.0, 1024.0), (1024.0, 1.0)] {
            let mut crop = Crop {
                width,
                height,
                zoom: 1.0,
                offset: point(0.0, 0.0),
            };
            for zoom in [1.0, 5.0, 0.1, 100.0] {
                crop.zoom(zoom);
                for offset in [-1000.0, 0.0, 1000.0] {
                    crop.offset = point(offset, -offset);
                    crop.clamp();
                    let (x, y, side) = crop.rectangle();
                    assert!(side > 0);
                    assert!(f64::from(x + side) <= f64::from(width));
                    assert!(f64::from(y + side) <= f64::from(height));
                }
            }
        }
    }

    #[gpui::test]
    fn cropped_logo_preserves_alpha_and_produces_bounded_png(cx: &mut gpui::TestAppContext) {
        let image = image::RgbaImage::from_pixel(80, 40, image::Rgba([240, 60, 10, 128]));
        let cropper = cx.update(|cx| Cropper::new(ProjectId::new(), image, cx));
        let png = cropper.encode().expect("encode");
        assert_eq!(
            muxy_protocol::ProjectPatch::Logo(Some(png.clone())).validate(),
            Ok(())
        );
        let decoded = image::load_from_memory(&png).expect("decode").to_rgba8();
        assert_eq!(decoded.dimensions(), (256, 256));
        assert_eq!(decoded.get_pixel(128, 128).0, [240, 60, 10, 128]);
    }

    #[test]
    fn load_rejects_non_images_and_oversized_input() {
        let file = tempfile::NamedTempFile::new().expect("file");
        std::fs::write(file.path(), b"not an image").expect("write");
        assert!(load(file.path()).is_err());
        file.as_file()
            .set_len(20 * 1024 * 1024 + 1)
            .expect("resize");
        assert!(load(file.path()).is_err());
    }
}
