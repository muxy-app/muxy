use std::collections::{HashMap, HashSet};

use gpui::{
    App, AppContext, Bounds, Context, Pixels, Point, Task, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowHandle, WindowOptions, point, px, size,
};
use muxy_app_core::{
    AppError, AppState, PaneDrop, PaneId, ProjectId, ServerId, SessionMove, TabId, TabPlace,
    TabWindowId, WindowBounds,
};
use muxy_protocol::Feature;
use muxy_protocol::SessionId;
use muxy_ui::tr;

use super::{AppModel, Quitting};
use crate::boot::Work;
use crate::views::tab_window::TabWindowView;

pub(crate) mod floating;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HeaderEdges {
    pub(crate) leading: Pixels,
    pub(crate) trailing: Pixels,
    pub(crate) carries: Option<TabWindowId>,
    pub(crate) floats: Option<TabWindowId>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Surface {
    #[default]
    Main,
    Tab(TabWindowId),
}

const MIN_SIZE: (f32, f32) = (420.0, 280.0);
const DEFAULT_SIZE: (f32, f32) = (900.0, 600.0);

#[derive(Default)]
pub(crate) struct TabWindows {
    open: HashMap<TabWindowId, WindowHandle<TabWindowView>>,
    opening: HashSet<TabWindowId>,
    key: Option<TabWindowId>,
    scope: Option<Surface>,
    focus: HashSet<TabWindowId>,
    grabs: HashMap<TabWindowId, Point<Pixels>>,
    bounds_save: Option<Task<()>>,
    pub(crate) overlay: Surface,
    pages: HashMap<PaneId, gpui::WindowId>,
    titles: HashMap<TabWindowId, String>,
    moving: HashMap<(ServerId, SessionId), ProjectId>,
    retried: HashSet<(ServerId, SessionId)>,
    controls: HashMap<TabWindowId, f32>,
}

impl TabWindows {
    pub(crate) fn handle(&self, id: TabWindowId) -> Option<WindowHandle<TabWindowView>> {
        self.open.get(&id).copied()
    }

    pub(crate) fn page_home(&self, pane: PaneId) -> Option<gpui::WindowId> {
        self.pages.get(&pane).copied()
    }

    pub(crate) fn set_page_home(
        &mut self,
        pane: PaneId,
        window: gpui::WindowId,
        main: gpui::WindowId,
    ) {
        if window == main {
            self.pages.remove(&pane);
        } else {
            self.pages.insert(pane, window);
        }
    }

    pub(crate) fn controls(&self, id: TabWindowId) -> f32 {
        self.controls.get(&id).copied().unwrap_or_default()
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn set_controls(&mut self, id: TabWindowId, width: f32) {
        self.controls.insert(id, width);
    }

    pub(crate) fn retitle(&mut self, id: TabWindowId, title: &str) -> bool {
        if self.titles.get(&id).is_some_and(|known| known == title) {
            return false;
        }
        self.titles.insert(id, title.to_owned());
        true
    }

    pub(crate) fn key_window(&self) -> Option<TabWindowId> {
        self.key
    }

    pub(crate) fn request_focus(&mut self, id: TabWindowId) {
        self.focus.insert(id);
    }

    pub(crate) fn take_focus(&mut self, id: TabWindowId) -> bool {
        self.focus.remove(&id)
    }

    pub(crate) fn take_grab(&mut self, id: TabWindowId) -> Option<Point<Pixels>> {
        self.grabs.remove(&id)
    }
}

impl AppModel {
    pub(crate) fn surface(&self) -> Surface {
        self.tab_windows
            .scope
            .unwrap_or_else(|| self.tab_windows.key.map_or(Surface::Main, Surface::Tab))
    }

    pub(crate) fn drawing<R>(&mut self, surface: Surface, draw: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.tab_windows.scope.replace(surface);
        let result = draw(self);
        self.tab_windows.scope = previous;
        result
    }

    pub(crate) fn surface_of(&self, window: &Window) -> Surface {
        let id = window.window_handle().window_id();
        self.tab_windows
            .open
            .iter()
            .find(|(_, handle)| handle.window_id() == id)
            .map_or(Surface::Main, |(tab_window, _)| Surface::Tab(*tab_window))
    }

