use super::*;
use crate::views::terminal::colors::Palette;
use crate::views::terminal::element::{Painting, RowPainting, RowRenderer, glyph, paint};
use gpui::{
    AnyView, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, canvas, div,
    font, point, px, rgb, size,
};
use muxy_protocol::{Color, Run, Style};
use std::rc::Rc;

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

#[test]
fn overlapping_quads_split_layers_and_preserve_order() {
    let color = rgb(0x12_3456).into();
    let quads = [
        (bounds(0.0, 0.0, 8.0, 16.0), color),
        (bounds(8.0, 0.0, 8.0, 16.0), color),
        (bounds(4.0, 0.0, 8.0, 16.0), color),
        (bounds(16.0, 0.0, 8.0, 16.0), color),
    ];
    assert_eq!(disjoint_prefix(&quads), (2, bounds(0.0, 0.0, 16.0, 16.0)));
    assert_eq!(
        disjoint_prefix(&quads[2..]),
        (2, bounds(4.0, 0.0, 20.0, 16.0))
    );
}

#[test]
fn braille_batches_keep_every_dot_and_color_across_adjacent_cells() {
    let mut paths = Paths {
        legacy_dots: true,
        ..Paths::default()
    };
    let mut quads = Vec::new();
    for column in 0..120_u16 {
        let mut color: Hsla = rgb(u32::from(column) * 100).into();
        color.a = 0.5;
        assert!(glyph::prepare(
            "⣿",
            bounds(f32::from(column) * 8.0, 0.0, 8.0, 16.0),
            color,
            2.0,
            &mut quads,
            &mut paths,
        ));
    }
    assert!(quads.is_empty());
    assert_eq!(paths.groups.len(), 1);
    assert_eq!(paths.groups[0].bounds, Some(bounds(0.0, 0.0, 960.0, 16.0)));
    assert_eq!(vector_paths(&paths.groups[0]).len(), 960);
    for (column, dots) in vector_paths(&paths.groups[0]).chunks_exact(8).enumerate() {
        let mut color: Hsla = rgb(u32::try_from(column).unwrap_or(0) * 100).into();
        color.a = 0.5;
        assert!(dots.iter().all(|(_, actual)| *actual == color));
    }
}

#[test]
fn paths_that_may_overlap_never_share_a_layer() {
    let mut paths = Paths {
        legacy_dots: true,
        ..Paths::default()
    };
    let mut quads = Vec::new();
    let color = rgb(0xff_ffff).into();
    for text in ["⣿", "╭", "⣿", "⣿"] {
        glyph::prepare(
            text,
            bounds(0.0, 0.0, 8.0, 16.0),
            color,
            2.0,
            &mut quads,
            &mut paths,
        );
    }
    assert_eq!(paths.groups.len(), 4);
    assert!(paths.groups[1].bounds.is_none());
    let mut cramped = Paths {
        legacy_dots: true,
        ..Paths::default()
    };
    glyph::prepare(
        "⣿",
        bounds(0.0, 0.0, 8.0, 4.0),
        color,
        1.0,
        &mut quads,
        &mut cramped,
    );
    assert_eq!(cramped.groups.len(), 8);
    assert!(cramped.groups.iter().all(|group| group.bounds.is_none()));
}

struct Chart {
    rows: Vec<Rc<RowPainting>>,
    unbatched: bool,
    renders: usize,
}

impl Render for Chart {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let rows = self.rows.clone();
        let unbatched = self.unbatched;
        let sequential_quads =
            std::env::var("MUXY_BENCH_LEGACY_QUADS").is_ok_and(|value| value == "1");
        canvas(
            move |_, _, _| Painting {
                rows,
                unbatched,
                sequential_quads,
                cell: size(px(8.0), px(16.0)),
                background_opacity: 0.5,
                selections: vec![(bounds(0.0, 0.0, 24.0, 16.0), gpui::blue())],
                cursor: Some(bounds(0.0, 0.0, 8.0, 16.0)),
                cursor_color: gpui::red(),
                cursor_text: gpui::white(),
                ..Painting::default()
            },
            |_, painting, window, cx| paint(painting, &Palette::new(true), window, cx),
        )
        .size_full()
    }
}

struct Host(Entity<Chart>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(AnyView::from(self.0.clone()).cached(div().size_full().style().clone()))
    }
}

fn chart(rows: u16, cols: u16, window: &mut Window) -> Chart {
    chart_with_text(rows, cols, window, None)
}

