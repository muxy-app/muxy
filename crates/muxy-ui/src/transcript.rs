use gpui::{
    App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, SharedString, Style, TextAlign, TextRun, TextStyle, Window,
    WrappedLine, px, size,
};
use std::{cell::RefCell, rc::Rc};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct Transcript {
    text: SharedString,
    maximum_lines: usize,
}

impl Transcript {
    pub fn new(text: impl Into<SharedString>, maximum_lines: usize) -> Self {
        Self {
            text: text.into(),
            maximum_lines: maximum_lines.max(1),
        }
    }
}

#[derive(Debug, Default)]
pub struct Layout {
    lines: Vec<WrappedLine>,
    line_height: Pixels,
}

fn shape(
    text: SharedString,
    width: Pixels,
    font_size: Pixels,
    style: &TextStyle,
    window: &Window,
) -> Vec<WrappedLine> {
    let run: TextRun = style.to_run(text.len());
    window
        .text_system()
        .shape_text(text, font_size, &[run], Some(width), None)
        .unwrap_or_default()
        .into_vec()
}

fn fits(lines: &[WrappedLine], width: Pixels, maximum: usize) -> bool {
    lines
        .iter()
        .map(|line| line.wrap_boundaries.len() + 1)
        .sum::<usize>()
        <= maximum
        && lines.iter().all(|line| line.size(px(1.0)).width <= width)
}

fn tail(
    text: &SharedString,
    width: Pixels,
    maximum: usize,
    font_size: Pixels,
    style: &TextStyle,
    window: &Window,
) -> Vec<WrappedLine> {
    let lines = shape(text.clone(), width, font_size, style, window);
    if fits(&lines, width, maximum) {
        return lines;
    }
    let boundaries: Vec<_> = text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .collect();
    let mut start = 0;
    let mut end = boundaries.len() - 1;
    while start < end {
        let middle = start + (end - start) / 2;
        let candidate = shape(
            format!("…{}", &text[boundaries[middle]..]).into(),
            width,
            font_size,
            style,
            window,
        );
        if fits(&candidate, width, maximum) {
            end = middle;
        } else {
            start = middle + 1;
        }
    }
    shape(
        format!("…{}", &text[boundaries[start]..]).into(),
        width,
        font_size,
        style,
        window,
    )
}

impl IntoElement for Transcript {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Transcript {
    type RequestLayoutState = Rc<RefCell<Layout>>;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout = Rc::new(RefCell::new(Layout::default()));
        let result = layout.clone();
        let text = self.text.clone();
        let maximum = self.maximum_lines;
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let id =
            window.request_measured_layout(Style::default(), move |known, available, window, _| {
                let width = known
                    .width
                    .or(match available.width {
                        AvailableSpace::Definite(width) => Some(width),
                        _ => None,
                    })
                    .unwrap_or(px(4096.0))
                    .max(px(1.0));
                let lines = tail(&text, width, maximum, font_size, &style, window);
                let mut dimensions = size(px(0.0), px(0.0));
                for line in &lines {
                    let measured = line.size(line_height);
                    dimensions.width = dimensions.width.max(measured.width);
                    dimensions.height += measured.height;
                }
                *result.borrow_mut() = Layout { lines, line_height };
                dimensions
            });
        (id, layout)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = layout.borrow();
        let mut origin = bounds.origin;
        for line in &layout.lines {
            let _ = line.paint(
                origin,
                layout.line_height,
                TextAlign::Left,
                Some(bounds),
                window,
                cx,
            );
            origin.y += line.size(layout.line_height).height;
        }
    }
}
