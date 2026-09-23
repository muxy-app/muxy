use std::cell::Cell;
use std::io;
use std::rc::Rc;

use block2::RcBlock;
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua,
    NSAppearanceNameDarkAqua, NSAutoresizingMaskOptions, NSBackingStoreType, NSBezelStyle,
    NSBorderType, NSButton, NSButtonCell, NSColor, NSEvent, NSEventModifierFlags, NSFont,
    NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold, NSModalResponse,
    NSModalResponseCancel, NSModalResponseOK, NSPanel, NSScrollView, NSTextAlignment,
    NSTextDelegate, NSTextField, NSTextView, NSTextViewDelegate, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString,
};

use super::parent_window;
use crate::controls::Style;
use crate::theme::Metrics;

pub const ADDITIONAL_PROMPT_LIMIT: usize = 2000;

/// A native prompt sheet. Dropping it dismisses the sheet as a cancellation.
#[derive(Debug)]
pub struct PromptConfirmation {
    panel: Retained<PromptPanel>,
    parent: Retained<NSWindow>,
    completed: Rc<Cell<bool>>,
}

impl Drop for PromptConfirmation {
    fn drop(&mut self) {
        if !self.completed.replace(true) {
            self.parent
                .endSheet_returnCode(&self.panel, NSModalResponseCancel);
        }
    }
}

define_class!(
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "MuxyPromptPanel"]
    #[ivars = Cell<bool>]
    #[derive(Debug)]
    struct PromptPanel;

    unsafe impl NSObjectProtocol for PromptPanel {}

    impl PromptPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> bool {
            let modifiers = event.modifierFlags()
                & (NSEventModifierFlags::Command | NSEventModifierFlags::Control
                    | NSEventModifierFlags::Option | NSEventModifierFlags::Shift);
            if modifiers == NSEventModifierFlags::Command
                && event.charactersIgnoringModifiers().is_some_and(|key| key.to_string() == "w")
            {
                self.finish(NSModalResponseCancel);
                true
            } else {
                // SAFETY: Other shortcuts retain NSPanel's normal responder-chain behavior.
                unsafe { msg_send![super(self), performKeyEquivalent: event] }
            }
        }

        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            self.finish(NSModalResponseCancel);
        }

        #[unsafe(method(performClose:))]
        fn perform_close(&self, _sender: Option<&AnyObject>) {
            self.finish(NSModalResponseCancel);
        }

        #[unsafe(method(close))]
        fn close_panel(&self) {
            self.finish(NSModalResponseCancel);
        }
    }
);

impl PromptPanel {
    fn finish(&self, response: NSModalResponse) {
        if !self.ivars().replace(true)
            && let Some(parent) = self.sheetParent()
        {
            // The callback can drop the public handle while ending the sheet.
            let panel = self.retain();
            parent.endSheet_returnCode(&panel, response);
        }
    }
}

struct PromptState {
    panel: Weak<PromptPanel>,
    content: Retained<NSView>,
    heading: Retained<NSTextField>,
    body: Retained<NSTextField>,
    buttons: [Retained<NSButton>; 3],
    label: Retained<NSTextField>,
    count: Retained<NSTextField>,
    scroll: Retained<NSScrollView>,
    input: Retained<NSTextView>,
    helper: Retained<NSTextField>,
    collapsed: PromptLayout,
    expanded: PromptLayout,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PromptState]
    #[name = "MuxyPromptTarget"]
    struct PromptTarget;

    unsafe impl NSObjectProtocol for PromptTarget {}

    unsafe impl NSTextDelegate for PromptTarget {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _notification: &NSNotification) {
            let state = self.ivars();
            let mut value = state.input.string().to_string();
            let original_length = value.len();
            cap_prompt(&mut value);
            if value.len() != original_length {
                let selection = state.input.selectedRange();
                state.input.setString(&NSString::from_str(&value));
                state.input.setSelectedRange(NSRange::new(
                    selection.location.min(value.encode_utf16().count()),
                    0,
                ));
            }
            state.count.setStringValue(&NSString::from_str(&format!(
                "{}/{ADDITIONAL_PROMPT_LIMIT}",
                value.chars().count(),
            )));
        }
    }

    unsafe impl NSTextViewDelegate for PromptTarget {
        #[unsafe(method(textView:doCommandBySelector:))]
        unsafe fn text_view_command(&self, _view: &NSTextView, command: Sel) -> bool {
            let response = if command == sel!(cancelOperation:) {
                Some(NSModalResponseCancel)
            } else if command == sel!(insertNewline:) {
                Some(NSModalResponseOK)
            } else {
                None
            };
            if let Some(response) = response {
                if let Some(panel) = self.ivars().panel.load() {
                    panel.finish(response);
                }
                true
            } else {
                false
            }
        }
    }

    impl PromptTarget {
        #[unsafe(method(addPrompt:))]
        fn add_prompt(&self, _sender: &NSButton) {
            let state = self.ivars();
            if let Some(panel) = state.panel.load() {
                let resized = resized_frame(panel.frame(), state.expanded.size);
                panel.setFrame_display(resized, false);
                state.apply_layout(true);
                panel.invalidateShadow();
                panel.makeFirstResponder(Some(&state.input));
            }
        }

        #[unsafe(method(confirm:))]
        fn confirm(&self, _sender: &NSButton) {
            if let Some(panel) = self.ivars().panel.load() {
                panel.finish(NSModalResponseOK);
            }
        }

        #[unsafe(method(cancel:))]
        fn cancel(&self, _sender: &NSButton) {
            if let Some(panel) = self.ivars().panel.load() {
                panel.finish(NSModalResponseCancel);
            }
        }
    }
);

