use std::time::Duration;

use gpui::{AnyWindowHandle, Context, Task, Window};

use super::pane::{PaneState, TerminalPane};

const BLINK_INTERVAL: Duration = Duration::from_millis(530);

pub(crate) struct CursorBlink {
    pub(crate) enabled: bool,
    pub(crate) visible: bool,
    task: Option<Task<()>>,
    window: Option<AnyWindowHandle>,
}

impl Default for CursorBlink {
    fn default() -> Self {
        Self {
            enabled: true,
            visible: true,
            task: None,
            window: None,
        }
    }
}

impl CursorBlink {
    pub(crate) fn reset(&mut self) {
        self.task = None;
        self.visible = true;
    }
}

impl TerminalPane {
    pub(super) fn restart_cursor_blink(&mut self, cx: &mut Context<Self>) {
        let running = self.cursor_blink.task.is_some();
        self.cursor_blink.reset();
        if running && let Some(window) = self.cursor_blink.window {
            self.start_cursor_blink(window, cx);
        }
    }

    fn should_blink_cursor(&self, window: &Window) -> bool {
        self.cursor_blink.enabled
            && self.state == PaneState::Live
            && self.focused
            && self.native_visible
            && self.focus.is_focused(window)
            && window.is_window_active()
            && self.scroll.view.is_none()
            && self.grid.as_ref().is_some_and(|grid| {
                grid.cursor.visible
                    && grid.cursor.col < grid.size.cols
                    && grid.cursor.row < self.viewport().unwrap_or(grid.size).rows
            })
    }

    pub(crate) fn sync_cursor_blink(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.cursor_blink.window = Some(window.window_handle());
        if !self.should_blink_cursor(window) {
            self.cursor_blink.reset();
        } else if self.cursor_blink.task.is_none() {
            self.start_cursor_blink(window.window_handle(), cx);
        }
    }

    fn start_cursor_blink(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        self.cursor_blink.task = Some(cx.spawn(async move |pane, cx| {
            loop {
                cx.background_executor().timer(BLINK_INTERVAL).await;
                let keep_blinking = window.update(cx, |_, window, cx| {
                    pane.update(cx, |pane, cx| {
                        let blinking = pane.should_blink_cursor(window);
                        if blinking {
                            crate::profiler::count(crate::profiler::Metric::CursorBlink, 1);
                            pane.cursor_blink.visible = !pane.cursor_blink.visible;
                        } else {
                            pane.cursor_blink.reset();
                        }
                        cx.notify();
                        blinking
                    })
                });
                if !matches!(keep_blinking, Ok(Ok(true))) {
                    break;
                }
            }
        }));
    }
}
