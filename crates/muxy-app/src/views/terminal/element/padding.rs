use gpui::{Bounds, Hsla, Pixels, point, px, rgb, size};
use muxy_protocol::{Run, Size};

use super::{Palette, TerminalPane, push_quad, viewport_size};

#[derive(Clone, Copy)]
pub(super) struct Frame {
    outer: Bounds<Pixels>,
    pub(super) content: Bounds<Pixels>,
    pub(super) grid: Bounds<Pixels>,
    pub(super) viewport: Size,
    cell: gpui::Size<Pixels>,
    color: muxy_app_core::settings::PaddingColor,
}

impl Frame {
    pub(super) fn configured(
        outer: Bounds<Pixels>,
        cell: gpui::Size<Pixels>,
        options: &muxy_app_core::settings::TerminalOptions,
    ) -> Self {
        let mut content = Bounds::new(
            outer.origin + point(px(options.padding_x[0]), px(options.padding_y[0])),
            size(
                (outer.size.width - px(options.padding_x.iter().sum())).max(px(0.0)),
                (outer.size.height - px(options.padding_y.iter().sum())).max(px(0.0)),
            ),
        );
        let viewport = viewport_size(content.size, cell);
        if options.padding_balance {
            let remainder = size(
                (content.size.width - cell.width * f32::from(viewport.cols)).max(px(0.0)),
                (content.size.height - cell.height * f32::from(viewport.rows)).max(px(0.0)),
            );
            content.origin += point(remainder.width / 2.0, remainder.height / 2.0);
            content.size = content.size - remainder;
        }
        Self {
            color: options.padding_color,
            outer,
            content,
            grid: Bounds::new(
                content.origin,
                size(
                    cell.width * f32::from(viewport.cols),
                    cell.height * f32::from(viewport.rows),
                ),
            ),
            viewport,
            cell,
        }
    }

    pub(super) fn backgrounds(
        self,
        view: &TerminalPane,
        palette: &Palette,
    ) -> Vec<(Bounds<Pixels>, Hsla)> {
        let mut quads = Vec::new();
        if self.color == muxy_app_core::settings::PaddingColor::Background {
            return quads;
        }
        let Some(grid) = view.displayed_grid() else {
            return quads;
        };
        let start = view.visible_start(grid);
        let remainder = view.scroll.pixel_remainder(f32::from(self.cell.height));
        for row in 0..=self.viewport.rows {
            if let Some(runs) = grid.content_row(start + usize::from(row)) {
                self.extend_row(runs, row, remainder, palette, &mut quads);
            }
        }
        quads
    }

    fn extend_row(
        self,
        runs: &[Run],
        row: u16,
        remainder: f32,
        palette: &Palette,
        quads: &mut Vec<(Bounds<Pixels>, Hsla)>,
    ) {
        let top = self.grid.top() + self.cell.height * f32::from(row) + px(remainder);
        let bottom = top + self.cell.height;
        if top >= self.grid.bottom() || bottom <= self.grid.top() {
            return;
        }
        let vertical = (top <= self.grid.top() || bottom >= self.grid.bottom())
            && (self.color == muxy_app_core::settings::PaddingColor::ExtendAlways
                || extend_vertical(runs, self.viewport.cols, palette));
        let mut column = 0_u16;
        for run in runs {
            let end = column.saturating_add(run.width).min(self.viewport.cols);
            if end <= column {
                continue;
            }
            let (foreground, background) = palette.style(run.style);
            let color = if run.text.starts_with('█') {
                foreground
            } else {
                background
            };
            if color != palette.background && (vertical || column == 0 || end == self.viewport.cols)
            {
                let left = if column == 0 {
                    self.outer.left()
                } else {
                    self.grid.left() + self.cell.width * f32::from(column)
                };
                let right = if end == self.viewport.cols {
                    self.outer.right()
                } else {
                    self.grid.left() + self.cell.width * f32::from(end)
                };
                let row_top = top.max(self.grid.top());
                let row_bottom = bottom.min(self.grid.bottom());
                for bounds in [
                    rect(left, row_top, self.grid.left().min(right), row_bottom),
                    rect(self.grid.right().max(left), row_top, right, row_bottom),
                    rect(
                        left,
                        self.outer.top(),
                        right,
                        if vertical && top <= self.grid.top() {
                            self.grid.top()
                        } else {
                            self.outer.top()
                        },
                    ),
                    rect(
                        left,
                        if vertical && bottom >= self.grid.bottom() {
                            self.grid.bottom()
                        } else {
                            self.outer.bottom()
                        },
                        right,
                        self.outer.bottom(),
                    ),
                ] {
                    let bounds = bounds.intersect(&self.outer);
                    if bounds.size.width > px(0.0) && bounds.size.height > px(0.0) {
                        push_quad(quads, bounds, rgb(color).into());
                    }
                }
            }
            column = end;
            if column == self.viewport.cols {
                break;
            }
        }
    }
}

fn rect(left: Pixels, top: Pixels, right: Pixels, bottom: Pixels) -> Bounds<Pixels> {
    Bounds::new(point(left, top), size(right - left, bottom - top))
}