    pub(crate) fn enter_surface(&mut self, surface: Surface) {
        self.tab_windows.key = match surface {
            Surface::Main => None,
            Surface::Tab(id) => Some(id),
        };
    }

    pub(crate) fn header_edges(&self, pane: PaneId, tab: &muxy_app_core::Tab) -> HeaderEdges {
        let Some(window) = &tab.window else {
            return HeaderEdges::default();
        };
        if !self.headers_on_top(tab) {
            return HeaderEdges::default();
        }
        let Some((_, [x, y, width, _])) = tab
            .layout
            .rects()
            .into_iter()
            .find(|(leaf, _)| *leaf == pane)
        else {
            return HeaderEdges::default();
        };
        let top = y < 0.001;
        if !top {
            return HeaderEdges::default();
        }
        if window.is_floating() {
            return HeaderEdges {
                floats: Some(window.id),
                ..HeaderEdges::default()
            };
        }
        let leading = if cfg!(target_os = "macos") && x < 0.001 {
            self.metrics.traffic_light_width()
        } else {
            px(0.0)
        };
        let trailing = if x + width > 0.999 {
            px(self.tab_windows.controls(window.id))
        } else {
            px(0.0)
        };
        HeaderEdges {
            leading,
            trailing,
            carries: Some(window.id),
            floats: None,
        }
    }

    pub(crate) fn headers_on_top(&self, tab: &muxy_app_core::Tab) -> bool {
        self.appearance.pane_headers
            && tab.zoomed.is_none()
            && tab
                .layout
                .rects()
                .into_iter()
                .filter(|(_, rect)| rect[1] < 0.001)
                .all(|(leaf, _)| {
                    tab.panes.iter().any(|pane| {
                        pane.id == leaf
                            && matches!(pane.content, muxy_app_core::PaneContent::Terminal { .. })
                    })
                })
    }

    fn refresh_open_contexts(&mut self, cx: &mut Context<Self>) {
        let panes: Vec<_> = self.grids.keys().copied().collect();
        for pane in panes {
            let context = self.opener_context(pane);
            if let Some(view) = self.terminal(&pane).map(|view| view.view.clone()) {
                view.update(cx, |view, _| view.open_context = context);
            }
        }
    }

    pub(crate) fn return_pane_window(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if let Some(id) = self.state.pane_window(pane) {
            self.return_tab_window(id, None, cx);
        }
    }

    pub(crate) fn window_for_pane(&self, pane: PaneId) -> gpui::AnyWindowHandle {
        self.state
            .pane_window(pane)
            .and_then(|id| self.tab_windows.handle(id))
            .map_or(self.window, Into::into)
    }

    pub(crate) fn window_tab(&self, id: TabWindowId) -> Option<TabId> {
        self.state.tab_window(id).map(|(_, tab, _)| tab.id)
    }

    pub(crate) fn sync_tab_windows(&mut self, cx: &mut Context<Self>) {
        let wanted: HashSet<_> = self
            .state
            .tab_windows()
            .filter(|(_, _, window)| !window.is_floating())
            .map(|(_, _, window)| window.id)
            .collect();
        let gone: Vec<_> = self
            .tab_windows
            .open
            .keys()
            .copied()
            .filter(|id| !wanted.contains(id))
            .collect();
        for id in gone {
            if let Some(handle) = self.tab_windows.open.remove(&id) {
                if self.tab_windows.key == Some(id) {
                    self.tab_windows.key = None;
                }
                self.tab_windows.focus.remove(&id);
                self.tab_windows.titles.remove(&id);
                let window = handle.window_id();
                self.move_drag.zones.forget_window(window);
                self.layout_drag.geometry.clear(window);
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                });
            }
        }
        self.tab_windows.opening.retain(|id| wanted.contains(id));
        let state = &self.state;
        self.tab_windows
            .pages
            .retain(|pane, _| state.pane(*pane).is_some());
        for id in wanted {
            if self.tab_windows.open.contains_key(&id) || !self.tab_windows.opening.insert(id) {
                continue;
            }
            let model = cx.weak_entity();
            cx.defer(move |cx| open_tab_window(&model, id, cx));
        }
    }

