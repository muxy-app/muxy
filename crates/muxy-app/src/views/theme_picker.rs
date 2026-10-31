use std::rc::Rc;

use gpui::Context;
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::tr;

use super::command_palette::Handler;
use super::settings::Change;
use crate::model::AppModel;

pub(crate) const PAGE_ID: &str = "change_theme";
pub(crate) const QUICK_PAGE_ID: &str = "change_quick_theme";
/// The Quick Terminal entry that follows the app's theme. No theme file can be named this.
pub(crate) const SAME_AS_APP: &str = "\0same-as-app";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ThemeTarget {
    App,
    QuickTerminal,
}

impl ThemeTarget {
    pub(crate) fn page_id(self) -> &'static str {
        match self {
            Self::App => PAGE_ID,
            Self::QuickTerminal => QUICK_PAGE_ID,
        }
    }
}

/// The id of the entry to mark as current.
pub(crate) fn current(model: &AppModel, target: ThemeTarget, dark: bool) -> String {
    match target {
        ThemeTarget::App => model.themes.active_name(model.appearance.theme(dark), dark),
        ThemeTarget::QuickTerminal => model.settings.quick_terminal.theme(dark).map_or_else(
            || SAME_AS_APP.to_owned(),
            |name| model.themes.active_name(name, dark),
        ),
    }
}

/// `None` picks Same as App for the Quick Terminal.
fn select(target: ThemeTarget, dark: bool, name: Option<String>) -> Handler {
    Rc::new(move |model, _, cx| {
        let change = match target {
            ThemeTarget::App => Change::Theme(dark, name.clone().unwrap_or_default()),
            ThemeTarget::QuickTerminal => {
                let mut quick = model.settings.quick_terminal.clone();
                quick.set_theme(dark, name.clone());
                Change::QuickTerminal(quick)
            }
        };
        model.change_preference(change, cx);
    })
}

pub(crate) fn command(
    model: &AppModel,
    target: ThemeTarget,
    dark: bool,
    cx: &Context<AppModel>,
) -> Command<Handler> {
    let mut entries = model.themes.entries.clone();
    entries.sort_by_cached_key(|entry| (entry.name.to_lowercase(), entry.name.clone()));
    let model = cx.weak_entity();
    Command::list(target.page_id(), tr!("Change Theme…"), move |cx| {
        let mut themes = Registry::default();
        let Some(model) = model.upgrade() else {
            return themes;
        };
        let active = current(model.read(cx), target, dark);
        if target == ThemeTarget::QuickTerminal {
            themes.register(
                Command::new(SAME_AS_APP, tr!("Same as App"), select(target, dark, None))
                    .current(active == SAME_AS_APP)
                    .keep_open(),
            );
        }
        for entry in &entries {
            themes.register(
                Command::new(
                    entry.name.clone(),
                    entry.name.clone(),
                    select(target, dark, Some(entry.name.clone())),
                )
                .current(entry.name == active)
                .keep_open()
                .swatches(
                    (0..16)
                        .filter_map(|slot| entry.scheme.palette_color(slot).map(Into::into))
                        .collect(),
                ),
            );
        }
        themes
    })
    .keywords("Change Theme")
}
