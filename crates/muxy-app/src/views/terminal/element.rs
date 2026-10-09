mod background;
mod block;
mod box_drawing;
mod cache;
#[cfg(test)]
mod cache_tests;
mod decoration;
mod dots;
mod glyph;
pub(super) mod images;
mod layers;
mod padding;
pub(super) mod shade;

use std::{rc::Rc, sync::Arc};

pub(crate) use cache::Rows;

use gpui::{
    App, Bounds, Entity, FontFeatures, FontStyle, FontWeight, Hsla, IntoElement, LineLayout,
    Pixels, Point, Styled, TextRun, Window, WrappedLine, WrappedLineLayout, canvas, fill, font,
    point, px, rgb, size,
};
use muxy_protocol::{MAX_COLS, MAX_ROWS, Run, Size, Style};

use super::{colors::Palette, pane::TerminalPane};

#[derive(Default)]
struct RowPainting {
    lines: Vec<(Point<Pixels>, WrappedLine)>,
    glyphs: Vec<PaintGlyph>,
    shades: Vec<shade::Shade>,
    backgrounds: Vec<(Bounds<Pixels>, Hsla)>,
    blocks: Vec<(Bounds<Pixels>, Hsla)>,
    block_layers: layers::QuadLayers,
    paths: layers::Paths,
    decorations: Vec<(Bounds<Pixels>, Hsla)>,
}

#[derive(Default)]
struct Painting {
    #[cfg(test)]
    unbatched: bool,
    #[cfg(test)]
    sequential_quads: bool,
    rows: Vec<Rc<RowPainting>>,
    thicken: f32,
    shade_sprites: Vec<shade::Sprite>,
    images: Vec<images::Placement>,
    selections: Vec<(Bounds<Pixels>, Hsla)>,
    matches: Vec<(Bounds<Pixels>, bool)>,
    decorations: Vec<(Bounds<Pixels>, Hsla)>,
    cursor: Option<Bounds<Pixels>>,
    cursor_shape: muxy_protocol::CursorShape,
    cursor_thickness: muxy_app_core::settings::CellHeight,
    cursor_color: Hsla,
    cursor_text: Hsla,
    background_opacity: f32,
    cell: gpui::Size<Pixels>,
}