impl PromptTarget {
    fn new(
        panel: &PromptPanel,
        title: &str,
        message: &str,
        confirm_label: &str,
        style: Style<'_>,
        main_thread: MainThreadMarker,
    ) -> Retained<Self> {
        let Style { theme, metrics } = style;
        let width = points(metrics.scaled(460.0) - metrics.spacing8() * 2.0);
        let content = content_view(style, main_thread);
        let heading = label(
            title,
            &NSFont::systemFontOfSize_weight(points(metrics.font_headline()), unsafe {
                NSFontWeightSemibold
            }),
            theme.fg,
            width,
            main_thread,
        );
        let body = label(
            message,
            &NSFont::systemFontOfSize(points(metrics.font_body())),
            theme.fg_muted,
            width,
            main_thread,
        );
        let buttons = [
            button("Add Prompt", "", sel!(addPrompt:), *metrics, main_thread),
            button("Cancel", "\u{1b}", sel!(cancel:), *metrics, main_thread),
            button(confirm_label, "\r", sel!(confirm:), *metrics, main_thread),
        ];
        let label = label(
            "Additional Prompt",
            &NSFont::systemFontOfSize_weight(points(metrics.font_footnote()), unsafe {
                NSFontWeightMedium
            }),
            theme.fg_muted,
            width,
            main_thread,
        );
        let count = prompt_count(style, main_thread);
        let count_size = count.frame().size;
        let helper = self::label(
            "Appended after the configured prompt for this action only.",
            &NSFont::systemFontOfSize(points(metrics.font_caption())),
            theme.fg_muted,
            width,
            main_thread,
        );
        let (scroll, input) = prompt_input(style, width, main_thread);
        for view in [
            &*heading as &NSView,
            &body,
            &buttons[0],
            &buttons[1],
            &buttons[2],
            &label,
            &count,
            &scroll,
            &helper,
        ] {
            content.addSubview(view);
        }
        let measurements = PromptMeasurements {
            heading: heading.frame().size.height,
            body: body.frame().size.height,
            editor_header: label.frame().size.height.max(count_size.height),
            count_width: count_size.width,
            helper: helper.frame().size.height,
            buttons: buttons
                .each_ref()
                .map(|button| button.alignmentRectForFrame(button.frame()).size),
        };
        let collapsed = PromptLayout::new(*metrics, &measurements, false);
        let expanded = PromptLayout::new(*metrics, &measurements, true);
        let target = Self::alloc(main_thread).set_ivars(PromptState {
            panel: Weak::new(panel),
            content,
            heading,
            body,
            buttons,
            label,
            count,
            scroll,
            input,
            helper,
            collapsed,
            expanded,
        });
        // SAFETY: NSObject initialization and matching target selectors stay on the main thread.
        let target: Retained<Self> = unsafe { msg_send![super(target), init] };
        for button in &target.ivars().buttons {
            unsafe { button.setTarget(Some(&target)) };
        }
        target
            .ivars()
            .input
            .setDelegate(Some(ProtocolObject::from_ref(&*target)));
        target.ivars().apply_layout(false);
        panel.setContentView(Some(&target.ivars().content));
        panel.setContentSize(target.ivars().collapsed.size);
        panel.setInitialFirstResponder(Some(&target.ivars().buttons[2]));
        target
    }

