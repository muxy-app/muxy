use super::*;
use crate::views::terminal::colors::Palette;
use crate::views::terminal::element::{Painting, RowPainting, RowRenderer, paint};
use gpui::{
    AnyView, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, canvas, div,
    font, point, px, size,
};
use muxy_protocol::{Color, Run, Style};
use std::rc::Rc;

#[gpui::test]
fn powerline_caps_use_cell_drawing_unless_a_font_is_explicitly_mapped(
    cx: &mut gpui::TestAppContext,
) {
    use muxy_app_core::settings::{CellHeight, FontMap, TerminalSettings};

    let mut terminal = TerminalSettings {
        cell_height: CellHeight::Percent(25.0),
        ..TerminalSettings::default()
    };
    terminal.options.padding_balance = true;
    let palette = Palette::new(true);
    let base_font = font("Menlo");
    let runs: Vec<_> = [
        "\u{e0b6}", "prompt", "\u{e0b0}", "\u{f179}", "\u{e0b4}", "\u{e0b5}", "\u{e0b7}",
    ]
    .into_iter()
    .map(|text| Run {
        text: text.into(),
        width: if text == "prompt" { 6 } else { 1 },
        style: Style::default(),
    })
    .collect();
    cx.add_empty_window().update(|window, _| {
        let cell = size(
            px(8.0),
            px(terminal.cell_height.apply(16.0, window.scale_factor())),
        );
        let frame = super::super::padding::Frame::configured(
            bounds(0.0, 0.0, 101.0, 51.0),
            cell,
            &terminal.options,
        );
        for mapped in [false, true] {
            if mapped {
                terminal.font.codepoints.push(FontMap {
                    start: 0xe0b4,
                    end: 0xe0b7,
                    family: "Menlo".into(),
                });
            }
            let mut renderer = RowRenderer {
                settings: &terminal.font,
                cell,
                palette: &palette,
                base_font: &base_font,
                font_size: px(13.0),
                metrics: (px(12.0), px(4.0)),
                window,
            };
            let mut painting = RowPainting::default();
            renderer.row(&runs, frame.content.origin, &mut painting, true);
            let text = painting.lines.iter().map(|(_, line)| line.text.as_ref()).collect::<String>();
            let expected = if mapped {
                "\u{e0b6}\u{200c}prompt\u{200c}\u{f179}\u{200c}\u{e0b4}\u{200c}\u{e0b5}\u{200c}\u{e0b7}\u{200c}"
            } else {
                "prompt\u{200c}\u{f179}\u{200c}"
            };
            assert_eq!(text, expected, "mapped={mapped}");
            assert_eq!(painting.paths.groups.len(), if mapped { 1 } else { 5 });
        }
    });
}

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
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