struct PaintGlyph {
    origin: Point<Pixels>,
    row_top: Pixels,
    font: gpui::FontId,
    id: gpui::GlyphId,
    size: Pixels,
    color: Hsla,
    emoji: bool,
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep terminal layout and input registration in the same canvas"
)]
pub(crate) fn terminal(view: Entity<TerminalPane>, palette: &Palette) -> impl IntoElement {
    let palette = *palette;
    let mouse_view = view.clone();
    canvas(
        move |bounds, window, cx| {
            let _span = crate::profiler::span(crate::profiler::Metric::TerminalPrepaint);
            let (base_font, font_size, cell) =
                typography(&view.read(cx).terminal, &palette, window);
            let frame = padding::Frame::configured(bounds, cell, &view.read(cx).terminal.options);
            let viewport = frame.viewport;
            #[cfg(target_os = "macos")]
            if let Some(native) = &view.read(cx).native_scroll {
                let _span = crate::profiler::span(crate::profiler::Metric::NativeScrollSync);
                let pane = view.read(cx);
                native.sync(muxy_ui::native_scroll::ScrollGeometry {
                    bounds,
                    content_height: f64::from(f32::from(bounds.size.height))
                        + pane.scrollable_rows(viewport.rows) * f64::from(f32::from(cell.height)),
                    from_bottom: pane.scroll.requested_pixels(f32::from(cell.height)),
                    line_height: f64::from(f32::from(cell.height)),
                    revision: pane.scroll.revision,
                    dark: ((palette.background >> 16) & 0xff) * 299
                        + ((palette.background >> 8) & 0xff) * 587
                        + (palette.background & 0xff) * 114
                        < 128_000,
                });
            }
            let physical_cell = physical_cell(cell, window.scale_factor());
            let weak = view.downgrade();
            window.defer(cx, move |_, cx| {
                let _ = weak.update(cx, |pane, cx| {
                    pane.cell_height = f32::from(cell.height);
                    pane.set_viewport(viewport, cx);
                    pane.geometry = Some((frame.content, cell));
                    if let Some(channel) = pane.channel()
                        && pane.sent_cell_size != Some((channel, physical_cell))
                    {
                        pane.sent_cell_size = Some((channel, physical_cell));
                        cx.emit(super::pane::PaneEvent::CellSize(channel, physical_cell));
                    }
                });
            });
            view.update(cx, |pane, cx| pane.sync_cursor_blink(window, cx));
            let mut painting = prepare(
                view.read(cx),
                frame.content.origin,
                cell,
                &palette,
                &base_font,
                font_size,
                window,
            );
            view.update(cx, |pane, cx| {
                let shades: Vec<_> = painting
                    .rows
                    .iter()
                    .flat_map(|row| row.shades.iter().copied())
                    .collect();
                painting.shade_sprites =
                    pane.shades
                        .prepare(&shades, frame.content, window.scale_factor());
                for image in pane.shades.retired() {
                    cx.drop_image(image, Some(window));
                }
                if let Some(grid) = pane.displayed_grid() {
                    let history = isize::try_from(grid.history.len()).unwrap_or(isize::MAX);
                    let start = isize::try_from(pane.visible_start(grid)).unwrap_or(isize::MAX);
                    if let Ok(offset) = i16::try_from(history - start) {
                        let origin = frame.content.origin
                            + point(
                                px(0.0),
                                cell.height * f32::from(offset)
                                    + px(pane.scroll.pixel_remainder(f32::from(cell.height))),
                            );
                        let graphics = grid.graphics.clone();
                        painting.images = pane.images.prepare(&graphics, origin, cell);
                        for image in pane.images.retired() {
                            cx.drop_image(image, Some(window));
                        }
                    }
                }
            });
            (painting, frame, frame.backgrounds(view.read(cx), &palette))
        },
        move |bounds, (painting, frame, padding), window, cx| {
            let _span = crate::profiler::span(crate::profiler::Metric::TerminalPaint);
            let focus_border = mouse_view.read(cx).focus_border;
            let focus = mouse_view.read(cx).focus.clone();
            window.handle_input(
                &focus,
                gpui::ElementInputHandler::new(frame.content, mouse_view.clone()),
                cx,
            );
            let event_view = mouse_view.clone();
            window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                if phase.bubble() {
                    event_view.update(cx, |pane, cx| pane.mouse_move(event, cx));
                }
            });
            if mouse_view
                .read(cx)
                .terminal
                .options
                .background_opacity_cells
            {
                let clip = frame.grid.intersect(&frame.content);
                let colored = painting
                    .rows
                    .iter()
                    .flat_map(|row| &row.backgrounds)
                    .map(|(bounds, _)| bounds.intersect(&clip))
                    .chain(padding.iter().map(|(bounds, _)| *bounds));
                let mut color: Hsla = rgb(palette.background).into();
                color.a = painting.background_opacity;
                for bounds in background::uncovered(bounds, colored) {
                    window.paint_quad(fill(bounds, color));
                }
            }
            for (bounds, mut color) in padding {
                color.a *= painting.background_opacity;
                window.paint_quad(fill(bounds, color));
            }
            window.with_content_mask(
                Some(gpui::ContentMask {
                    bounds: frame.grid.intersect(&frame.content),
                }),
                |window| {
                    paint(painting, &palette, window, cx);
                    super::ime::paint(&mouse_view, window, cx);
                },
            );
            if let Some(color) = focus_border {
                window.paint_quad(gpui::outline(bounds, color, gpui::BorderStyle::Solid));
            }
        },
    )
    .size_full()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn viewport_size(bounds: gpui::Size<Pixels>, cell: gpui::Size<Pixels>) -> Size {
    Size {
        cols: (bounds.width / cell.width)
            .floor()
            .clamp(1.0, f32::from(MAX_COLS)) as u16,
        rows: (bounds.height / cell.height)
            .floor()
            .clamp(1.0, f32::from(MAX_ROWS)) as u16,
    }
}