    fn detach(&self) {
        self.ivars().input.setDelegate(None);
        // SAFETY: Clearing a target does not invoke its action.
        for button in &self.ivars().buttons {
            unsafe { button.setTarget(None) };
        }
    }
}

impl PromptState {
    fn apply_layout(&self, expanded: bool) {
        let layout = if expanded {
            &self.expanded
        } else {
            &self.collapsed
        };
        self.content.setFrameSize(layout.size);
        self.heading.setFrame(layout.heading);
        self.body.setFrame(layout.body);
        for (button, rect) in self.buttons.iter().zip(layout.buttons) {
            button.setFrame(button.frameForAlignmentRect(rect));
        }
        self.buttons[0].setHidden(expanded);
        self.label.setFrame(layout.label);
        self.count.setFrame(layout.count);
        self.scroll.setFrame(layout.editor);
        self.helper.setFrame(layout.helper);
        self.label.setHidden(!expanded);
        self.count.setHidden(!expanded);
        self.scroll.setHidden(!expanded);
        self.helper.setHidden(!expanded);
    }
}

/// Confirms an action, optionally expanding a plain-text prompt in the same sheet.
/// Only the confirmation button returns `Some`, including an empty prompt.
pub fn confirm_with_prompt(
    window: &gpui::Window,
    title: &str,
    message: &str,
    confirm_label: &str,
    style: Style<'_>,
    on_complete: impl FnOnce(Option<String>) + 'static,
) -> io::Result<PromptConfirmation> {
    let main_thread = MainThreadMarker::new()
        .ok_or_else(|| io::Error::other("native dialogs require the main thread"))?;
    let parent = parent_window(window, main_thread)?;
    let panel = PromptPanel::alloc(main_thread).set_ivars(Cell::new(false));
    // SAFETY: This main-thread panel is retained by the handle, not released by AppKit on close.
    let panel: Retained<PromptPanel> = unsafe {
        let panel = msg_send![super(panel),
            initWithContentRect: NSRect::ZERO,
            styleMask: NSWindowStyleMask::Borderless,
            backing: NSBackingStoreType::Buffered,
            defer: false,
        ];
        panel
    };
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setHidesOnDeactivate(false);
    panel.setOpaque(false);
    panel.setHasShadow(true);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setTitle(&NSString::from_str(title));
    let appearance = NSAppearance::appearanceNamed(unsafe {
        if style.theme.bg.l < 0.5 {
            NSAppearanceNameDarkAqua
        } else {
            NSAppearanceNameAqua
        }
    });
    panel.setAppearance(appearance.as_deref());
    let target = PromptTarget::new(&panel, title, message, confirm_label, style, main_thread);
    let default_cell = target.ivars().buttons[2]
        .cell()
        .and_then(|cell| cell.downcast::<NSButtonCell>().ok());
    panel.setDefaultButtonCell(default_cell.as_deref());
    let completed = Rc::new(Cell::new(false));
    let finished = Rc::clone(&completed);
    let callback = Cell::new(Some(on_complete));
    // The completion block retains the delegate, which holds only a weak panel reference.
    let handler = RcBlock::new(move |response| {
        let cancelled = finished.replace(true);
        target.detach();
        if let Some(panel) = target.ivars().panel.load() {
            panel.orderOut(None);
        }
        if let Some(callback) = callback.take() {
            callback(if cancelled {
                None
            } else {
                prompt_response(response, target.ivars().input.string().to_string())
            });
        }
    });
    parent.beginSheet_completionHandler(&panel, Some(&handler));
    Ok(PromptConfirmation {
        panel,
        parent,
        completed,
    })
}

struct PromptMeasurements {
    heading: f64,
    body: f64,
    editor_header: f64,
    count_width: f64,
    helper: f64,
    buttons: [NSSize; 3],
}

#[derive(Debug)]
struct PromptLayout {
    size: NSSize,
    heading: NSRect,
    body: NSRect,
    buttons: [NSRect; 3],
    label: NSRect,
    count: NSRect,
    editor: NSRect,
    helper: NSRect,
}