fn extend_vertical(runs: &[Run], cols: u16, palette: &Palette) -> bool {
    let mut width = 0_u16;
    for run in runs {
        if run.width == 0 {
            continue;
        }
        if palette.resolve(run.style.bg, palette.background) == palette.background
            || has_powerline(run)
        {
            return false;
        }
        width = width.saturating_add(run.width);
        if width >= cols {
            return true;
        }
    }
    false
}

fn has_powerline(run: &Run) -> bool {
    run.text.chars().any(|character| {
        matches!(character, '\u{e0b0}'..='\u{e0c8}' | '\u{e0ca}' | '\u{e0cc}'..='\u{e0d2}' | '\u{e0d4}')
    })
}

/// The background a full-screen program paints its screen in: the one most of
/// its rows start and end in, when that isn't the terminal's own. Edges, not
/// area, because panels and messages can cover most of an app's middle.
pub(super) fn screen_background<'a>(
    rows: impl IntoIterator<Item = &'a [Run]>,
    size: Size,
    palette: &Palette,
) -> Option<u32> {
    let mut edges: Vec<(u32, u32)> = Vec::new();
    let mut samples = 0_u32;
    for runs in rows.into_iter().take(usize::from(size.rows)) {
        for column in [0, size.cols.saturating_sub(1)] {
            let color = background_at(runs, column, palette);
            match edges.iter_mut().find(|(candidate, _)| *candidate == color) {
                Some((_, count)) => *count += 1,
                None => edges.push((color, 1)),
            }
            samples += 1;
        }
    }
    edges
        .into_iter()
        .find(|&(color, count)| count * 2 > samples && color != palette.background)
        .map(|(color, _)| color)
}

fn background_at(runs: &[Run], column: u16, palette: &Palette) -> u32 {
    let mut start = 0_u16;
    for run in runs {
        let end = start.saturating_add(run.width);
        if column < end {
            return palette.style(run.style).1;
        }
        start = end;
    }
    palette.background
}

#[cfg(test)]
mod tests {
    use muxy_protocol::{Color, Style};

    use super::*;

    fn run(text: &str, style: Style) -> Run {
        Run {
            text: text.into(),
            width: u16::try_from(text.chars().count()).unwrap_or(u16::MAX),
            style,
        }
    }

    fn on(bg: Color) -> Style {
        Style {
            bg,
            ..Style::default()
        }
    }

    fn background(rows: &[Vec<Run>], cols: u16) -> Option<u32> {
        let size = Size {
            cols,
            rows: u16::try_from(rows.len()).unwrap_or(u16::MAX),
        };
        screen_background(rows.iter().map(Vec::as_slice), size, &Palette::new(true))
    }

    #[test]
    fn a_full_screen_app_gives_the_color_its_rows_start_and_end_in() {
        let main = on(Color::Rgb(0x26, 0x26, 0x24));
        let panel = on(Color::Rgb(0x1e, 0x1e, 0x1e));
        let rows = [
            vec![run(" ", main), run(" Greeting     ", panel), run(" ", main)],
            vec![run(" ", main), run("  Hi! a long  ", panel), run(" ", main)],
            vec![run(" ", main), run("  message box ", panel), run(" ", main)],
            vec![run("                ", main)],
        ];
        assert_eq!(background(&rows, 16), Some(0x26_26_24));
    }

    #[test]
    fn inverse_text_counts_its_foreground_as_the_background() {
        let inverse = Style {
            fg: Color::Rgb(0xaa, 0xbb, 0xcc),
            inverse: true,
            ..Style::default()
        };
        assert_eq!(background(&[vec![run("  ", inverse)]], 2), Some(0xaa_bb_cc));
        assert_eq!(
            background(&[vec![run("    ", on(Color::Indexed(4)))]], 4),
            Some(Palette::new(true).indexed(4))
        );
    }

    #[test]
    fn a_screen_whose_edges_are_the_terminal_background_keeps_it() {
        let blue = on(Color::Rgb(0x10, 0x20, 0x30));
        let red = on(Color::Rgb(0x90, 0x10, 0x10));
        for rows in [
            vec![vec![run("$ ls", Style::default())], Vec::new()],
            vec![vec![run("    ", blue)], Vec::new()],
            vec![vec![run("  ", blue), run("  ", Style::default())]],
            vec![vec![run("  ", blue), run("  ", red)]],
            vec![vec![
                run(" ", Style::default()),
                run("  ", blue),
                run(" ", Style::default()),
            ]],
            Vec::new(),
        ] {
            assert_eq!(background(&rows, 4), None, "{rows:?}");
        }
    }

    #[test]
    fn runs_wider_than_the_screen_count_only_what_shows() {
        let blue = on(Color::Rgb(0x10, 0x20, 0x30));
        let red = on(Color::Rgb(0x90, 0x10, 0x10));
        let rows = [vec![run("  ", blue), run("  ", red)]];
        assert_eq!(background(&rows, 2), Some(0x10_20_30));
    }
}