fn typography(
    terminal: &muxy_app_core::settings::TerminalSettings,
    palette: &Palette,
    window: &Window,
) -> (gpui::Font, Pixels, gpui::Size<Pixels>) {
    let font_size = px(terminal.font_size);
    let mut base_font = font(
        terminal
            .font_families
            .first()
            .cloned()
            .unwrap_or_else(|| "Menlo".into()),
    );
    base_font.fallbacks = Some(gpui::FontFallbacks::from_fonts(
        terminal.font_families.iter().skip(1).cloned().collect(),
    ));
    base_font.features = FontFeatures(Arc::new(terminal.font.features.clone()));
    let sample = window.text_system().shape_line(
        "M".into(),
        font_size,
        &[text_run(1, Style::default(), palette, &base_font)],
        None,
    );
    let cell = size(
        px(terminal
            .font
            .cell_width
            .apply(f32::from(sample.width), window.scale_factor())),
        px(terminal.cell_height.apply(
            f32::from(sample.ascent + sample.descent),
            window.scale_factor(),
        )),
    );
    (base_font, font_size, cell)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn physical_cell(cell: gpui::Size<Pixels>, scale: f32) -> muxy_protocol::CellSize {
    muxy_protocol::CellSize {
        width: (f32::from(cell.width) * scale).round().clamp(1.0, 4096.0) as u16,
        height: (f32::from(cell.height) * scale).round().clamp(1.0, 4096.0) as u16,
    }
}

struct RowRenderer<'a> {
    settings: &'a muxy_app_core::settings::FontOptions,
    cell: gpui::Size<Pixels>,
    palette: &'a Palette,
    base_font: &'a gpui::Font,
    font_size: Pixels,
    metrics: (Pixels, Pixels),
    window: &'a mut Window,
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep preparation of a complete frame together"
)]
fn prepare(
    view: &TerminalPane,
    origin: Point<Pixels>,
    cell: gpui::Size<Pixels>,
    palette: &Palette,
    base_font: &gpui::Font,
    font_size: Pixels,
    window: &mut Window,
) -> Painting {
    let _span = crate::profiler::span(crate::profiler::Metric::TerminalPrepare);
    let mut painting = Painting {
        cell,
        cursor_thickness: view.terminal.options.cursor_thickness,
        background_opacity: if view.terminal.options.background_opacity_cells {
            view.background_alpha()
        } else {
            1.0
        },
        thicken: if view.terminal.font.thicken {
            f32::from(view.terminal.font.thicken_strength) / 255.0
        } else {
            0.0
        },
        ..Painting::default()
    };
    let Some(grid) = view.displayed_grid() else {
        view.rows.borrow_mut().clear();
        return painting;
    };
    let height = view.viewport().map_or(grid.size.rows, |size| size.rows);
    let start = view.visible_start(grid);
    let remainder = view.scroll.pixel_remainder(f32::from(cell.height));
    let origin = origin + point(px(0.0), px(remainder));
    let metrics = window.text_system().shape_line(
        "M".into(),
        font_size,
        &[text_run(1, Style::default(), palette, base_font)],
        None,
    );
    let painting_thickens = painting.thicken > 0.0;
    let mut renderer = RowRenderer {
        settings: &view.terminal.font,
        cell,
        palette,
        base_font,
        font_size,
        metrics: (metrics.ascent, metrics.descent),
        window,
    };
    let count = usize::from(height) + usize::from(remainder < 0.0);
    let mut rows = view.rows.borrow_mut();
    rows.begin(&renderer, count);
    for index in 0..count {
        let Some(runs) = grid.content_row(start + index) else {
            continue;
        };
        let row = u16::try_from(index).unwrap_or(MAX_ROWS);
        let position = origin + point(px(0.0), cell.height * f32::from(row));
        search_highlights(view, start + index, position, cell, &mut painting);
        link_highlight(view, start + index, position, cell, palette, &mut painting);
        if let Some(bounds) = selection_bounds(view, start + index, position, cell) {
            let mut color: Hsla = rgb(palette
                .selection_background
                .map_or(palette.indexed(4), |color| {
                    color.resolve(palette.foreground, palette.background)
                }))
            .into();
            if palette.selection_background.is_none() {
                color.a = 0.35;
            }
            painting.selections.push((bounds, color));
            let mut column = 0;
            if let Some(color) = palette.selection_background {
                for run in runs {
                    let run_bounds = Bounds::new(
                        position + point(cell.width * f32::from(column), px(0.0)),
                        size(cell.width * f32::from(run.width), cell.height),
                    );
                    let selected = run_bounds.intersect(&bounds);
                    if selected.size.width > px(0.0) {
                        let (fg, bg) = palette.style(run.style);
                        painting
                            .selections
                            .push((selected, rgb(color.resolve(fg, bg)).into()));
                    }
                    column += run.width;
                }
            }
        }
        let selected = view.selection.and_then(|selection| {
            let row =
                isize::try_from(start + index).ok()? - isize::try_from(grid.history.len()).ok()?;
            let columns = selection.columns(row, grid);
            (!columns.is_empty() && palette.selection_foreground.is_some())
                .then(|| selected_runs(runs, columns, palette))
        });
        painting.rows.push(rows.prepare(
            index,
            selected.as_deref().unwrap_or(runs),
            position,
            painting_thickens || (view.scroll.view.is_none() && row == grid.cursor.row),
            &mut renderer,
        ));
    }
    if view.scroll.view.is_none()
        && view.focused
        && view.cursor_blink.visible
        && grid.cursor.visible
        && grid.cursor.col < grid.size.cols
        && grid.cursor.row < height
    {
        painting.cursor_shape = grid.cursor.shape;
        let cursor_style = grid
            .content_row(grid.history.len() + usize::from(grid.cursor.row))
            .and_then(|runs| {
                let mut column = 0;
                runs.iter()
                    .find(|run| {
                        column += run.width;
                        column > grid.cursor.col
                    })
                    .map(|run| run.style)
            })
            .unwrap_or_default();
        let (foreground, background) = palette.style(cursor_style);
        painting.cursor_color = rgb(palette.cursor_color.map_or(palette.cursor, |color| {
            color.resolve(foreground, background)
        }))
        .into();
        painting.cursor_color.a = palette.cursor_opacity;
        painting.cursor_text = rgb(palette.cursor_text.map_or(palette.background, |color| {
            color.resolve(foreground, background)
        }))
        .into();
        painting.cursor = Some(Bounds::new(
            origin
                + point(
                    cell.width * f32::from(grid.cursor.col),
                    cell.height * f32::from(grid.cursor.row),
                ),
            cell,
        ));
    }
    painting
}

