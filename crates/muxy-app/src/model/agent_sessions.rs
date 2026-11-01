mod list;

pub(crate) use list::{Entry, Recency};

use std::path::PathBuf;

use gpui::{AppContext, Context, Entity, FocusHandle, Focusable, Subscription, Task, Window};
use muxy_app_core::agent_sessions::{Catalog, Listing, PROVIDERS, Roots, Session, supported};
use muxy_app_core::settings::AgentSessionsPosition;
use muxy_app_core::{ProjectId, ProjectStatus};
use muxy_protocol::AgentProvider;
use muxy_ui::l10n::tr_key;
use muxy_ui::panel::{PanelId, PanelMode, PanelPlacement, PanelPosition, PanelResizeState};
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use muxy_ui::tr;

use super::{AppModel, ConnectionState, Quitting};
use crate::views::overlays::Overlay;

pub(crate) const PANEL: &str = "builtin:agent-sessions";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Scope {
    pub(crate) project: ProjectId,
    pub(crate) directory: PathBuf,
}

pub(crate) struct AgentSessions {
    pub(crate) input: Option<Entity<TextInput>>,
    pub(crate) controls: [FocusHandle; 5],
    pub(crate) listing: Listing,
    pub(crate) filtered: Vec<usize>,
    pub(crate) scope: Option<Scope>,
    pub(crate) resize: PanelResizeState,
    pub(crate) list: list::SessionList,
    pub(crate) show_unsupported: bool,
    pub(crate) anchor: muxy_ui::popover::PopoverAnchor,
    pub(crate) toggle: muxy_ui::popover::PopoverAnchor,
    pub(crate) highlighted_provider: usize,
    pub(crate) task: Option<Task<()>>,
    pub(crate) roots: Option<Roots>,
    catalog: Catalog,
    subscription: Option<Subscription>,
}

impl AgentSessions {
    pub(crate) fn new(cx: &mut Context<AppModel>) -> Self {
        Self {
            input: None,
            controls: std::array::from_fn(|_| cx.focus_handle()),
            listing: Listing::default(),
            filtered: Vec::new(),
            scope: None,
            resize: PanelResizeState::default(),
            list: list::SessionList::default(),
            show_unsupported: false,
            anchor: muxy_ui::popover::PopoverAnchor::default(),
            toggle: muxy_ui::popover::PopoverAnchor::default(),
            highlighted_provider: 0,
            task: None,
            roots: if cfg!(test) {
                None
            } else {
                Roots::from_environment()
            },
            catalog: Catalog::default(),
            subscription: None,
        }
    }
}

impl AppModel {
    pub(crate) fn agent_sessions_scope(&self) -> Option<Scope> {
        let project = self.state.current_project();
        (self.appearance.agent_sessions_visible
            && project.server_id.is_local()
            && project.status() == ProjectStatus::Available)
            .then(|| Scope {
                project: project.id,
                directory: project.directory.clone(),
            })
    }