    fn tab_window_options(
        &mut self,
        id: TabWindowId,
        cx: &mut Context<Self>,
    ) -> Option<(WindowOptions, Option<Point<Pixels>>)> {
        if !self.tab_windows.opening.remove(&id) || self.quitting != Quitting::Idle {
            return None;
        }
        let (_, tab, saved) = self
            .state
            .tab_window(id)
            .filter(|(_, _, window)| !window.is_floating())?;
        let title = self.tab_window_title(tab.id, cx);
        let bounds = saved
            .bounds
            .and_then(gpui_bounds)
            .unwrap_or_else(|| default_bounds(self.main_bounds(cx)));
        let options = WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
            window_background: WindowBackgroundAppearance::Transparent,
            #[cfg(target_os = "linux")]
            window_decorations: Some(gpui::WindowDecorations::Client),
            is_movable: !cfg!(target_os = "macos"),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                appears_transparent: true,
                traffic_light_position: Some(crate::views::titlebar::traffic_light_position(
                    self.metrics,
                    1.0,
                )),
            }),
            ..crate::window_options()
        };
        Some((options, self.tab_windows.take_grab(id)))
    }

    fn tab_window_opened(
        &mut self,
        id: TabWindowId,
        opened: Result<WindowHandle<TabWindowView>, String>,
        cx: &mut Context<Self>,
    ) {
        let handle = match opened {
            Ok(handle) => handle,
            Err(error) => {
                self.fail(error, cx);
                return;
            }
        };
        if self
            .state
            .tab_window(id)
            .is_none_or(|(_, _, window)| window.is_floating())
        {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            });
            return;
        }
        self.tab_windows.open.insert(id, handle);
        self.tab_windows.focus.insert(id);
        self.tab_windows.key = Some(id);
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        });
        cx.notify();
    }

    fn main_bounds(&self, cx: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        self.window.update(cx, |_, window, _| window.bounds()).ok()
    }

    pub(crate) fn tab_window_title(&self, tab: TabId, cx: &App) -> String {
        let Some(project) = self.tab_project(tab) else {
            return String::new();
        };
        let Some(tab) = self.tab(tab) else {
            return String::new();
        };
        let title = self.webview_title(tab, cx);
        if title.is_empty() {
            project.name.clone()
        } else {
            format!("{title} — {}", project.name)
        }
    }

    pub(crate) fn tab_window_activated(
        &mut self,
        id: TabWindowId,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        if active {
            self.tab_windows.key = Some(id);
            self.tab_windows.focus.insert(id);
            if self.path.with_file_name("terminal.toml").exists() {
                self.reload_configuration(cx);
            }
            self.acknowledge_focused_activity(cx);
        } else if self.tab_windows.key == Some(id) {
            self.tab_windows.key = None;
            self.cancel_tab_and_layout_drags(cx);
        }
        if let Some(pane) = self
            .state
            .tab_window(id)
            .and_then(|(_, _, window)| window.active_pane())
            .and_then(|pane| self.terminal(&pane))
        {
            pane.view
                .update(cx, |pane, cx| pane.focus_changed(active, cx));
        }
        cx.notify();
    }

    pub(crate) fn tab_window_moved(
        &mut self,
        id: TabWindowId,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = window.window_bounds().get_bounds();
        let bounds = WindowBounds {
            x: f64::from(f32::from(bounds.origin.x)),
            y: f64::from(f32::from(bounds.origin.y)),
            width: f64::from(f32::from(bounds.size.width)),
            height: f64::from(f32::from(bounds.size.height)),
        };
        let saved = self
            .state
            .tab_window(id)
            .and_then(|(_, _, window)| window.bounds);
        if saved == Some(bounds) || self.state.set_tab_window_bounds(id, Some(bounds)).is_err() {
            return;
        }
        self.tab_windows.bounds_save = Some(cx.spawn(async move |model, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(200))
                .await;
            let _ = model.update(cx, |model, cx| {
                model.save(cx);
            });
        }));
    }

    pub(crate) fn tab_window_closed(&mut self, id: TabWindowId, cx: &mut Context<Self>) {
        let Some(handle) = self.tab_windows.open.remove(&id) else {
            return;
        };
        self.tab_windows.titles.remove(&id);
        self.tab_windows.focus.remove(&id);
        if self.tab_windows.key == Some(id) {
            self.tab_windows.key = None;
        }
        let window = handle.window_id();
        self.move_drag.zones.forget_window(window);
        self.layout_drag.geometry.clear(window);
        cx.notify();
    }

    pub(crate) fn close_tab_window(&mut self, id: TabWindowId, cx: &mut Context<Self>) {
        if let Some(tab) = self.window_tab(id) {
            self.close_tab(tab, cx);
        }
    }

    pub(crate) fn raise_pane_window(&mut self, pane: PaneId, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.state.pane_window(pane) else {
            return false;
        };
        let _ = self.state.focus_pane(pane);
        if self.is_float(id) {
            self.reveal_float(id, cx);
            let handle = self.window;
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.activate_window());
            });
            return true;
        }
        self.tab_windows.focus.insert(id);
        self.tab_windows.key = Some(id);
        if let Some(handle) = self.tab_windows.handle(id) {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.activate_window());
            });
        }
        cx.notify();
        true
    }

    pub(crate) fn show_main_window(&mut self, cx: &mut Context<Self>) {
        if self.surface() != Surface::Main {
            self.raise_main_window(cx);
        }
    }

    pub(crate) fn raise_main_window(&mut self, cx: &mut Context<Self>) {
        self.tab_windows.key = None;
        let handle = self.window;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        });
    }

    pub(crate) fn move_tab_to_window(
        &mut self,
        tab: TabId,
        grab: Option<Point<Pixels>>,
        bounds: Option<WindowBounds>,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.apply_move(
            |state| Ok((state.move_tab_to_window(tab, bounds)?, Vec::new())),
            cx,
        ) {
            self.opened_by_drop(id, grab);
        }
    }

    pub(crate) fn move_pane_to_window(
        &mut self,
        pane: PaneId,
        grab: Option<Point<Pixels>>,
        bounds: Option<WindowBounds>,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.apply_move(
            |state| Ok((state.move_pane_to_window(pane, bounds)?, Vec::new())),
            cx,
        ) {
            self.opened_by_drop(id, grab);
        }
    }

    fn opened_by_drop(&mut self, id: TabWindowId, grab: Option<Point<Pixels>>) {
        if let Some(grab) = grab {
            self.tab_windows.grabs.insert(id, grab);
        }
    }

    pub(crate) fn return_tab_window(
        &mut self,
        id: TabWindowId,
        place: Option<TabPlace>,
        cx: &mut Context<Self>,
    ) {
        let Some(place) = place.or_else(|| {
            self.state
                .tab_window(id)
                .map(|(project, _, _)| TabPlace::end(project.id))
        }) else {
            return;
        };
        if self
            .apply_move(|state| Ok(((), state.return_tab(id, place)?)), cx)
            .is_some()
        {
            self.raise_main_window(cx);
        }
    }

    pub(crate) fn move_tab_to_place(
        &mut self,
        tab: TabId,
        place: TabPlace,
        cx: &mut Context<Self>,
    ) {
        let detached = self.tab(tab).is_some_and(muxy_app_core::Tab::detached);
        if self
            .apply_move(|state| Ok(((), state.move_tab_to(tab, place)?)), cx)
            .is_some()
            && detached
        {
            self.raise_main_window(cx);
        }
    }

    pub(crate) fn move_pane_to_new_tab(
        &mut self,
        pane: PaneId,
        place: TabPlace,
        cx: &mut Context<Self>,
    ) {
        if self
            .apply_move(|state| state.move_pane_to_new_tab(pane, place), cx)
            .is_some()
            && self.surface() != Surface::Main
        {
            self.raise_main_window(cx);
        }
    }

    pub(crate) fn move_pane_into(&mut self, pane: PaneId, drop: PaneDrop, cx: &mut Context<Self>) {
        if self
            .apply_move(|state| Ok(((), state.move_pane_to(pane, drop)?)), cx)
            .is_some()
        {
            match self.state.pane_window(pane) {
                Some(id) if self.is_float(id) => {
                    if self.surface() != Surface::Main {
                        self.raise_main_window(cx);
                    }
                    self.enter_float(id, cx);
                }
                Some(id) => {
                    self.tab_windows.focus.insert(id);
                    if self.surface() != Surface::Tab(id) {
                        self.raise_pane_window(pane, cx);
                    }
                }
                None if self.surface() != Surface::Main => self.raise_main_window(cx),
                None => {}
            }
        }
    }

    pub(crate) fn resend_session_move(
        &mut self,
        moved: SessionMove,
        origin: ProjectId,
        cx: &mut Context<Self>,
    ) {
        self.tab_windows
            .moving
            .insert((moved.server, moved.session), origin);
        if !self.send(
            moved.server,
            Work::MoveSession(moved.session, moved.project),
            cx,
        ) {
            self.tab_windows
                .moving
                .remove(&(moved.server, moved.session));
        }
    }

    pub(crate) fn session_moved(
        &mut self,
        server: ServerId,
        session: SessionId,
        result: Result<(), muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        let origin = self.tab_windows.moving.remove(&(server, session));
        if !matches!(&result, Err(error) if !refused(error)) {
            self.tab_windows.retried.remove(&(server, session));
        }
        let error = match result {
            Ok(()) => {
                self.state.settle_session_move(server, session);
                self.save(cx);
                return;
            }
            Err(error) if refused(&error) => error,
            Err(error) => {
                if let (Some(origin), Some(pending)) =
                    (origin, self.state.pending_session_move(server, session))
                    && self.ready(server)
                    && self.tab_windows.retried.insert((server, session))
                {
                    self.resend_session_move(pending, origin, cx);
                    return;
                }
                self.server_problem(
                    server,
                    tr!(
                        "Could not confirm the terminal’s move to its new project; it is sent again on reconnect: %@",
                        error.to_string()
                    )
                    .to_string(),
                    cx,
                );
                return;
            }
        };
        self.undo_session_move(server, session, origin, cx);
        self.server_problem(
            server,
            tr!(
                "Could not move the terminal to its new project: %@",
                error.to_string()
            )
            .to_string(),
            cx,
        );
    }

    fn undo_session_move(
        &mut self,
        server: ServerId,
        session: SessionId,
        origin: Option<ProjectId>,
        cx: &mut Context<Self>,
    ) {
        let previous = self.state.clone();
        self.state.settle_session_move(server, session);
        let Some(origin) = origin else {
            if !self.save(cx) {
                self.state = previous;
            }
            return;
        };
        let panes: Vec<_> = self.state.session_panes(server, session).collect();
        let mut reverted = true;
        for pane in panes {
            reverted &= self
                .state
                .move_pane_to_new_tab(pane, TabPlace::end(origin))
                .is_ok();
        }
        if !reverted || !self.save(cx) {
            self.state = previous;
        }
        self.refresh_open_contexts(cx);
        self.changed(cx);
    }

    pub(crate) fn move_refusal(&self, moves: &[SessionMove]) -> Option<String> {
        let server = moves.first()?.server;
        if !self.ready(server) {
            return Some(
                tr!("Connect to the server before moving its terminals to another project.")
                    .to_string(),
            );
        }
        (!self.server_supports(server, Feature::SESSION_MOVE)).then(|| {
            tr!("This server can’t move terminals between projects. Update it, then try again.")
                .to_string()
        })
    }

    fn apply_move<T>(
        &mut self,
        edit: impl FnOnce(&mut AppState) -> Result<(T, Vec<SessionMove>), AppError>,
        cx: &mut Context<Self>,
    ) -> Option<T> {
        if self.quitting != Quitting::Idle || self.close_request.is_some() {
            return None;
        }
        let previous = self.state.clone();
        let (value, moves) = match edit(&mut self.state) {
            Ok(result) => result,
            Err(error) => {
                self.state = previous;
                self.fail(error.to_string(), cx);
                return None;
            }
        };
        let starting = self.pending.keys().any(|pane| {
            previous.pane_project(*pane).map(|project| project.id)
                != self.state.pane_project(*pane).map(|project| project.id)
        });
        if starting {
            self.state = previous;
            self.fail(
                tr!("Wait for the terminal to start before moving it to another project.")
                    .to_string(),
                cx,
            );
            return None;
        }
        let mut moves = moves;
        let mut seen = HashSet::new();
        moves.retain(|moved| seen.insert((moved.server, moved.session)));
        if moves.iter().any(|moved| {
            self.tab_windows
                .moving
                .contains_key(&(moved.server, moved.session))
                || previous
                    .pending_session_move(moved.server, moved.session)
                    .is_some()
        }) {
            self.state = previous;
            self.fail(
                tr!("Wait for this terminal’s last move to finish, then try again.").to_string(),
                cx,
            );
            return None;
        }
        let shared = moves.iter().any(|moved| {
            self.state
                .session_panes(moved.server, moved.session)
                .any(|pane| {
                    self.state.pane_project(pane).map(|project| project.id) != Some(moved.project)
                })
        });
        if shared {
            self.state = previous;
            self.fail(
                tr!("Another pane shows this terminal too. Move both together.").to_string(),
                cx,
            );
            return None;
        }
        if let Some(refusal) = self.move_refusal(&moves) {
            self.state = previous;
            self.fail(refusal, cx);
            return None;
        }
        self.state.record_session_moves(&moves);
        if !self.save(cx) {
            self.state = previous;
            return None;
        }
        for moved in moves {
            if let Some(origin) = previous
                .session_panes(moved.server, moved.session)
                .next()
                .and_then(|pane| previous.pane_project(pane))
            {
                self.tab_windows
                    .moving
                    .insert((moved.server, moved.session), origin.id);
            }
            if !self.send(
                moved.server,
                Work::MoveSession(moved.session, moved.project),
                cx,
            ) {
                let origin = self
                    .tab_windows
                    .moving
                    .remove(&(moved.server, moved.session));
                self.undo_session_move(moved.server, moved.session, origin, cx);
                self.fail(
                    tr!("Connect to the server before moving its terminals to another project.")
                        .to_string(),
                    cx,
                );
            }
        }
        self.refresh_open_contexts(cx);
        self.cancel_tab_and_layout_drags(cx);
        self.changed(cx);
        self.focus_owner.request_active_pane();
        Some(value)
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Saved coordinates are checked against the GPUI pixel range"
)]
fn gpui_bounds(bounds: WindowBounds) -> Option<Bounds<Pixels>> {
    [bounds.x, bounds.y, bounds.width, bounds.height]
        .into_iter()
        .all(|value| value.abs() <= f64::from(f32::MAX))
        .then(|| {
            Bounds::new(
                point(px(bounds.x as f32), px(bounds.y as f32)),
                size(
                    px((bounds.width as f32).max(MIN_SIZE.0)),
                    px((bounds.height as f32).max(MIN_SIZE.1)),
                ),
            )
        })
}

