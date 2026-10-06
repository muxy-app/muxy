//! The app language: the language pack chosen in Settings, or English while
//! it is unavailable.

use std::sync::Arc;

use gpui::Context;
use muxy_app_core::localization;
use muxy_ui::tr;

use super::AppModel;

impl AppModel {
    /// Languages the enabled extensions provide, as `(selection, label)`.
    pub(crate) fn app_languages(&self) -> Vec<(String, String)> {
        localization::providers(&self.extensions.registry)
            .into_iter()
            .map(|provider| {
                let label = tr!("%@ — %@", &provider.title, &provider.extension).to_string();
                (provider.selection(), label)
            })
            .collect()
    }

    /// Loads the selected language pack and redraws every window in it.
    /// Before extensions load the selection is unknown, so English stays.
    pub(crate) fn sync_language(&mut self, cx: &mut Context<Self>) {
        if !self.extensions_loaded() {
            return;
        }
        let provider = localization::provider(&self.extensions.registry, &self.appearance.language);
        self.language_task = Some(cx.spawn(async move |model, cx| {
            let loaded = match provider {
                Some(provider) => {
                    let owner = provider.extension.clone();
                    let loaded = cx
                        .background_executor()
                        .spawn(async move { localization::load(&provider) })
                        .await;
                    Some((owner, loaded))
                }
                None => None,
            };
            let _ = model.update(cx, |model, cx| {
                let language = match loaded {
                    Some((_, Ok(language))) => Some(Arc::new(language)),
                    Some((owner, Err(error))) => {
                        model.extension_log(
                            &owner,
                            format!("Language pack failed to load: {error}"),
                        );
                        None
                    }
                    None => None,
                };
                muxy_core::l10n::set_language(language);
                cx.set_menus(crate::menus());
                model.sync_preferences(cx);
                cx.refresh_windows();
            });
        }));
    }
}