impl PromptLayout {
    fn new(metrics: Metrics, measured: &PromptMeasurements, expanded: bool) -> Self {
        let width = points(metrics.scaled(460.0));
        let padding = points(metrics.spacing8());
        let gap = points(metrics.scaled(14.0));
        let spacing = points(metrics.spacing3());
        let editor_spacing = points(metrics.spacing2());
        let content_width = width - 2.0 * padding;
        let button_height = measured
            .buttons
            .iter()
            .map(|size| size.height)
            .fold(0.0, f64::max);
        let confirm_x = width - padding - measured.buttons[2].width;
        let cancel_x = confirm_x - spacing - measured.buttons[1].width;
        let buttons = std::array::from_fn(|index| {
            let size = measured.buttons[index];
            frame(
                [padding, cancel_x, confirm_x][index],
                padding + (button_height - size.height) / 2.0,
                size.width,
                size.height,
            )
        });
        let mut y = padding + button_height + gap;
        let helper = frame(padding, y, content_width, measured.helper);
        let editor = frame(
            padding,
            y + measured.helper + editor_spacing,
            content_width,
            points(metrics.scaled(140.0)),
        );
        let header_y = editor.origin.y + editor.size.height + editor_spacing;
        let label = frame(
            padding,
            header_y,
            content_width - measured.count_width - spacing,
            measured.editor_header,
        );
        let count = frame(
            width - padding - measured.count_width,
            header_y,
            measured.count_width,
            measured.editor_header,
        );
        if expanded {
            y = header_y + measured.editor_header + gap;
        }
        let body = frame(padding, y, content_width, measured.body);
        y += measured.body + gap;
        let heading = frame(padding, y, content_width, measured.heading);
        Self {
            size: NSSize::new(width, y + measured.heading + padding),
            heading,
            body,
            buttons,
            label,
            count,
            editor,
            helper,
        }
    }
}

fn resized_frame(old: NSRect, size: NSSize) -> NSRect {
    frame(
        old.origin.x,
        old.origin.y + old.size.height - size.height,
        size.width,
        size.height,
    )
}

fn content_view(style: Style<'_>, main_thread: MainThreadMarker) -> Retained<NSView> {
    let content = NSView::initWithFrame(NSView::alloc(main_thread), NSRect::ZERO);
    content.setWantsLayer(true);
    if let Some(layer) = content.layer() {
        layer.setBackgroundColor(Some(&color(style.theme.bg).CGColor()));
        layer.setCornerRadius(points(style.metrics.radius_xl()));
        layer.setMasksToBounds(true);
    }
    content
}

fn label(
    text: &str,
    font: &NSFont,
    foreground: gpui::Hsla,
    width: f64,
    main_thread: MainThreadMarker,
) -> Retained<NSTextField> {
    let label = NSTextField::wrappingLabelWithString(&NSString::from_str(text), main_thread);
    label.setFont(Some(font));
    label.setTextColor(Some(&color(foreground)));
    label.setDrawsBackground(false);
    label.setMaximumNumberOfLines(0);
    label.setPreferredMaxLayoutWidth(width);
    let height = label.cell().map_or(0.0, |cell| {
        cell.cellSizeForBounds(frame(0.0, 0.0, width, f64::MAX))
            .height
            .ceil()
    });
    label.setFrame(frame(0.0, 0.0, width, height));
    label
}

fn prompt_count(style: Style<'_>, main_thread: MainThreadMarker) -> Retained<NSTextField> {
    let Style { theme, metrics } = style;
    let count = NSTextField::labelWithString(
        &NSString::from_str(&format!(
            "{ADDITIONAL_PROMPT_LIMIT}/{ADDITIONAL_PROMPT_LIMIT}"
        )),
        main_thread,
    );
    count.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(
        points(metrics.font_caption()),
        unsafe { NSFontWeightRegular },
    )));
    count.setTextColor(Some(&color(theme.fg_dim)));
    count.setAlignment(NSTextAlignment::Right);
    count.sizeToFit();
    count.setStringValue(&NSString::from_str(&format!("0/{ADDITIONAL_PROMPT_LIMIT}")));
    count
}