fn chart_with_text(rows: u16, cols: u16, window: &mut Window, text: Option<&str>) -> Chart {
    let palette = Palette::new(true);
    let settings = muxy_app_core::settings::FontOptions::default();
    let base_font = font("Menlo");
    let mut renderer = RowRenderer {
        settings: &settings,
        cell: size(px(8.0), px(16.0)),
        palette: &palette,
        base_font: &base_font,
        font_size: px(13.0),
        metrics: (px(12.0), px(4.0)),
        window,
    };
    Chart {
        rows: (0..rows)
            .map(|row| {
                let runs: Vec<_> = (0..cols)
                    .map(|col| Run {
                        text: text.unwrap_or(if row % 3 == 0 { "█" } else { "⣿" }).into(),
                        width: 1,
                        style: Style {
                            fg: Color::Indexed(u8::try_from(col % 16).unwrap_or(0)),
                            bg: Color::Indexed(u8::try_from((col + 1) % 16).unwrap_or(0)),
                            faint: col % 2 == 0,
                            ..Style::default()
                        },
                    })
                    .collect();
                let mut painting = RowPainting::default();
                painting.paths.legacy_dots =
                    std::env::var("MUXY_BENCH_LEGACY_DOTS").is_ok_and(|value| value == "1");
                renderer.row(
                    &runs,
                    point(px(0.0), px(f32::from(row) * 16.0)),
                    &mut painting,
                    false,
                );
                Rc::new(painting)
            })
            .collect(),
        unbatched: false,
        renders: 0,
    }
}

#[gpui::test]
fn layered_terminal_scene_survives_replay_and_cursor_clipping(cx: &mut gpui::TestAppContext) {
    let (host, cx) = cx.add_window_view(|window, cx| Host(cx.new(|_| chart(4, 16, window))));
    cx.run_until_parked();
    let chart = host.read_with(cx, |host, _| host.0.clone());
    let renders = chart.read_with(cx, |chart, _| chart.renders);
    assert!(renders > 0);
    for _ in 0..5 {
        host.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert_eq!(chart.read_with(cx, |chart, _| chart.renders), renders);
    }
    chart.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(chart.read_with(cx, |chart, _| chart.renders) > renders);
}

#[gpui::test]
#[ignore = "manual headless benchmark of terminal painting and GPUI scene replay"]
fn terminal_scene_paint_and_replay_benchmark(cx: &mut gpui::TestAppContext) {
    use std::io::Write;
    let (host, cx) = cx.add_window_view(|window, cx| Host(cx.new(|_| chart(40, 120, window))));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    cx.run_until_parked();
    let chart = host.read_with(cx, |host, _| host.0.clone());
    for unbatched in [true, false] {
        chart.update(cx, |chart, cx| {
            chart.unbatched = unbatched;
            cx.notify();
        });
        cx.run_until_parked();
        for replay in [false, true] {
            let renders = chart.read_with(cx, |chart, _| chart.renders);
            let started = std::time::Instant::now();
            for _ in 0..20 {
                if replay {
                    host.update(cx, |_, cx| cx.notify());
                } else {
                    chart.update(cx, |_, cx| cx.notify());
                }
                cx.run_until_parked();
            }
            let elapsed = started.elapsed();
            let rendered = chart.read_with(cx, |chart, _| chart.renders) - renders;
            assert_eq!(rendered, if replay { 0 } else { 20 });
            let _ = writeln!(
                std::io::stderr(),
                "40x120 shapes, 20 frames, layered={}, replay={replay}: {elapsed:?}",
                !unbatched,
            );
        }
    }
}

#[gpui::test]
#[ignore = "manual headless matrix of redraw costs for different visible terminal content"]
fn terminal_redraw_matrix(cx: &mut gpui::TestAppContext) {
    use std::io::Write;
    for text in ["x", "┼", "█", "⣿"] {
        for rows in [10, 40] {
            let (host, cx) = cx.add_window_view(|window, cx| {
                Host(cx.new(|_| chart_with_text(rows, 120, window, Some(text))))
            });
            cx.simulate_resize(size(px(1000.0), px(700.0)));
            cx.run_until_parked();
            let chart = host.read_with(cx, |host, _| host.0.clone());
            for replay in [false, true] {
                let renders = chart.read_with(cx, |chart, _| chart.renders);
                let started = std::time::Instant::now();
                for _ in 0..3 {
                    if replay {
                        host.update(cx, |_, cx| cx.notify());
                    } else {
                        chart.update(cx, |_, cx| cx.notify());
                    }
                    cx.run_until_parked();
                }
                let elapsed = started.elapsed();
                let rendered = chart.read_with(cx, |chart, _| chart.renders) - renders;
                assert_eq!(rendered, if replay { 0 } else { 3 });
                let _ = writeln!(
                    std::io::stderr(),
                    "redraw text={text:?} rows={rows} cols=120 replay={replay} frames=3 ms_per_frame={:.3}",
                    elapsed.as_secs_f64() * 1000.0 / 3.0
                );
            }
        }
    }
}

