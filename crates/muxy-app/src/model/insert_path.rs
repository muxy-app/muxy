use gpui::Context;
use muxy_app_core::{PaneId, ServerId};
use muxy_protocol::{ErrorCode, ServerPath};
use muxy_ui::tr;

use super::AppModel;
use crate::views::terminal::insert_path::{Entry, Lister, Source};

impl AppModel {
    pub(crate) fn insert_path(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if self.overlay.is_some() {
            return;
        }
        if let Some(pane) = self.active_pane().and_then(|id| self.terminal(&id)) {
            pane.view.update(cx, |pane, cx| {
                pane.open_insert_path(&self.theme, self.metrics, window, cx);
            });
        }
    }

    pub(super) fn browse_paths(&mut self, id: PaneId, cx: &mut Context<Self>) {
        let Some(view) = self.terminal(&id).map(|pane| pane.view.clone()) else {
            return;
        };
        let server = view
            .read(cx)
            .attachment()
            .map(|(server, _)| server)
            .or_else(|| self.state.pane_server(id))
            .unwrap_or_else(ServerId::local);
        let source = self.path_source(server, cx);
        view.update(cx, |pane, cx| pane.browse_paths(source, cx));
    }

    fn path_source(&mut self, server: ServerId, cx: &mut Context<Self>) -> Source {
        let name = self.server_label(server);
        let list = if !self.ready(server) {
            Err(tr!("%@ isn't connected.", &name).to_string())
        } else if let Some(client) = self.extensions.client(server) {
            Ok(lister(client, name))
        } else {
            self.extension_client(server, cx);
            Err(tr!("Still connecting to %@. Try again in a moment.", &name).to_string())
        };
        let recents = if self.settings.panes.insert_path_recents {
            self.state.recent_paths(server).to_vec()
        } else {
            Vec::new()
        };
        Source {
            list,
            home: self
                .state
                .server_home(server)
                .and_then(|home| home.directory.to_str().map(str::to_owned)),
            recents,
        }
    }

    pub(super) fn remember_chosen_path(&mut self, id: PaneId, path: &str, cx: &mut Context<Self>) {
        if !self.settings.panes.insert_path_recents {
            return;
        }
        let server = self
            .terminal(&id)
            .and_then(|pane| pane.view.read(cx).attachment())
            .map(|(server, _)| server);
        if let Some(server) = server
            && self.state.remember_path(server, path)
        {
            self.save(cx);
        }
    }
}

fn lister(client: muxy_client::Client, name: String) -> Lister {
    std::sync::Arc::new(
        move |directory| match client.list_entries(ServerPath::from(directory)) {
            Ok(entries) => Ok(entries
                .into_iter()
                .filter_map(|entry| {
                    Some(Entry {
                        name: String::from_utf8(entry.name.0).ok()?,
                        is_directory: entry.is_directory,
                    })
                })
                .collect()),
            Err(muxy_client::ClientError::Server(error))
                if error.code == ErrorCode::Unsupported =>
            {
                Err(tr!("Update Muxy on %@ to browse its files.", &name).to_string())
            }
            Err(muxy_client::ClientError::Server(error)) => Err(error.message),
            Err(error) => Err(error.to_string()),
        },
    )
}
