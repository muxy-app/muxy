//! The app's one proxy: which terminals start with it, and keeping the app's
//! own requests and ssh on it while it applies to the whole app.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use gpui::Context;
use muxy_app_core::settings::ProxySettings;
use muxy_app_core::{PaneId, ServerId};
use muxy_protocol::Feature;
use muxy_ui::tr;

use super::{AppModel, ConnectionState};

pub(crate) struct ProxyState {
    pub(crate) settings: Option<ProxySettings>,
    /// Why `proxy.toml` couldn't be read.
    pub(crate) error: Option<String>,
    pub(crate) anchor: muxy_ui::popover::PopoverAnchor,
    path: PathBuf,
    /// Whether the terminal a pane is about to start uses the proxy, as Apply
    /// or Remove Proxy chose. Other new terminals follow the whole-app choice.
    pub(super) choices: HashMap<PaneId, bool>,
}

impl ProxyState {
    pub(super) fn load(path: PathBuf) -> Self {
        let (settings, error) = match ProxySettings::load(&path) {
            Ok(settings) => (settings, None),
            Err(error) => (None, Some(error.to_string())),
        };
        crate::proxy::use_for_app(settings.as_ref());
        Self {
            settings,
            error,
            anchor: std::rc::Rc::default(),
            path,
            choices: HashMap::new(),
        }
    }

    pub(crate) fn global(&self) -> bool {
        self.settings
            .as_ref()
            .is_some_and(|settings| settings.global)
    }
}

impl AppModel {
    /// Saves the proxy, or removes it with `None`, and applies it to the app's
    /// own requests and to ssh. Running terminals keep what they started with.
    pub(crate) fn set_proxy(
        &mut self,
        settings: Option<ProxySettings>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match &settings {
            Some(settings) => settings.save(&self.proxy.path),
            None => ProxySettings::remove(&self.proxy.path),
        }
        .map_err(|error| error.to_string())?;
        self.proxy.settings = settings;
        self.proxy.error = None;
        crate::proxy::use_for_app(self.proxy.settings.as_ref());
        self.sync_ssh_proxy(cx);
        cx.notify();
        Ok(())
    }

    /// ssh goes through the proxy while it applies to the whole app. A server
    /// reached a new way reconnects if it was connected; its terminals keep
    /// running there.
    pub(super) fn sync_ssh_proxy(&mut self, cx: &mut Context<Self>) {
        let proxy = match self
            .proxy
            .settings
            .as_ref()
            .filter(|settings| settings.global)
            .map(crate::proxy::ssh_command)
            .transpose()
        {
            Ok(proxy) => proxy,
            Err(error) => {
                self.fail(
                    tr!("Could not use the proxy for SSH: %@", error.to_string()).to_string(),
                    cx,
                );
                None
            }
        };
        if self.servers.proxy == proxy {
            return;
        }
        self.servers.proxy = proxy;
        for entry in self.settings.servers.clone() {
            let target = self.servers.target(&entry).ok();
            let Some(runtime) = self.servers.get(entry.id) else {
                continue;
            };
            if runtime.target == target {
                continue;
            }
            let connected = runtime.connection != ConnectionState::Disconnected;
            self.disconnect(entry.id, cx);
            self.servers.restart(&entry);
            if connected {
                self.connect_server(entry.id, cx);
            }
        }
    }

    /// This computer's server is connected but too old to start terminals
    /// with the proxy, until it is restarted.
    pub(crate) fn proxy_needs_newer_server(&self) -> bool {
        let local = ServerId::local();
        self.ready(local) && !self.server_supports(local, Feature::SESSION_ENVIRONMENT)
    }

    pub(crate) fn server_supports(&self, server: ServerId, feature: Feature) -> bool {
        self.servers
            .get(server)
            .is_some_and(|runtime| runtime.features.contains(&feature))
    }

    /// Whether `pane`'s terminal can start again with the proxy, or without it.
    pub(crate) fn can_restart_terminal(&self, pane: PaneId, proxy: bool) -> bool {
        let server = self.state.pane_server(pane).unwrap_or_else(ServerId::local);
        self.can_detach_terminal(pane)
            && !self.is_quick_terminal(pane)
            && self.ready(server)
            && (!proxy
                || self.proxy.settings.is_some()
                    && self.server_supports(server, Feature::SESSION_ENVIRONMENT))
    }

    /// Ends `pane`'s terminal and starts a new one in its place and folder,
    /// with the proxy or without it. Asks first if a program is running.
    pub(crate) fn restart_terminal(&mut self, pane: PaneId, proxy: bool, cx: &mut Context<Self>) {
        if !self.can_restart_terminal(pane, proxy) {
            return;
        }
        if let Some(tab) = self.pane_tab(pane) {
            self.begin_close_tabs(vec![tab], Some(pane), Some(proxy), cx);
        }
    }

    /// The environment a new terminal for `pane` starts with. Notes on the
    /// pane whether that includes the proxy, before the layout is saved.
    pub(super) fn creation_environment(
        &mut self,
        pane: PaneId,
        server: ServerId,
    ) -> BTreeMap<String, String> {
        let proxy = self
            .proxy
            .choices
            .get(&pane)
            .copied()
            .unwrap_or_else(|| server.is_local() && self.proxy.global());
        let settings = self
            .proxy
            .settings
            .as_ref()
            .filter(|_| proxy && self.server_supports(server, Feature::SESSION_ENVIRONMENT));
        let environment = settings.map(ProxySettings::environment).unwrap_or_default();
        let _ = self.state.set_pane_proxy(pane, settings.is_some());
        environment
    }

    /// The terminal menu's way to the proxy for `pane`.
    pub(super) fn proxy_menu_item(&self, pane: PaneId) -> crate::views::menu::Item {
        use crate::views::menu::{Command, Item};
        let proxied = self.pane_uses_proxy(pane);
        if !proxied && self.proxy.settings.is_none() {
            return Item::action(tr!("Set Up Proxy…"), Command::ProxySettings);
        }
        let item = Item::action(
            if proxied {
                tr!("Remove Proxy")
            } else {
                tr!("Apply Proxy")
            },
            Command::RestartWithProxy(pane, !proxied),
        );
        if self.can_restart_terminal(pane, !proxied) {
            item
        } else {
            item.disabled()
        }
    }

    pub(crate) fn pane_uses_proxy(&self, pane: PaneId) -> bool {
        self.state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .any(|candidate| candidate.id == pane && candidate.proxy)
    }
}