fn assert_quad_order(layers: &QuadLayers, quads: &[(Bounds<Pixels>, Hsla)]) {
    let mut positions = vec![usize::MAX; quads.len()];
    let mut position = 0;
    for layer in &layers.layers {
        for (offset, &index) in layer.indices.iter().enumerate() {
            assert_eq!(positions[index], usize::MAX);
            positions[index] = position;
            position += 1;
            assert_eq!(layer.bounds.union(&quads[index].0), layer.bounds);
            for &other in &layer.indices[..offset] {
                assert!(!quads[index].0.intersects(&quads[other].0));
            }
        }
    }
    assert_eq!(position, quads.len());
    for (index, &(bounds, _)) in quads.iter().enumerate() {
        for (other, &(next, _)) in quads.iter().enumerate().skip(index + 1) {
            if bounds.intersects(&next) {
                assert!(positions[index] < positions[other]);
            }
        }
    }
}

#[test]
fn box_drawing_layers_preserve_every_stroke_and_overlap_order() {
    for scale in [1.0, 1.5, 2.0] {
        let mut layers = QuadLayers::default();
        let mut quads = Vec::new();
        let mut paths = Paths {
            legacy_dots: true,
            ..Paths::default()
        };
        for (column, cp) in (0_u16..).zip(0x2500..=0x257f) {
            let mut color: Hsla = rgb(cp * 100).into();
            color.a = 0.5;
            let start = quads.len();
            assert!(glyph::prepare(
                &char::from_u32(cp).unwrap_or(' ').to_string(),
                bounds(0.25 + f32::from(column) * 7.75, 0.25, 7.75, 15.5),
                color,
                scale,
                &mut quads,
                &mut paths,
            ));
            layers.append_cell(&quads, start);
        }
        assert_quad_order(&layers, &quads);
    }
}

#[test]
fn adjacent_crosses_share_four_layers() {
    let mut layers = QuadLayers::default();
    let mut quads = Vec::new();
    let mut paths = Paths {
        legacy_dots: true,
        ..Paths::default()
    };
    for column in 0..120_u16 {
        let start = quads.len();
        glyph::prepare(
            "┼",
            bounds(f32::from(column) * 8.0, 0.0, 8.0, 16.0),
            gpui::white(),
            2.0,
            &mut quads,
            &mut paths,
        );
        layers.append_cell(&quads, start);
    }
    assert_eq!(quads.len(), 480);
    assert_eq!(layers.layers.len(), 4);
    assert_quad_order(&layers, &quads);
}

#[test]
fn overlapping_cells_keep_the_original_order() {
    let mut layers = QuadLayers::default();
    let mut quads = Vec::new();
    layers.append_cell(&quads, 0);
    assert!(layers.layers.is_empty());
    for x in [0.0, 8.0, 4.0, 16.0] {
        let start = quads.len();
        quads.extend([
            (bounds(x, 0.0, 8.0, 16.0), gpui::red()),
            (bounds(x + 2.0, 2.0, 4.0, 12.0), gpui::blue()),
        ]);
        layers.append_cell(&quads, start);
    }
    assert_eq!(layers.layers.len(), 4);
    assert_quad_order(&layers, &quads);
}

fn vector_paths(group: &PathGroup) -> &[(Path<Pixels>, Hsla)] {
    match &group.shapes {
        Shapes::Paths(paths) => paths,
        Shapes::Dots(_) => &[],
    }
}

#[gpui::test]
#[ignore = "manual headless benchmark of terminal row preparation"]
fn terminal_prepare_matrix(cx: &mut gpui::TestAppContext) {
    use std::io::Write;
    let (_, cx) = cx.add_window_view(|window, cx| Host(cx.new(|_| chart(1, 1, window))));
    cx.run_until_parked();
    for text in ["x", "┼", "█", "⣿"] {
        cx.update(|window, _| {
            let started = std::time::Instant::now();
            for _ in 0..5 {
                std::hint::black_box(chart_with_text(40, 120, window, Some(text)));
            }
            let _ = writeln!(
                std::io::stderr(),
                "prepare text={text:?} rows=40 cols=120 frames=5 ms_per_frame={:.3}",
                started.elapsed().as_secs_f64() * 1000.0 / 5.0
            );
        });
    }
}

#[gpui::test]
fn circles_survive_scene_replay_and_cursor_clipping(cx: &mut gpui::TestAppContext) {
    let (host, cx) = cx
        .add_window_view(|window, cx| Host(cx.new(|_| chart_with_text(4, 16, window, Some("⣿")))));
    cx.run_until_parked();
    let chart = host.read_with(cx, |host, _| host.0.clone());
    let renders = chart.read_with(cx, |chart, _| chart.renders);
    for _ in 0..3 {
        host.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert_eq!(chart.read_with(cx, |chart, _| chart.renders), renders);
    chart.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(chart.read_with(cx, |chart, _| chart.renders) > renders);
}
