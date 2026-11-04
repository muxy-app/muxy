use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSApplication, NSEvent, NSEventMask, NSEventType, NSWindow};

#[derive(Debug)]
struct DragState {
    window: Retained<NSWindow>,
    mouse_down: RefCell<Option<Retained<NSEvent>>>,
}

impl DragState {
    fn handle(&self, event: &NSEvent, mtm: MainThreadMarker) -> bool {
        match event.r#type() {
            NSEventType::LeftMouseDragged => {
                let mouse_down = self.mouse_down.take();
                if let Some(mouse_down) = mouse_down
                    && event.window(mtm).as_deref() == Some(&*self.window)
                {
                    // This starts a nested native event loop, so run before GPUI
                    // dispatch rather than from inside a borrowed App or Window.
                    self.window.performWindowDragWithEvent(&mouse_down);
                    return true;
                }
            }
            NSEventType::LeftMouseDown | NSEventType::LeftMouseUp | NSEventType::MouseMoved => {
                self.mouse_down.take();
            }
            _ => {}
        }
        false
    }
}

/// Enables native window movement only for explicitly claimed background presses.
#[derive(Debug)]
pub struct WindowDrag {
    state: Rc<DragState>,
    monitor: Retained<AnyObject>,
}

impl WindowDrag {
    pub fn new(window: &gpui::Window) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let window = crate::appkit::ns_window(window).ok()?;
        window.setMovable(false);
        window.setMovableByWindowBackground(false);
        let state = Rc::new(DragState {
            window,
            mouse_down: RefCell::new(None),
        });
        let monitored = state.clone();
        let handler = RcBlock::new(move |event: NonNull<NSEvent>| {
            // SAFETY: AppKit lends a valid event to this main-thread monitor.
            if monitored.handle(unsafe { event.as_ref() }, mtm) {
                std::ptr::null_mut()
            } else {
                event.as_ptr()
            }
        });
        // SAFETY: The block returns the original event or null to consume it.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::LeftMouseDown
                    | NSEventMask::LeftMouseDragged
                    | NSEventMask::LeftMouseUp
                    | NSEventMask::MouseMoved,
                &handler,
            )
        }?;
        Some(Self { state, monitor })
    }

    /// Call during a left press on empty titlebar space, after control hit testing.
    pub fn begin(&self) {
        self.cancel();
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        if let Some(event) = NSApplication::sharedApplication(mtm).currentEvent()
            && event.r#type() == NSEventType::LeftMouseDown
            && event.clickCount() == 1
            && event.window(mtm).as_deref() == Some(&*self.state.window)
        {
            *self.state.mouse_down.borrow_mut() = Some(event);
        }
    }

    pub fn cancel(&self) {
        self.state.mouse_down.take();
    }
}

impl Drop for WindowDrag {
    fn drop(&mut self) {
        // SAFETY: This token was returned by AppKit's monitor registration above.
        unsafe { NSEvent::removeMonitor(&self.monitor) };
    }
}