fn default_bounds(main: Option<Bounds<Pixels>>) -> Bounds<Pixels> {
    let origin = main.map_or_else(
        || point(px(120.0), px(120.0)),
        |main| main.origin + point(px(48.0), px(48.0)),
    );
    Bounds::new(origin, size(px(DEFAULT_SIZE.0), px(DEFAULT_SIZE.1)))
}

pub(crate) fn dropped_bounds(size: gpui::Size<Pixels>) -> WindowBounds {
    WindowBounds {
        x: 120.0,
        y: 120.0,
        width: f64::from(f32::from(size.width).max(MIN_SIZE.0)),
        height: f64::from(f32::from(size.height).max(MIN_SIZE.1)),
    }
}

fn open_tab_window(model: &gpui::WeakEntity<AppModel>, id: TabWindowId, cx: &mut App) {
    let Ok(Some((options, grab))) = model.update(cx, |model, cx| model.tab_window_options(id, cx))
    else {
        return;
    };
    let owner = model.clone();
    let opened = cx
        .open_window(options, move |window, cx| {
            let closing = owner.clone();
            window.on_window_should_close(cx, move |_, cx| {
                let closing = closing.clone();
                cx.defer(move |cx| {
                    let _ = closing.update(cx, |model, cx| model.close_tab_window(id, cx));
                });
                false
            });
            if let Some(grab) = grab
                && let Some(cursor) = crate::views::move_drag::cursor()
            {
                muxy_ui::screen::move_window_to(window, cursor, grab);
                muxy_ui::screen::keep_on_screen(window);
            }
            cx.new(|cx| TabWindowView::new(owner, id, window, cx))
        })
        .map_err(|error| error.to_string());
    let _ = model.update(cx, |model, cx| model.tab_window_opened(id, opened, cx));
}

fn refused(error: &muxy_client::ClientError) -> bool {
    matches!(
        error,
        muxy_client::ClientError::Server(_)
            | muxy_client::ClientError::Invalid(_)
            | muxy_client::ClientError::UnexpectedReply(_)
    )
}