    pub(crate) fn toggle_agent_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        self.appearance.agent_sessions_visible = !self.appearance.agent_sessions_visible;
        self.save_appearance(cx);
        self.sync_agent_sessions(cx);
        if self.appearance.agent_sessions_visible {
            if let Some(input) = &self.agent_sessions.input {
                self.focus_owner.place(&input.focus_handle(cx), window);
            }
        } else {
            self.focus_active(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn sync_agent_sessions(&mut self, cx: &mut Context<Self>) {
        let id = PanelId::new(PANEL);
        if self.appearance.agent_sessions_visible {
            let placement = self.agent_sessions_placement();
            if self.panels.placement(&id) != Some(&placement) {
                self.place_panel(placement, cx);
            }
            if self.agent_sessions.input.is_none() {
                let input = cx.new(|cx| {
                    TextInput::new(InputStyle::compact(&self.theme, &self.metrics), cx)
                        .with_placeholder_key(tr_key!("Search sessions…"))
                        .with_key_context(muxy_ui::text_input::BARE_CONTEXT)
                });
                self.agent_sessions.subscription =
                    Some(cx.subscribe(&input, |model, _, event, cx| match event {
                        InputEvent::Changed => model.filter_agent_sessions(cx),
                        InputEvent::Submitted => model.resume_selected_agent_session(cx),
                        InputEvent::Cancelled => {
                            model.focus_owner.request_active_pane();
                            cx.notify();
                        }
                    }));
                self.agent_sessions.input = Some(input);
            }
        } else {
            self.panels.remove(&id);
            self.agent_sessions.resize.end();
            self.agent_sessions.anchor.set(None);
            if let Some(input) = &self.agent_sessions.input {
                self.focus_owner.forget(&input.focus_handle(cx));
            }
            if matches!(self.overlay, Some(Overlay::SessionProviders)) {
                self.dismiss_overlay(cx);
            }
        }
        let scope = self.agent_sessions_scope();
        if self.agent_sessions.scope != scope {
            self.agent_sessions.scope = scope;
            self.agent_sessions.listing = Listing::default();
            self.filter_agent_sessions(cx);
            self.refresh_agent_sessions(cx);
        }
    }

    pub(crate) fn agent_sessions_placement(&self) -> PanelPlacement {
        PanelPlacement::new(
            PANEL,
            match self.appearance.agent_sessions_position {
                AgentSessionsPosition::Right => PanelPosition::Right,
                AgentSessionsPosition::Bottom => PanelPosition::Bottom,
            },
            if self.appearance.agent_sessions_pinned {
                PanelMode::Pinned
            } else {
                PanelMode::Floating
            },
        )
    }

    pub(crate) fn move_agent_sessions(&mut self, toggle_mode: bool, cx: &mut Context<Self>) {
        let appearance = &mut self.appearance;
        if toggle_mode {
            appearance.agent_sessions_pinned = !appearance.agent_sessions_pinned;
        } else {
            appearance.agent_sessions_position = match appearance.agent_sessions_position {
                AgentSessionsPosition::Right => AgentSessionsPosition::Bottom,
                AgentSessionsPosition::Bottom => AgentSessionsPosition::Right,
            };
        }
        self.agent_sessions.resize.end();
        self.save_appearance(cx);
    }

    pub(crate) fn refresh_agent_sessions(&mut self, cx: &mut Context<Self>) {
        if self.agent_sessions.task.is_some() {
            return;
        }
        let (Some(scope), Some(roots)) = (
            self.agent_sessions_scope(),
            self.agent_sessions.roots.clone(),
        ) else {
            return;
        };
        let mut catalog = std::mem::take(&mut self.agent_sessions.catalog);
        let requested = scope.clone();
        let scan = cx.background_executor().spawn(async move {
            let listing = catalog.scan(&roots, &requested.directory);
            (catalog, listing)
        });
        self.agent_sessions.task = Some(cx.spawn(async move |model, cx| {
            let (catalog, listing) = scan.await;
            let _ = model.update(cx, |model, cx| {
                model.agent_sessions.task = None;
                model.agent_sessions.catalog = catalog;
                if model.agent_sessions_scope().as_ref() == Some(&scope) {
                    model.agent_sessions.listing = listing;
                    model.filter_agent_sessions(cx);
                    if model.agent_sessions.listing.pending {
                        model.refresh_agent_sessions(cx);
                    }
                } else {
                    model.refresh_agent_sessions(cx);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn filter_agent_sessions(&mut self, cx: &mut Context<Self>) {
        let query = self
            .agent_sessions
            .input
            .as_ref()
            .map_or("", |input| input.read(cx).text());
        self.agent_sessions.filtered = self.agent_sessions.list.rebuild(
            &self.agent_sessions.listing.sessions,
            &self.appearance.agent_sessions_providers,
            query,
        );
        cx.notify();
    }

    pub(crate) fn clear_session_search(&mut self, cx: &mut Context<Self>) {
        if let Some(input) = &self.agent_sessions.input {
            input.update(cx, |input, cx| input.set_text("", cx));
        }
        self.filter_agent_sessions(cx);
    }

    pub(crate) fn set_session_providers(
        &mut self,
        providers: Vec<AgentProvider>,
        cx: &mut Context<Self>,
    ) {
        self.appearance.agent_sessions_providers = providers
            .into_iter()
            .filter(|provider| supported(*provider))
            .collect();
        self.save_appearance(cx);
    }

    pub(crate) fn all_session_providers(&mut self, cx: &mut Context<Self>) {
        self.set_session_providers(
            PROVIDERS
                .into_iter()
                .filter(|provider| supported(*provider))
                .collect(),
            cx,
        );
    }

    pub(crate) fn resume_selected_agent_session(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.agent_sessions.list.selected_index()
            && let Some(session) = self.agent_sessions.listing.sessions.get(index).cloned()
        {
            self.resume_agent_session(&session, cx);
        }
    }

    pub(crate) fn move_agent_session(&mut self, backwards: bool, cx: &mut Context<Self>) {
        self.agent_sessions.list.advance(backwards);
        cx.notify();
    }

    pub(crate) fn toggle_session_provider(
        &mut self,
        provider: AgentProvider,
        cx: &mut Context<Self>,
    ) {
        if !supported(provider) {
            return;
        }
        let providers = &mut self.appearance.agent_sessions_providers;
        if providers.contains(&provider) {
            providers.retain(|item| *item != provider);
        } else {
            providers.push(provider);
        }
        self.save_appearance(cx);
        self.filter_agent_sessions(cx);
    }

    pub(crate) fn toggle_session_providers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::SessionProviders)) {
            self.dismiss_overlay(cx);
        } else {
            self.dismiss_overlay(cx);
            self.overlay = Some(Overlay::SessionProviders);
            self.agent_sessions.highlighted_provider = 0;
            self.focus_owner.place(&self.overlay_focus, window);
            let input = self
                .agent_sessions
                .input
                .as_ref()
                .map(|input| input.focus_handle(cx));
            self.focus_owner.return_after_overlay(input.as_ref());
            window.prevent_default();
        }
        cx.notify();
    }

    pub(crate) fn resume_agent_session(&mut self, session: &Session, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle
            || self.close_prompt.is_some()
            || self.close_request.is_some()
            || self.pending_close.is_some()
            || self.overlay.is_some()
            || self.agent_sessions.scope.is_none()
            || self.agent_sessions.scope != self.agent_sessions_scope()
            || !self.agent_sessions.listing.sessions.contains(session)
        {
            return;
        }
        let Some(command) = session.resume_command() else {
            return;
        };
        if !session.path.is_file() {
            self.fail(
                tr!("This session is no longer available. Refresh the session list.").to_string(),
                cx,
            );
            return;
        }
        let previous = self.state.clone();
        let result = (|| {
            let tab = self
                .state
                .open_terminal_tab(self.state.current_project().id)?;
            self.state.set_tab_title(tab, Some(session.title.clone()))?;
            let pane = self
                .state
                .window()
                .active_pane
                .ok_or_else(|| "could not create session pane".to_owned())?;
            self.state.set_startup_command(pane, &command)?;
            Ok::<_, Box<dyn std::error::Error>>(pane)
        })();
        let pane = match result {
            Ok(pane) => pane,
            Err(error) => {
                self.state = previous;
                self.fail(error.to_string(), cx);
                return;
            }
        };
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        self.initial_directories
            .insert(pane, self.state.current_project().directory.clone());
        self.changed(cx);
        self.focus_owner.request_active_pane();
        let server = self.state.current_project().server_id;
        if self.connection(server) == ConnectionState::Disconnected {
            self.connect_server(server, cx);
        }
    }
}
