use std::rc::Rc;

use gpui::{Action, AppContext, Context, Focusable, Window};
use muxy_core::shortcuts::ShortcutId;
use muxy_ui::command_palette::{Command, CommandPalette, CommandPaletteEvent, Registry};

use super::overlays::Overlay;
use crate::model::AppModel;

pub(crate) type Handler = Rc<dyn Fn(&mut AppModel, &mut Window, &mut Context<AppModel>)>;

pub(crate) fn action(
    model: &AppModel,
    id: ShortcutId,
    title: &'static str,
    action: impl Action,
) -> Command<Handler> {
    let handler: Handler = Rc::new(move |_, window, cx| {
        window.dispatch_action(action.boxed_clone(), cx);
    });
    let command = Command::new(id.name(), title, handler);
    match model.settings.keymap.chord(id) {
        Some(chord) => command.shortcut(
            gpui::Keystroke::parse(chord.as_str())
                .map_or_else(|_| chord.to_string(), |key| key.to_string()),
        ),
        None => command,
    }
}

impl AppModel {
    pub(crate) fn command_registry(
        &mut self,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> Registry<Handler> {
        self.reload_themes(cx);
        let mut registry = Registry::default();
        super::workspace::register_commands(&mut registry, self);
        super::sidebar::register_commands(&mut registry, self, cx);
        super::settings::register_commands(&mut registry, self);
        registry.register(super::theme_picker::command(self, dark, cx));
        self.register_extension_commands(&mut registry);
        registry
    }

    pub(crate) fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if let Some(Overlay::Commands { palette, .. }) = &self.overlay
            && !palette.read(cx).is_page(super::theme_picker::PAGE_ID)
        {
            self.dismiss_overlay(cx);
            return;
        }
        self.open_command_palette(None, window, cx);
    }

    pub(crate) fn open_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Overlay::Commands { palette, .. }) = &self.overlay
            && palette.read(cx).is_page(super::theme_picker::PAGE_ID)
        {
            self.dismiss_overlay(cx);
            return;
        }
        self.open_command_palette(Some(super::theme_picker::PAGE_ID), window, cx);
    }

    fn open_command_palette(
        &mut self,
        page: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        let registry = self.command_registry(self.dark, cx);
        let palette =
            cx.new(|cx| CommandPalette::new(registry, self.theme.clone(), self.metrics, cx));
        if let Some(page) = page {
            let page = page.to_owned();
            let palette = palette.clone();
            cx.defer(move |cx| {
                palette.update(cx, |palette, cx| palette.open_root_page(&page, cx));
            });
        }
        let dark = self.dark;
        self.overlay_subscription = Some(cx.subscribe_in(
            &palette,
            window,
            move |model, palette, event, window, cx| {
                if let CommandPaletteEvent::Applied(handler) = event {
                    handler(model, window, cx);
                    let active = model.themes.active_name(&model.appearance, dark);
                    palette.update(cx, |palette, cx| palette.set_current(&active, cx));
                    return;
                }
                model.dismiss_overlay(cx);
                model.focus_active(window, cx);
                model.focus_requested = false;
                if let CommandPaletteEvent::Selected(handler) = event {
                    handler(model, window, cx);
                }
            },
        ));
        palette.focus_handle(cx).focus(window);
        self.overlay = Some(Overlay::Commands { palette, dark });
        cx.notify();
    }
}