fn selected_runs(runs: &[Run], selected: std::ops::Range<u16>, palette: &Palette) -> Vec<Run> {
    let mut result = Vec::new();
    let mut column = 0;
    for run in runs {
        let end = column + run.width;
        let start_selected = selected.start.max(column).min(end);
        let end_selected = selected.end.max(column).min(end);
        if start_selected >= end_selected {
            result.push(run.clone());
        } else {
            let (foreground, background) = palette.style(run.style);
            let rgb = |color: u32| {
                let [_, r, g, b] = color.to_be_bytes();
                muxy_protocol::Color::Rgb(r, g, b)
            };
            let style = Style {
                fg: rgb(palette
                    .selection_foreground
                    .map_or(foreground, |color| color.resolve(foreground, background))),
                bg: if run.style.inverse {
                    rgb(background)
                } else {
                    run.style.bg
                },
                inverse: false,
                ..run.style
            };
            if run.text.is_ascii() && run.text.len() == usize::from(run.width) {
                for (start, end, style) in [
                    (column, start_selected, run.style),
                    (start_selected, end_selected, style),
                    (end_selected, end, run.style),
                ] {
                    if start < end {
                        result.push(Run {
                            text: run.text[usize::from(start - column)..usize::from(end - column)]
                                .into(),
                            width: end - start,
                            style,
                        });
                    }
                }
            } else {
                result.push(Run {
                    style,
                    ..run.clone()
                });
            }
        }
        column = end;
    }
    result
}

