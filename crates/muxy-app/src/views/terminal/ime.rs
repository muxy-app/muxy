use std::ops::Range;

use gpui::{
    App, Bounds, Context, Entity, EntityInputHandler, Pixels, Point, TextRun, UTF16Selection,
    Window, fill, font, point, px, rgb, size,
};

use super::pane::{PaneState, TerminalPane};

#[derive(Default)]
pub(crate) struct Composition {
    pub(crate) text: String,
    selection: Range<usize>,
}

impl Composition {
    fn range(&self, range: Range<usize>) -> Range<usize> {
        let start = utf8_offset(&self.text, range.start);
        start..utf8_offset(&self.text, range.end).max(start)
    }

    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> usize {
        let range = range.map_or(0..self.text.len(), |range| self.range(range));
        let start = self.text[..range.start].encode_utf16().count();
        self.text.replace_range(range, text);
        start
    }
}

fn utf8_offset(text: &str, offset: usize) -> usize {
    let mut utf16 = 0;
    for (index, ch) in text.char_indices() {
        if utf16 >= offset {
            return index;
        }
        utf16 += ch.len_utf16();
    }
    text.len()
}

impl EntityInputHandler for TerminalPane {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.composition.range(range);
        *actual = Some(
            self.composition.text[..range.start].encode_utf16().count()
                ..self.composition.text[..range.end].encode_utf16().count(),
        );
        Some(self.composition.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        (self.state == PaneState::Live).then(|| UTF16Selection {
            range: self.composition.selection.clone(),
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.composition.text.is_empty()).then(|| 0..self.composition.text.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let composition = std::mem::take(&mut self.composition);
        self.send_paste(composition.text.as_bytes(), cx);
        if !composition.text.is_empty() {
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut composition = std::mem::take(&mut self.composition);
        let had_composition = !composition.text.is_empty();
        composition.replace(range, text);
        self.send_paste(composition.text.as_bytes(), cx);
        if had_composition {
            cx.notify();
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state != PaneState::Live {
            return;
        }
        let start = self.composition.replace(range, text);
        let length = text.encode_utf16().count();
        self.composition.selection = selection.map_or(start + length..start + length, |range| {
            start + range.start.min(length)
                ..start + range.end.min(length).max(range.start.min(length))
        });
        self.scroll_to_bottom(cx);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.composition_bounds()
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.composition_bounds()?
            .contains(&position)
            .then_some(self.composition.selection.start)
    }
}

impl TerminalPane {
    fn composition_bounds(&self) -> Option<Bounds<Pixels>> {
        let (bounds, cell) = self.geometry?;
        let grid = self.grid.as_ref()?;
        Some(Bounds::new(
            bounds.origin
                + point(
                    cell.width * f32::from(grid.cursor.col.min(grid.size.cols.saturating_sub(1))),
                    cell.height * f32::from(grid.cursor.row.min(grid.size.rows.saturating_sub(1))),
                ),
            cell,
        ))
    }
}

pub(super) fn paint(view: &Entity<TerminalPane>, window: &mut Window, cx: &mut App) {
    let pane = view.read(cx);
    if pane.composition.text.is_empty() || !pane.focus.is_focused(window) {
        return;
    }
    let Some(mut bounds) = pane.composition_bounds() else {
        return;
    };
    let mut base_font = font(
        pane.terminal
            .font_families
            .first()
            .cloned()
            .unwrap_or_else(|| "Menlo".into()),
    );
    base_font.fallbacks = Some(gpui::FontFallbacks::from_fonts(
        pane.terminal
            .font_families
            .iter()
            .skip(1)
            .cloned()
            .collect(),
    ));
    let foreground = rgb(pane.palette.foreground);
    let background = rgb(pane.palette.background);
    let line = window.text_system().shape_line(
        pane.composition.text.clone().into(),
        px(pane.terminal.font_size),
        &[TextRun {
            len: pane.composition.text.len(),
            font: base_font,
            color: foreground.into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    );
    bounds.size.width = line.width.max(bounds.size.width);
    window.paint_quad(fill(bounds, background));
    let _ = line.paint(bounds.origin, bounds.size.height, window, cx);
    window.paint_quad(fill(
        Bounds::new(
            point(bounds.left(), bounds.bottom() - px(1.0)),
            size(bounds.size.width, px(1.0)),
        ),
        foreground,
    ));
}
