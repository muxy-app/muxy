use super::{Api, Cell, FALSE, Page, Pointer, ffi, gpointer, seat};

#[derive(Default)]
pub(super) struct State {
    inspector: Cell<bool>,
    grabbed: Cell<Option<bool>>,
}

impl State {
    pub(super) fn release(&self) {
        self.grabbed.set(None);
    }
}

impl Page {
    pub(super) fn input_focus(&self, g: &Api) -> gpointer {
        let focus = unsafe { g.gtk_window_get_focus(self.window) };
        if focus.is_null() { self.view } else { focus }
    }

    fn attached_inspector(&self, g: &Api) -> Option<gpointer> {
        self.browser.as_ref()?;
        unsafe {
            let inspector = g.webkit_web_view_get_inspector(self.view);
            if g.webkit_web_inspector_is_attached(inspector) == FALSE {
                return None;
            }
            let view = g.webkit_web_inspector_get_web_view(inspector);
            (!view.is_null()).then_some(view)
        }
    }

    pub(super) fn pointer_target(&self, g: &Api, event: Pointer) -> (gpointer, f64, f64) {
        let (x, y) = match event {
            Pointer::Moved { position, .. }
            | Pointer::Pressed { position, .. }
            | Pointer::Released { position, .. }
            | Pointer::Scrolled { position, .. } => self.units(position),
            Pointer::Left => (0.0, 0.0),
        };
        let inspector = self.attached_inspector(g);
        let (mut origin_x, mut origin_y) = (0, 0);
        let inside = inspector.is_some_and(|view| unsafe {
            g.gtk_widget_translate_coordinates(
                view,
                self.view,
                0,
                0,
                &raw mut origin_x,
                &raw mut origin_y,
            ) != FALSE
                && x >= f64::from(origin_x)
                && y >= f64::from(origin_y)
                && x < f64::from(origin_x) + f64::from(g.gtk_widget_get_allocated_width(view))
                && y < f64::from(origin_y) + f64::from(g.gtk_widget_get_allocated_height(view))
        });
        if matches!(event, Pointer::Moved { pressed: None, .. }) {
            self.input.grabbed.set(None);
        }
        let over_inspector = match event {
            Pointer::Left => self.input.inspector.get(),
            Pointer::Scrolled { .. } => inside,
            _ => self.input.grabbed.get().unwrap_or(inside),
        } && inspector.is_some();
        if matches!(event, Pointer::Pressed { .. }) {
            self.input.grabbed.set(Some(over_inspector));
        } else if matches!(event, Pointer::Released { .. }) {
            self.input.grabbed.set(None);
        }
        let previous = self.input.inspector.replace(over_inspector);
        if previous != over_inspector {
            let previous = if previous { inspector } else { Some(self.view) };
            if let Some(previous) = previous {
                unsafe {
                    let window = g.gtk_widget_get_window(previous);
                    if !window.is_null() {
                        Self::dispatch::<super::GdkEventCrossing>(
                            g,
                            ffi::EVENT_LEAVE_NOTIFY,
                            window,
                            g.gdk_seat_get_pointer(seat(g)),
                            |_| {},
                        );
                    }
                }
            }
        }
        let (view, x, y) = inspector
            .filter(|_| over_inspector)
            .map_or((self.view, x, y), |view| {
                (view, x - f64::from(origin_x), y - f64::from(origin_y))
            });
        if matches!(event, Pointer::Pressed { .. }) {
            unsafe {
                g.gtk_widget_grab_focus(view);
            }
        }
        (view, x, y)
    }
}
