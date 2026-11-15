use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAlert, NSAutoresizingMaskOptions, NSBorderType, NSFont, NSScrollView, NSTextField,
    NSTextView, NSView, NSWindow,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

const ALERT_TEXT_WIDTH: f64 = 230.0;
const BOX_WIDTH: f64 = 380.0;
const BOX_INSET: f64 = 6.0;
const LINE_PADDING: f64 = 10.0;
const BORDER: f64 = 4.0;
const INPUT_GAP: f64 = 8.0;

#[derive(Debug, PartialEq)]
enum Placement {
    Inline,
    Scrolling(f64),
}

fn placement(inline_height: f64, box_height: f64, limit: f64) -> Placement {
    if inline_height <= limit {
        Placement::Inline
    } else {
        Placement::Scrolling(box_height.min(limit).ceil())
    }
}

fn limit(parent: &NSWindow) -> f64 {
    parent.screen().map_or(240.0, |screen| {
        (screen.visibleFrame().size.height * 0.3).clamp(120.0, 320.0)
    })
}

fn font() -> Retained<NSFont> {
    NSFont::systemFontOfSize(NSFont::smallSystemFontSize())
}

fn measure(text: &str, width: f64, main_thread: MainThreadMarker) -> f64 {
    let label = NSTextField::wrappingLabelWithString(&NSString::from_str(text), main_thread);
    label.setFont(Some(&font()));
    label.cell().map_or(f64::MAX, |cell| {
        cell.cellSizeForBounds(NSRect::new(NSPoint::ZERO, NSSize::new(width, f64::MAX)))
            .height
    })
}

pub(super) fn show(
    alert: &NSAlert,
    parent: &NSWindow,
    message: &str,
    input: Option<&NSView>,
    main_thread: MainThreadMarker,
) {
    let text_width = BOX_WIDTH - 2.0 * BOX_INSET;
    let height = match placement(
        measure(message, ALERT_TEXT_WIDTH, main_thread),
        measure(message, text_width - LINE_PADDING, main_thread) + 2.0 * BOX_INSET + BORDER,
        limit(parent),
    ) {
        Placement::Inline => {
            alert.setInformativeText(&NSString::from_str(message));
            if let Some(input) = input {
                alert.setAccessoryView(Some(input));
            }
            return;
        }
        Placement::Scrolling(height) => height,
    };
    alert.setInformativeText(&NSString::from_str(""));
    let scroll = scrolling_text(message, height, main_thread);
    let Some(input) = input else {
        alert.setAccessoryView(Some(&scroll));
        return;
    };
    let below = input.frame().size.height + INPUT_GAP;
    let container = NSView::initWithFrame(
        NSView::alloc(main_thread),
        NSRect::new(NSPoint::ZERO, NSSize::new(BOX_WIDTH, height + below)),
    );
    scroll.setFrameOrigin(NSPoint::new(0.0, below));
    container.addSubview(&scroll);
    container.addSubview(input);
    alert.setAccessoryView(Some(&container));
}

fn scrolling_text(message: &str, height: f64, main_thread: MainThreadMarker) -> Retained<NSView> {
    let scroll = NSScrollView::initWithFrame(
        NSScrollView::alloc(main_thread),
        NSRect::new(NSPoint::ZERO, NSSize::new(BOX_WIDTH, height)),
    );
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(false);
    scroll.setAutohidesScrollers(true);
    scroll.setBorderType(NSBorderType::BezelBorder);
    scroll.setDrawsBackground(false);
    let size = scroll.contentSize();
    let text = NSTextView::initWithFrame(
        NSTextView::alloc(main_thread),
        NSRect::new(NSPoint::ZERO, size),
    );
    text.setRichText(false);
    text.setEditable(false);
    text.setSelectable(false);
    text.setDrawsBackground(false);
    text.setFont(Some(&font()));
    text.setTextContainerInset(NSSize::new(BOX_INSET, BOX_INSET));
    text.setMinSize(NSSize::new(0.0, size.height));
    text.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
    text.setVerticallyResizable(true);
    text.setHorizontallyResizable(false);
    text.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    text.setString(&NSString::from_str(message));
    scroll.setDocumentView(Some(&text));
    Retained::into_super(scroll)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_taller_than_the_limit_scrolls_in_a_box_no_taller_than_it() {
        assert_eq!(placement(80.0, 60.0, 240.0), Placement::Inline);
        assert_eq!(placement(240.0, 180.0, 240.0), Placement::Inline);
        assert_eq!(placement(400.0, 180.4, 240.0), Placement::Scrolling(181.0));
        assert_eq!(
            placement(9000.0, 6000.0, 240.0),
            Placement::Scrolling(240.0)
        );
    }
}
