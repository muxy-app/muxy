use std::time::Duration;

use gpui::{Window, px, size};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::{
    connection::Connection,
    protocol::xproto::{self, ConnectionExt, EventMask, InputFocus, PropMode, StackMode},
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use crate::quick_terminal::{panel::QuickTerminalConfiguration, view::AccessibilityNode};

#[derive(Debug)]
pub(in crate::quick_terminal::platform) struct PanelAdapter {
    connection: RustConnection,
    window: u32,
    root: u32,
    active_window: u32,
    previous: Option<u32>,
    visible: bool,
}

#[allow(clippy::unused_self, reason = "Shared panel backend interface")]
impl PanelAdapter {
    pub(in crate::quick_terminal::platform) fn configure(window: &Window) -> Result<Self, String> {
        if let Some(reason) =
            super::capability(window).reason(muxy_core::capability::Feature::QuickTerminal)
        {
            return Err(reason.to_string());
        }
        let handle = HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?;
        let native = match handle.as_raw() {
            RawWindowHandle::Xcb(handle) => handle.window.get(),
            RawWindowHandle::Xlib(handle) => {
                u32::try_from(handle.window).map_err(|error| error.to_string())?
            }
            _ => return Err("Quick Terminal needs an X11 window".into()),
        };
        Self::native(native)
    }

    fn native(native: u32) -> Result<Self, String> {
        let (connection, screen) = x11rb::connect(None).map_err(|error| error.to_string())?;
        let root = connection.setup().roots[screen].root;
        let active_window = atom(&connection, b"_NET_ACTIVE_WINDOW")?;
        let panel = Self {
            connection,
            window: native,
            root,
            active_window,
            previous: None,
            visible: false,
        };
        panel.configure_properties()?;
        Ok(panel)
    }

    fn configure_properties(&self) -> Result<(), String> {
        let connection = &self.connection;
        let window_type = atom(connection, b"_NET_WM_WINDOW_TYPE")?;
        let utility = atom(connection, b"_NET_WM_WINDOW_TYPE_UTILITY")?;
        connection
            .change_property32(
                PropMode::REPLACE,
                self.window,
                window_type,
                xproto::AtomEnum::ATOM,
                &[utility],
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
        let state = atom(connection, b"_NET_WM_STATE")?;
        let states = [
            b"_NET_WM_STATE_ABOVE".as_slice(),
            b"_NET_WM_STATE_SKIP_TASKBAR",
            b"_NET_WM_STATE_SKIP_PAGER",
            b"_NET_WM_STATE_STICKY",
        ]
        .into_iter()
        .map(|name| atom(connection, name))
        .collect::<Result<Vec<_>, _>>()?;
        connection
            .change_property32(
                PropMode::REPLACE,
                self.window,
                state,
                xproto::AtomEnum::ATOM,
                &states,
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
        let desktop = atom(connection, b"_NET_WM_DESKTOP")?;
        connection
            .change_property32(
                PropMode::REPLACE,
                self.window,
                desktop,
                xproto::AtomEnum::CARDINAL,
                &[u32::MAX],
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
        connection
            .unmap_window(self.window)
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "Panel dimensions are bounded and converted between device and logical pixels"
    )]
    pub(in crate::quick_terminal::platform) fn prepare(
        &mut self,
        configuration: QuickTerminalConfiguration,
        window: &mut Window,
    ) -> Result<(), String> {
        let configuration = configuration.normalized();
        let geometry = self
            .connection
            .get_geometry(self.root)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?;
        let pointer = self
            .connection
            .query_pointer(self.root)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?;
        let mut area = (
            0_i32,
            0_i32,
            u32::from(geometry.width),
            u32::from(geometry.height),
        );
        if let Ok(cookie) = x11rb::protocol::randr::get_monitors(&self.connection, self.root, true)
            && let Ok(monitors) = cookie.reply()
            && let Some(monitor) = monitors.monitors.iter().find(|monitor| {
                i32::from(pointer.root_x) >= i32::from(monitor.x)
                    && i32::from(pointer.root_x) < i32::from(monitor.x) + i32::from(monitor.width)
                    && i32::from(pointer.root_y) >= i32::from(monitor.y)
                    && i32::from(pointer.root_y) < i32::from(monitor.y) + i32::from(monitor.height)
            })
        {
            area = (
                i32::from(monitor.x),
                i32::from(monitor.y),
                u32::from(monitor.width),
                u32::from(monitor.height),
            );
        }
        let current_desktop = property(&self.connection, self.root, b"_NET_CURRENT_DESKTOP")
            .first()
            .copied()
            .unwrap_or(0) as usize;
        let workareas = property(&self.connection, self.root, b"_NET_WORKAREA");
        if let Some(workarea) = current_desktop
            .checked_mul(4)
            .and_then(|start| workareas.get(start..start.saturating_add(4)))
        {
            let left = area.0.max(workarea[0].cast_signed());
            let top = area.1.max(workarea[1].cast_signed());
            let right = (i64::from(area.0) + i64::from(area.2))
                .min(i64::from(workarea[0].cast_signed()) + i64::from(workarea[2]));
            let bottom = (i64::from(area.1) + i64::from(area.3))
                .min(i64::from(workarea[1].cast_signed()) + i64::from(workarea[3]));
            if right > i64::from(left) && bottom > i64::from(top) {
                area = (
                    left,
                    top,
                    (right - i64::from(left)) as u32,
                    (bottom - i64::from(top)) as u32,
                );
            }
        }
        let scale = window.scale_factor();
        let width = ((configuration.width as f32 * scale) as u32)
            .min(area.2)
            .max(1);
        let height = ((configuration.height as f32 * scale) as u32)
            .min(area.3)
            .max(1);
        window.resize(size(px(width as f32 / scale), px(height as f32 / scale)));
        self.connection
            .configure_window(
                self.window,
                &xproto::ConfigureWindowAux::new()
                    .x(area.0 + ((area.2 - width) / 2).cast_signed())
                    .y(area.1)
                    .width(width)
                    .height(height),
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub(in crate::quick_terminal::platform) fn sync_bounds(&self, _visible: bool) {}

    pub(in crate::quick_terminal::platform) fn is_active(&self, active: bool) -> bool {
        active || self.is_key()
    }

    fn is_key(&self) -> bool {
        self.connection
            .get_input_focus()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|reply| reply.focus == self.window)
    }

    pub(in crate::quick_terminal::platform) fn show(&mut self, _duration: Duration) {
        if !self.visible {
            self.previous = property(&self.connection, self.root, b"_NET_ACTIVE_WINDOW")
                .first()
                .copied()
                .filter(|&id| id != 0 && id != self.window);
        }
        self.visible = true;
        let _ = self.connection.map_window(self.window);
        let _ = self.connection.configure_window(
            self.window,
            &xproto::ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        );
        self.activate(self.window);
    }

    pub(in crate::quick_terminal::platform) fn begin_hide(&mut self, _duration: Duration) {}

    pub(in crate::quick_terminal::platform) fn finish_hide(&mut self, restores_focus: bool) {
        let is_key = self.is_key();
        let _ = self.connection.unmap_window(self.window);
        self.visible = false;
        if let Some(previous) = self.previous.take().filter(|_| restores_focus && is_key) {
            self.activate(previous);
        }
        let _ = self.connection.flush();
    }

    fn activate(&self, window: u32) {
        let message = xproto::ClientMessageEvent::new(
            32,
            window,
            self.active_window,
            [1, x11rb::CURRENT_TIME, 0, 0, 0],
        );
        let _ = self.connection.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            message,
        );
        let _ =
            self.connection
                .set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME);
        let _ = self.connection.flush();
    }

    pub(in crate::quick_terminal::platform) fn install_accessibility(
        &mut self,
        _nodes: &[AccessibilityNode],
    ) {
    }
}

impl Drop for PanelAdapter {
    fn drop(&mut self) {
        let _ = self.connection.unmap_window(self.window);
        let _ = self.connection.flush();
    }
}

fn atom(connection: &RustConnection, name: &[u8]) -> Result<u32, String> {
    connection
        .intern_atom(false, name)
        .map_err(|error| error.to_string())?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| error.to_string())
}

fn property(connection: &RustConnection, window: u32, name: &[u8]) -> Vec<u32> {
    atom(connection, name)
        .ok()
        .and_then(|atom| {
            connection
                .get_property(false, window, atom, xproto::AtomEnum::ANY, 0, 4096)
                .ok()
        })
        .and_then(|cookie| cookie.reply().ok())
        .and_then(|reply| reply.value32().map(Iterator::collect))
        .unwrap_or_default()
}
