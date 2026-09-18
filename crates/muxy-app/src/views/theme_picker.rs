use std::rc::Rc;

use gpui::Context;
use muxy_ui::command_palette::{Command, Registry};

use super::command_palette::Handler;
use super::settings::Change;
use crate::model::AppModel;

pub(crate) const PAGE_ID: &str = "change_theme";

pub(crate) fn command(model: &AppModel, dark: bool, cx: &Context<AppModel>) -> Command<Handler> {
    let mut entries = model.themes.entries.clone();
    entries.sort_by_cached_key(|entry| (entry.name.to_lowercase(), entry.name.clone()));
    let model = cx.weak_entity();
    Command::list(PAGE_ID, "Change Theme…", move |cx| {
        let mut themes = Registry::default();
        let Some(model) = model.upgrade() else {
            return themes;
        };
        let model = model.read(cx);
        let active = model.themes.active_name(&model.appearance, dark);
        for entry in &entries {
            let name = entry.name.clone();
            let handler: Handler = Rc::new(move |model, _, cx| {
                model.change_preference(Change::Theme(dark, name.clone()), cx);
            });
            themes.register(
                Command::new(entry.name.clone(), entry.name.clone(), handler)
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
}