fn prompt_input(
    style: Style<'_>,
    width: f64,
    main_thread: MainThreadMarker,
) -> (Retained<NSScrollView>, Retained<NSTextView>) {
    let Style { theme, metrics } = style;
    let scroll = NSScrollView::initWithFrame(
        NSScrollView::alloc(main_thread),
        frame(0.0, 0.0, width, points(metrics.scaled(140.0))),
    );
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(false);
    scroll.setAutohidesScrollers(true);
    scroll.setBorderType(NSBorderType::NoBorder);
    scroll.setDrawsBackground(false);
    scroll.contentView().setDrawsBackground(false);
    scroll.setWantsLayer(true);
    if let Some(layer) = scroll.layer() {
        layer.setCornerRadius(points(metrics.radius_sm()));
        layer.setBorderWidth(1.0);
        layer.setBorderColor(Some(&color(theme.border).CGColor()));
        layer.setBackgroundColor(Some(&color(theme.surface).CGColor()));
        layer.setMasksToBounds(true);
    }
    let size = scroll.contentSize();
    let input = NSTextView::initWithFrame(
        NSTextView::alloc(main_thread),
        NSRect::new(NSPoint::ZERO, size),
    );
    input.setRichText(false);
    input.setImportsGraphics(false);
    input.setEditable(true);
    input.setSelectable(true);
    input.setAllowsUndo(true);
    input.setAccessibilityLabel(Some(&NSString::from_str("Additional prompt")));
    input.setToolTip(Some(&NSString::from_str(
        "Option-Return inserts a new line.",
    )));
    input.setTextColor(Some(&color(theme.fg)));
    input.setDrawsBackground(false);
    input.setFont(Some(&NSFont::systemFontOfSize(points(
        metrics.font_footnote(),
    ))));
    let inset = points(metrics.spacing3());
    input.setTextContainerInset(NSSize::new(inset, inset));
    input.setMinSize(NSSize::new(0.0, size.height));
    input.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
    input.setVerticallyResizable(true);
    input.setHorizontallyResizable(false);
    input.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    // SAFETY: The container belongs to this main-thread text view.
    if let Some(container) = unsafe { input.textContainer() } {
        container.setContainerSize(NSSize::new(size.width - 2.0 * inset, f64::MAX));
        container.setWidthTracksTextView(true);
        container.setLineFragmentPadding(0.0);
    }
    scroll.setDocumentView(Some(&input));
    (scroll, input)
}

fn button(
    title: &str,
    key: &str,
    action: Sel,
    metrics: Metrics,
    main_thread: MainThreadMarker,
) -> Retained<NSButton> {
    // SAFETY: Each selector is implemented by PromptTarget, installed before presentation.
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            None,
            Some(action),
            main_thread,
        )
    };
    button.setBezelStyle(NSBezelStyle::Push);
    button.setFont(Some(&NSFont::systemFontOfSize(points(
        metrics.font_emphasis(),
    ))));
    button.setKeyEquivalent(&NSString::from_str(key));
    button.setKeyEquivalentModifierMask(NSEventModifierFlags::empty());
    button.sizeToFit();
    button
}

fn color(value: gpui::Hsla) -> Retained<NSColor> {
    let value: gpui::Rgba = value.into();
    NSColor::colorWithSRGBRed_green_blue_alpha(
        f64::from(value.r),
        f64::from(value.g),
        f64::from(value.b),
        f64::from(value.a),
    )
}

fn points(value: gpui::Pixels) -> f64 {
    f64::from(f32::from(value))
}

fn frame(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

fn cap_prompt(prompt: &mut String) {
    if let Some((boundary, _)) = prompt.char_indices().nth(ADDITIONAL_PROMPT_LIMIT) {
        prompt.truncate(boundary);
    }
}

fn prompt_response(response: NSModalResponse, mut prompt: String) -> Option<String> {
    (response == NSModalResponseOK).then(|| {
        cap_prompt(&mut prompt);
        prompt
    })
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "Layout fixtures use exactly representable point values."
)]
mod tests {
    use super::*;

    fn measurements(scale: f64) -> PromptMeasurements {
        PromptMeasurements {
            heading: 17.0 * scale,
            body: 45.0 * scale,
            editor_header: 14.0 * scale,
            count_width: 54.0 * scale,
            helper: 12.0 * scale,
            buttons: [80.0, 65.0, 120.0].map(|width| NSSize::new(width * scale, 24.0 * scale)),
        }
    }

