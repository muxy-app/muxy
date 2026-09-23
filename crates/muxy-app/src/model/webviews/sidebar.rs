//! The extension sidebar: an enabled extension's `sidebar` page shown in place
//! of the built-in sidebar when the user selects it in Settings.

use gpui::{Context, Window};
use muxy_ui::webview::assets::Source;

use super::{AppModel, Surface, SurfaceKind};

pub(crate) struct SidebarSurface {
    pub owner: String,
    pub id: String,
    pub surface: Surface,
}

impl AppModel {
    /// Enabled extensions that provide a sidebar, as `(extension, label)`.
    pub(crate) fn extension_sidebars(&self) -> Vec<(String, String)> {
        self.extensions
            .registry
            .active()
            .filter_map(|extension| {
                let sidebar = extension.manifest.sidebar.as_ref()?;
                Some((
                    extension.name.clone(),
                    sidebar
                        .title
                        .clone()
                        .unwrap_or_else(|| extension.name.clone()),
                ))
            })
            .collect()
    }

    /// Whether the selected extension sidebar replaces the built-in one. The
    /// selection is kept while its extension is unavailable, and the built-in
    /// sidebar shows meanwhile.
    pub(crate) fn extension_sidebar_active(&self) -> bool {
        let owner = &self.appearance.extension_sidebar;
        !owner.is_empty()
            && (!self.extensions_loaded()
                || self
                    .extensions
                    .registry
                    .enabled(owner)
                    .is_some_and(|extension| extension.manifest.sidebar.is_some()))
    }

    /// Creates, replaces, or removes the sidebar page to match the selection.
    pub(super) fn sync_extension_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wanted = self
            .extensions
            .registry
            .enabled(&self.appearance.extension_sidebar)
            .and_then(|extension| {
                let sidebar = extension.manifest.sidebar.as_ref()?;
                Some((
                    extension.name.clone(),
                    sidebar.id.clone(),
                    Source {
                        owner: extension.name.clone(),
                        directory: extension.directory.clone(),
                        entry: sidebar.entry.clone(),
                    },
                    sidebar.default_data.clone(),
                ))
            });
        let current = self
            .webviews
            .sidebar
            .as_ref()
            .map(|sidebar| (sidebar.owner.as_str(), sidebar.id.as_str()));
        if current
            == wanted
                .as_ref()
                .map(|(owner, id, ..)| (owner.as_str(), id.as_str()))
        {
            return;
        }
        self.webviews.sidebar = None;
        let Some((owner, id, source, data)) = wanted else {
            self.webviews.sidebar_failure = None;
            return;
        };
        if self.webviews.sidebar_failure.as_ref() == Some(&(owner.clone(), id.clone())) {
            return;
        }
        match self.create_webview(
            source,
            format!("sidebar:{owner}:{id}"),
            data,
            SurfaceKind::Sidebar,
            window,
            cx,
        ) {
            Ok(surface) => {
                self.webviews.sidebar_failure = None;
                self.webviews.sidebar = Some(SidebarSurface { owner, id, surface });
            }
            Err(error) => {
                self.webviews.sidebar_failure = Some((owner, id));
                self.set_banner_error(Some(error));
            }
        }
    }
}
