//! The apps the titlebar's Open in IDE control opens the current project in.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{Context, Window};
use muxy_app_core::ProjectStatus;
use muxy_ui::popover::PopoverAnchor;
use muxy_ui::tr;

use super::AppModel;
use crate::opener::ProjectApp;
use crate::views::settings::Change;

/// Opens a project folder with the app of an id.
pub(crate) type Open = Arc<dyn Fn(&Path, &str) -> io::Result<()> + Send + Sync>;

/// An app with its icon, ready to draw.
#[derive(Clone)]
pub(crate) struct Listed {
    pub(crate) app: ProjectApp,
    pub(crate) icon: Option<Arc<gpui::RenderImage>>,
}

impl AsRef<ProjectApp> for Listed {
    fn as_ref(&self) -> &ProjectApp {
        &self.app
    }
}

pub(crate) struct ProjectApps {
    /// The file manager and the installed editors, once they are found.
    pub(crate) listed: Option<Vec<Listed>>,
    /// Where the control is drawn, so its menu drops down from it.
    pub(crate) anchor: PopoverAnchor,
    pub(crate) open: Open,
}

impl Default for ProjectApps {
    fn default() -> Self {
        Self {
            listed: None,
            anchor: PopoverAnchor::default(),
            open: Arc::new(crate::opener::open_project),
        }
    }
}

/// Icons are drawn this many pixels square, enough for a 16 point icon on a
/// Retina display.
#[cfg(all(target_os = "macos", not(test)))]
const ICON_PIXELS: u32 = 64;

#[cfg(not(test))]
fn icon(app: &ProjectApp) -> Option<Arc<gpui::RenderImage>> {
    #[cfg(target_os = "macos")]
    {
        let bgra = muxy_ui::app_icon::bgra(&app.path, ICON_PIXELS)?;
        let pixels = image::RgbaImage::from_raw(ICON_PIXELS, ICON_PIXELS, bgra)?;
        Some(Arc::new(gpui::RenderImage::new([image::Frame::new(
            pixels,
        )])))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        None
    }
}

impl AppModel {
    /// Looks for the installed editors and their icons in the background.
    #[cfg(not(test))]
    pub(super) fn discover_project_apps(cx: &mut Context<Self>) {
        let found = cx.background_executor().spawn(async {
            crate::opener::project_apps()
                .into_iter()
                .map(|app| Listed {
                    icon: icon(&app),
                    app,
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |model, cx| {
            let listed = found.await;
            let _ = model.update(cx, |model, cx| {
                model.project_apps.listed = Some(listed);
                cx.notify();
            });
        })
        .detach();
    }

    /// The current project's folder, when it is on this computer and there.
    pub(crate) fn project_apps_directory(&self) -> Option<PathBuf> {
        let project = self.state.current_project();
        (project.server_id.is_local() && project.status() == ProjectStatus::Available)
            .then(|| project.directory.clone())
    }

    /// The app the control's main button opens.
    pub(crate) fn default_project_app(&self) -> Option<&Listed> {
        crate::opener::default_project_app(
            self.project_apps.listed.as_deref()?,
            self.settings.openers.project_target.as_deref(),
        )
    }

    /// Opens the current project in the app at `index` of the list, which
    /// becomes the one the main button opens.
    pub(crate) fn open_project_in(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(app) = self
            .project_apps
            .listed
            .as_ref()
            .and_then(|listed| listed.get(index))
            .map(|listed| listed.app.id.clone())
        else {
            return;
        };
        if self.project_apps_directory().is_none() {
            return;
        }
        if self.settings.openers.project_target.as_deref() != Some(app.as_str()) {
            self.change_preference(Change::ProjectTarget(app.clone()), cx);
        }
        self.open_project_with(app, cx);
    }

    /// Opens the current project in the app the main button shows, leaving
    /// the saved choice alone: an app that wasn't found this time stays chosen.
    pub(crate) fn open_project_in_default(&mut self, cx: &mut Context<Self>) {
        if let Some(app) = self
            .default_project_app()
            .map(|listed| listed.app.id.clone())
        {
            self.open_project_with(app, cx);
        }
    }

    fn open_project_with(&mut self, app: String, cx: &mut Context<Self>) {
        let Some(directory) = self.project_apps_directory() else {
            return;
        };
        let open = Arc::clone(&self.project_apps.open);
        let result = crate::opener::submit(move || open(&directory, &app));
        let failed = |model: &mut Self, error: String, cx: &mut Context<Self>| {
            model.forget_uninstalled_project_apps();
            model.fail(tr!("Could not open project: %@", error).to_string(), cx);
        };
        match result {
            Ok(result) => cx
                .spawn(async move |model, cx| {
                    if let Ok(Err(error)) = result.recv().await {
                        let _ = model.update(cx, |model, cx| {
                            failed(model, error.to_string(), cx);
                        });
                    }
                })
                .detach(),
            Err(error) => failed(self, error.to_string(), cx),
        }
    }

    /// Drops editors deleted since they were found, so the main button and
    /// the menu offer only apps that are still installed.
    fn forget_uninstalled_project_apps(&mut self) {
        if let Some(listed) = &mut self.project_apps.listed {
            listed.retain(|listed| listed.app.is_file_manager() || listed.app.path.exists());
        }
    }

    /// Drops the list of apps down from the control.
    pub(crate) fn open_project_apps_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::views::menu::{Command, Item, MenuIcon};
        use crate::views::overlays::Overlay;
        let (Some(_), Some(listed)) = (
            self.project_apps_directory(),
            self.project_apps.listed.as_ref(),
        ) else {
            return;
        };
        let item = |index: usize, listed: &Listed| {
            Item::action(listed.app.name.clone(), Command::OpenProjectIn(index)).icon(
                listed.icon.clone().map_or_else(
                    || {
                        MenuIcon::Glyph(if listed.app.is_file_manager() {
                            muxy_ui::icon::Icon::Folder
                        } else {
                            muxy_ui::icon::Icon::Code
                        })
                    },
                    MenuIcon::Image,
                ),
            )
        };
        let mut items: Vec<_> = listed
            .iter()
            .enumerate()
            .filter(|(_, listed)| listed.app.is_file_manager())
            .map(|(index, listed)| item(index, listed))
            .collect();
        let mut editors: Vec<_> = listed
            .iter()
            .enumerate()
            .filter(|(_, listed)| !listed.app.is_file_manager())
            .collect();
        editors.sort_by_key(|(_, listed)| listed.app.name.to_lowercase());
        if !editors.is_empty() {
            items.push(Item::heading(tr!("Editors & IDEs")).separated());
            items.extend(
                editors
                    .into_iter()
                    .map(|(index, listed)| item(index, listed)),
            );
        }
        let anchor = self.project_apps.anchor.get();
        let position = anchor.map_or_else(
            || gpui::point(gpui::px(8.0), gpui::px(40.0)),
            |anchor| anchor.bottom_left(),
        );
        self.open_menu(items, position, window, cx);
        if let Some(Overlay::Menu(menu)) = &mut self.overlay {
            menu.drops_from = anchor;
        }
    }
}