impl RowRenderer<'_> {
    fn row(
        &mut self,
        runs: &[Run],
        position: Point<Pixels>,
        painting: &mut RowPainting,
        collect_glyphs: bool,
    ) {
        let mut column = 0_u16;
        let mut text = String::new();
        let mut styles = Vec::new();
        let mut columns = Vec::new();
        for run in runs {
            let (foreground, background) = self.palette.style(run.style);
            let bounds = Bounds::new(
                position + point(self.cell.width * f32::from(column), px(0.0)),
                size(self.cell.width * f32::from(run.width), self.cell.height),
            );
            if background != self.palette.background
                || run.style.bg != muxy_protocol::Color::Default
                || run.style.inverse
            {
                push_quad(&mut painting.backgrounds, bounds, rgb(background).into());
            }
            let mut foreground: Hsla = rgb(foreground).into();
            if run.style.faint {
                foreground.a = 0.5;
            }
            if run.style.invisible {
                column = column.saturating_add(run.width);
                continue;
            }
            decoration::prepare(
                &mut painting.decorations,
                run.style,
                bounds,
                foreground,
                self.palette,
                self.window.scale_factor(),
            );
            let mapped = self.settings.codepoints.iter().any(|map| {
                run.text
                    .chars()
                    .any(|ch| (map.start..=map.end).contains(&u32::from(ch)))
            });
            let first_block = painting.blocks.len();
            if !mapped && shade::prepare(&run.text, bounds, foreground, &mut painting.shades) {
            } else if let Some(quads) = (!mapped)
                .then(|| block::quads(&run.text, bounds, foreground, self.window.scale_factor()))
                .flatten()
            {
                painting.blocks.extend(quads);
            } else if !mapped
                && glyph::prepare(
                    &run.text,
                    bounds,
                    foreground,
                    self.window.scale_factor(),
                    &mut painting.blocks,
                    &mut painting.paths,
                )
            {
            } else {
                let parts = font_parts(run, self.settings);
                let mut next_column = column;
                for part in parts.as_deref().unwrap_or(std::slice::from_ref(run)) {
                    let mut font = self.base_font.clone();
                    configure_font(&mut font, part, self.settings);
                    append_run(
                        part,
                        next_column,
                        &mut text,
                        &mut styles,
                        &mut columns,
                        self.palette,
                        &font,
                    );
                    next_column += part.width;
                }
            }
            painting
                .block_layers
                .append_cell(&painting.blocks, first_block);
            column = column.saturating_add(run.width);
        }

        self.shape(
            text,
            &styles,
            &columns,
            column,
            position,
            painting,
            collect_glyphs,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn shape(
        &mut self,
        text: String,
        styles: &[TextRun],
        columns: &[u16],
        column: u16,
        position: Point<Pixels>,
        painting: &mut RowPainting,
        collect_glyphs: bool,
    ) {
        if !text.is_empty() {
            let Ok(mut lines) = self.window.text_system().shape_text(
                text.into(),
                self.font_size,
                styles,
                None,
                None,
            ) else {
                return;
            };
            let Some(mut line) = lines.pop() else {
                return;
            };
            align_to_cells(&mut line, columns, self.cell.width, column, self.metrics);
            let baseline = position
                + point(
                    px(0.0),
                    (self.cell.height - line.ascent() - line.descent()) / 2.0 + line.ascent(),
                );
            for shaped in line.runs().iter().filter(|_| collect_glyphs) {
                for glyph in &shaped.glyphs {
                    let mut end = 0;
                    let color = styles
                        .iter()
                        .find_map(|style| {
                            end += style.len;
                            (glyph.index < end).then_some(style.color)
                        })
                        .unwrap_or_else(|| rgb(self.palette.foreground).into());
                    painting.glyphs.push(PaintGlyph {
                        origin: baseline + glyph.position,
                        row_top: position.y,
                        font: shaped.font_id,
                        id: glyph.id,
                        size: self.font_size,
                        color,
                        emoji: glyph.is_emoji,
                    });
                }
            }
            painting.lines.push((position, line));
        }
    }
}

fn search_highlights(
    view: &TerminalPane,
    index: usize,
    position: Point<Pixels>,
    cell: gpui::Size<Pixels>,
    painting: &mut Painting,
) {
    let Some(grid) = view.displayed_grid() else {
        return;
    };
    if let Some(find) = &view.find {
        for (found, current) in find.results.highlights(grid, index) {
            painting.matches.push((
                Bounds::new(
                    position + point(cell.width * f32::from(found.start), px(0.0)),
                    size(cell.width * f32::from(found.end - found.start), cell.height),
                ),
                current,
            ));
        }
    }
}

fn selection_bounds(
    view: &TerminalPane,
    index: usize,
    position: Point<Pixels>,
    cell: gpui::Size<Pixels>,
) -> Option<Bounds<Pixels>> {
    let selection = view.selection?;
    let grid = view.displayed_grid()?;
    let row = isize::try_from(index).ok()? - isize::try_from(grid.history.len()).ok()?;
    let columns = selection.columns(row, grid);
    (!columns.is_empty()).then(|| {
        Bounds::new(
            position + point(cell.width * f32::from(columns.start), px(0.0)),
            size(
                cell.width * f32::from(columns.end - columns.start),
                cell.height,
            ),
        )
    })
}

fn align_to_cells(
    line: &mut WrappedLine,
    columns: &[u16],
    cell_width: Pixels,
    width: u16,
    metrics: (Pixels, Pixels),
) {
    let mut runs = line.runs().to_vec();
    let mut anchors = std::collections::HashMap::new();
    for glyph in runs.iter().flat_map(|run| &run.glyphs) {
        if let Some(column) = columns.get(glyph.index) {
            anchors.entry(*column).or_insert(glyph.position.x);
        }
    }
    for run in &mut runs {
        for glyph in &mut run.glyphs {
            if let Some(column) = columns.get(glyph.index) {
                glyph.position.x =
                    cell_width * f32::from(*column) + glyph.position.x - anchors[column];
            }
        }
    }
    **line = Arc::new(WrappedLineLayout {
        unwrapped_layout: Arc::new(LineLayout {
            font_size: line.font_size(),
            width: cell_width * f32::from(width),
            ascent: metrics.0,
            descent: metrics.1,
            runs,
            len: line.len(),
        }),
        wrap_boundaries: line.wrap_boundaries.clone(),
        wrap_width: None,
    });
}

fn append_run(
    run: &Run,
    column: u16,
    text: &mut String,
    styles: &mut Vec<TextRun>,
    columns: &mut Vec<u16>,
    palette: &Palette,
    base_font: &gpui::Font,
) {
    if run.text.bytes().all(|byte| byte == b' ') {
        return;
    }
    let start = text.len();
    text.push_str(&run.text);
    text.push('\u{200c}');
    let ascii = run.text.is_ascii();
    columns.extend(run.text.bytes().enumerate().map(|(index, _)| {
        let offset = if ascii {
            u16::try_from(index).unwrap_or(u16::MAX)
        } else {
            0
        };
        column.saturating_add(offset)
    }));
    columns.extend(std::iter::repeat_n(column, '\u{200c}'.len_utf8()));
    styles.push(text_run(text.len() - start, run.style, palette, base_font));
}

fn font_parts(run: &Run, settings: &muxy_app_core::settings::FontOptions) -> Option<Vec<Run>> {
    if !run.text.is_ascii() || settings.codepoints.is_empty() {
        return None;
    }
    let family = |ch: char| {
        settings
            .codepoints
            .iter()
            .rev()
            .find(|map| (map.start..=map.end).contains(&u32::from(ch)))
            .map(|map| map.family.as_str())
    };
    let mut parts = Vec::<Run>::new();
    let mut previous = None;
    for ch in run.text.chars() {
        let current = family(ch);
        if current == previous
            && let Some(part) = parts.last_mut()
        {
            part.text.push(ch);
            part.width += 1;
        } else {
            parts.push(Run {
                text: ch.to_string(),
                width: 1,
                style: run.style,
            });
        }
        previous = current;
    }
    Some(parts)
}

fn configure_font(
    font: &mut gpui::Font,
    run: &Run,
    settings: &muxy_app_core::settings::FontOptions,
) {
    let families = match (run.style.bold, run.style.italic) {
        (true, true) => &settings.bold_italic,
        (true, false) => &settings.bold,
        (false, true) => &settings.italic,
        (false, false) => return configure_codepoint(font, run, settings),
    };
    if let Some(family) = families.first() {
        font.family = family.clone().into();
        font.fallbacks = Some(gpui::FontFallbacks::from_fonts(
            families.iter().skip(1).cloned().collect(),
        ));
    }
    configure_codepoint(font, run, settings);
}

fn configure_codepoint(
    font: &mut gpui::Font,
    run: &Run,
    settings: &muxy_app_core::settings::FontOptions,
) {
    if let Some(mapping) = settings.codepoints.iter().rev().find(|mapping| {
        run.text
            .chars()
            .any(|ch| (mapping.start..=mapping.end).contains(&u32::from(ch)))
    }) {
        font.family = mapping.family.clone().into();
    }
}

fn text_run(len: usize, style: Style, palette: &Palette, base_font: &gpui::Font) -> TextRun {
    let mut font = base_font.clone();
    font.weight = if style.bold {
        FontWeight::BOLD
    } else {
        FontWeight::NORMAL
    };
    font.style = if style.italic {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };
    let mut color: Hsla = rgb(palette.style(style).0).into();
    if style.faint {
        color.a = 0.5;
    }
    TextRun {
        len,
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

fn push_quad(quads: &mut Vec<(Bounds<Pixels>, Hsla)>, bounds: Bounds<Pixels>, color: Hsla) {
    if let Some((previous, previous_color)) = quads.last_mut()
        && *previous_color == color
        && previous.origin.y == bounds.origin.y
        && previous.right() == bounds.left()
        && previous.size.height == bounds.size.height
    {
        previous.size.width += bounds.size.width;
    } else {
        quads.push((bounds, color));
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep the terminal paint order explicit"
)]
fn paint(painting: Painting, palette: &Palette, window: &mut Window, cx: &mut App) {
    #[cfg(test)]
    let layered = !painting.unbatched;
    #[cfg(not(test))]
    let layered = true;
    images::paint(&painting.images, i64::MIN..i64::from(i32::MIN / 2), window);
    for row in &painting.rows {
        layers::quads(
            &row.backgrounds,
            painting.background_opacity,
            layered,
            window,
        );
    }
    images::paint(&painting.images, i64::from(i32::MIN / 2)..0, window);
    for (bounds, current) in painting.matches {
        let mut color: Hsla = rgb(palette.indexed(3)).into();
        color.a = if current { 0.65 } else { 0.25 };
        window.paint_quad(fill(bounds, color));
    }
    for (bounds, color) in painting.selections {
        window.paint_quad(fill(bounds, color));
    }
    for &(bounds, color) in painting
        .decorations
        .iter()
        .chain(painting.rows.iter().flat_map(|row| &row.decorations))
    {
        window.paint_quad(fill(bounds, color));
    }
    for row in &painting.rows {
        #[cfg(test)]
        if painting.sequential_quads {
            layers::quads(&row.blocks, 1.0, layered, window);
            continue;
        }
        row.block_layers.paint(&row.blocks, None, layered, window);
    }
    shade::paint(&painting.shade_sprites, window);
    for row in &painting.rows {
        row.paths.paint(None, layered, window);
    }
    if painting.thicken > 0.0 {
        let amount = px(0.5 / window.scale_factor());
        for glyph in painting
            .rows
            .iter()
            .flat_map(|row| &row.glyphs)
            .filter(|glyph| !glyph.emoji)
        {
            let mut color = glyph.color;
            color.a *= painting.thicken * 0.5;
            for offset in [point(-amount, px(0.0)), point(amount, px(0.0))] {
                let _ = window.paint_glyph(
                    glyph.origin + offset,
                    glyph.font,
                    glyph.id,
                    glyph.size,
                    color,
                );
            }
        }
    }
    for (origin, line) in painting.rows.iter().flat_map(|row| &row.lines) {
        if let Err(error) = line.paint(
            *origin,
            painting.cell.height,
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        ) {
            use std::io::Write;
            let _ = writeln!(
                std::io::stderr(),
                "muxy-app: could not paint terminal text: {error}"
            );
        }
    }
    images::paint(&painting.images, 0..i64::MAX, window);
    if let Some(cursor) = painting.cursor {
        let color = painting.cursor_color;
        let natural = if painting.cursor_shape == muxy_protocol::CursorShape::Hollow {
            1.0
        } else {
            1.0 / window.scale_factor()
        };
        let thickness = px(painting
            .cursor_thickness
            .apply(natural, window.scale_factor()))
        .min(cursor.size.width)
        .min(cursor.size.height);
        match painting.cursor_shape {
            muxy_protocol::CursorShape::Bar => window.paint_quad(fill(
                Bounds::new(cursor.origin, size(thickness, cursor.size.height)),
                color,
            )),
            muxy_protocol::CursorShape::Underline => window.paint_quad(fill(
                Bounds::new(
                    point(cursor.left(), cursor.bottom() - thickness),
                    size(cursor.size.width, thickness),
                ),
                color,
            )),
            muxy_protocol::CursorShape::Hollow => {
                let mut quad = gpui::outline(cursor, color, gpui::BorderStyle::Solid);
                quad.border_widths = gpui::Edges::all(thickness);
                window.paint_quad(quad);
            }
            muxy_protocol::CursorShape::Block | muxy_protocol::CursorShape::Unrecognized(_) => {
                window.paint_quad(fill(cursor, color));
                window.with_content_mask(Some(gpui::ContentMask { bounds: cursor }), |window| {
                    for glyph in painting
                        .rows
                        .iter()
                        .flat_map(|row| &row.glyphs)
                        .filter(|glyph| glyph.row_top == cursor.top())
                    {
                        if glyph.emoji {
                            let _ =
                                window.paint_emoji(glyph.origin, glyph.font, glyph.id, glyph.size);
                            continue;
                        }
                        let _ = window.paint_glyph(
                            glyph.origin,
                            glyph.font,
                            glyph.id,
                            glyph.size,
                            painting.cursor_text,
                        );
                    }
                    for row in &painting.rows {
                        row.paths.paint(Some(painting.cursor_text), layered, window);
                    }
                    for row in &painting.rows {
                        #[cfg(test)]
                        if painting.sequential_quads {
                            for &(bounds, _) in &row.blocks {
                                window.paint_quad(fill(bounds, painting.cursor_text));
                            }
                            continue;
                        }
                        row.block_layers.paint(
                            &row.blocks,
                            Some(painting.cursor_text),
                            layered,
                            window,
                        );
                    }
                });
            }
        }
    }
}

fn link_highlight(
    view: &TerminalPane,
    index: usize,
    position: Point<Pixels>,
    cell: gpui::Size<Pixels>,
    palette: &Palette,
    painting: &mut Painting,
) {
    let Some(grid) = view.displayed_grid() else {
        return;
    };
    if view.link_hover.target.is_some()
        && let Some(link) = &view.link_hover.candidate
        && isize::try_from(index)
            .ok()
            .and_then(|index| index.checked_sub(isize::try_from(grid.history.len()).ok()?))
            == Some(link.row)
    {
        painting.decorations.push((
            Bounds::new(
                position
                    + point(
                        cell.width * f32::from(link.columns.start),
                        cell.height - px(2.0),
                    ),
                size(
                    cell.width * f32::from(link.columns.end - link.columns.start),
                    px(1.0),
                ),
            ),
            rgb(palette.foreground).into(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaping_preserves_server_cell_boundaries_and_clusters() {
        for runs in [
            vec![("ل", 1), ("ا", 1), ("x", 1)],
            vec![("👩‍", 2), ("💻", 2), ("x", 1)],
            vec![("👩‍💻", 2), ("x", 1)],
            vec![("❤️", 1), ("x", 1)],
            vec![("❤️", 2), ("x", 1)],
            vec![("e\u{301}", 1), ("x", 1)],
        ] {
            let mut text = String::new();
            let mut styles = Vec::new();
            let mut columns = Vec::new();
            let mut column = 0;
            for (value, width) in runs {
                let start = text.len();
                append_run(
                    &Run {
                        text: value.into(),
                        width,
                        style: Style::default(),
                    },
                    column,
                    &mut text,
                    &mut styles,
                    &mut columns,
                    &Palette::new(true),
                    &font("Menlo"),
                );
                assert_eq!(&text[start..], format!("{value}\u{200c}"));
                assert_eq!(columns[start], column);
                column += width;
            }
            assert_eq!(text.len(), columns.len());
            assert_eq!(styles.iter().map(|run| run.len).sum::<usize>(), text.len());
            let last = text.find('x').map(|index| columns[index]);
            assert_eq!(last, Some(column - 1));
        }
    }
}
