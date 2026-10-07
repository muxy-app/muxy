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
            || run.text.chars().any(|character| {
                matches!(character, '\u{e0b0}'..='\u{e0c8}' | '\u{e0ca}' | '\u{e0cc}'..='\u{e0d2}' | '\u{e0d4}')
            })
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