    #[test]
    fn collapsed_and_expanded_keep_the_swift_width_and_single_button_row() {
        for expanded in [false, true] {
            let layout = PromptLayout::new(Metrics::new(1.0), &measurements(1.0), expanded);
            assert_eq!(layout.size.width, 460.0);
            assert_eq!(layout.heading.origin.x, 20.0);
            assert_eq!(layout.heading.size.width, 420.0);
            assert_eq!(
                layout.size.height - layout.heading.origin.y - layout.heading.size.height,
                20.0
            );
            assert_eq!(
                layout.heading.origin.y - layout.body.origin.y - layout.body.size.height,
                14.0
            );
            assert_eq!(layout.buttons[0], frame(20.0, 20.0, 80.0, 24.0));
            assert_eq!(layout.buttons[1], frame(249.0, 20.0, 65.0, 24.0));
            assert_eq!(layout.buttons[2], frame(320.0, 20.0, 120.0, 24.0));
        }
    }

    #[test]
    fn editor_is_between_body_and_buttons_with_swift_spacing() {
        let metrics = Metrics::new(1.0);
        let measured = measurements(1.0);
        let collapsed = PromptLayout::new(metrics, &measured, false);
        let expanded = PromptLayout::new(metrics, &measured, true);
        assert_eq!(collapsed.body.origin.y, 58.0);
        assert_eq!(expanded.helper, frame(20.0, 58.0, 420.0, 12.0));
        assert_eq!(expanded.editor, frame(20.0, 74.0, 420.0, 140.0));
        assert_eq!(expanded.label.origin.y, 218.0);
        assert_eq!(expanded.count, frame(386.0, 218.0, 54.0, 14.0));
        assert_eq!(expanded.body.origin.y, 246.0);
        assert_eq!(expanded.size.height - collapsed.size.height, 188.0);
    }

    #[test]
    fn layout_scales_and_expansion_preserves_the_top_edge() {
        for scale in [0.75, 1.0, 1.5, 2.0] {
            let metrics = Metrics::new(scale);
            let scale = f64::from(scale);
            let measured = measurements(scale);
            let collapsed = PromptLayout::new(metrics, &measured, false);
            let expanded = PromptLayout::new(metrics, &measured, true);
            assert_eq!(expanded.size.width, 460.0 * scale);
            assert_eq!(expanded.editor.size.height, 140.0 * scale);
            assert_eq!(expanded.editor.origin.x, 20.0 * scale);
            assert_eq!(
                expanded.editor.origin.y - expanded.helper.origin.y - expanded.helper.size.height,
                4.0 * scale
            );
            assert_eq!(
                expanded.buttons[2].origin.x
                    - expanded.buttons[1].origin.x
                    - expanded.buttons[1].size.width,
                6.0 * scale
            );
            let old = NSRect::new(NSPoint::new(100.0, 300.0), collapsed.size);
            let new = resized_frame(old, expanded.size);
            assert_eq!(new.origin.x, old.origin.x);
            assert_eq!(
                new.origin.y + new.size.height,
                old.origin.y + old.size.height
            );
            assert_eq!(new.size.width, old.size.width);
        }
    }

    #[test]
    fn only_confirmation_returns_a_prompt() {
        assert_eq!(
            prompt_response(NSModalResponseOK, String::new()),
            Some(String::new())
        );
        assert_eq!(
            prompt_response(NSModalResponseOK, "guidance".into()),
            Some("guidance".into())
        );
        for response in [NSModalResponseCancel, -1000, -1001, 1002] {
            assert_eq!(prompt_response(response, "guidance".into()), None);
        }
    }

    #[test]
    fn cap_counts_unicode_characters_without_splitting_them() {
        for value in ["", "plain text", "🦀 café\n你好"] {
            let mut prompt = value.to_owned();
            cap_prompt(&mut prompt);
            assert_eq!(prompt, value);
        }
        for character in ['a', 'é', '🦀'] {
            let expected = character.to_string().repeat(ADDITIONAL_PROMPT_LIMIT);
            let mut prompt = format!("{expected}overflow");
            cap_prompt(&mut prompt);
            assert_eq!(prompt, expected);
            assert_eq!(prompt.chars().count(), ADDITIONAL_PROMPT_LIMIT);
            assert_eq!(
                prompt_response(NSModalResponseOK, format!("{expected}extra")),
                Some(expected)
            );
        }
        let mut prompt = format!("{}e\u{301}", "a".repeat(ADDITIONAL_PROMPT_LIMIT - 1));
        cap_prompt(&mut prompt);
        assert!(prompt.ends_with('e'));
        assert_eq!(prompt.chars().count(), ADDITIONAL_PROMPT_LIMIT);
    }
}
