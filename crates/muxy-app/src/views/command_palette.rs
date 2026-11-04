use std::rc::Rc;

use gpui::{AnyElement, AppContext, Context, Entity, Focusable, IntoElement, Window};
use muxy_core::shortcuts::ShortcutId;
use muxy_ui::command_palette::{Command, CommandPalette, CommandPaletteEvent, Registry};

use super::overlays::{Mark, Overlay, OverlayView};
use crate::model::AppModel;

pub(crate) type Handler = Rc<dyn Fn(&mut AppModel, &mut Window, &mut Context<AppModel>)>;

/// The open command palette, or a picker shown in it like the theme picker.
pub(crate) struct Palette {
    pub(crate) palette: Entity<CommandPalette<Handler>>,
    /// The appearance its theme picker previews.
    pub(crate) dark: bool,
}

impl OverlayView for Palette {
    fn render(
        &self,
        _: Option<Mark>,
        _: &AppModel,
        _: &Window,
        _: &mut Context<AppModel>,
    ) -> AnyElement {
        self.palette.clone().into_any_element()
    }
}

/// `command` showing the shortcut the user's keymap gives `id`.
pub(crate) fn with_shortcut(
    model: &AppModel,
    id: ShortcutId,
    command: Command<Handler>,
) -> Command<Handler> {
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
        super::workspace::commands::register_listed(&mut registry, self);
        super::sidebar::register_commands(&mut registry, self, cx);
        super::tips::register_commands(&mut registry, self);
        super::settings::register_commands(&mut registry, self);
        registry.register(super::theme_picker::command(
            self,
            super::theme_picker::ThemeTarget::App,
            dark,
            cx,
        ));
        registry.register(super::remote_servers::command(self, cx));
        self.register_extension_commands(&mut registry);
        self.register_custom_commands(&mut registry);
        registry
    }

    pub(crate) fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialogs.is_open() {
            return;
        }
        if let Some(Overlay::Commands(Palette { palette, .. })) = &self.overlay
            && !palette.read(cx).is_page(super::theme_picker::PAGE_ID)
        {
            self.dismiss_overlay(cx);
            return;
        }
        self.open_command_palette(None, window, cx);
    }

    pub(crate) fn open_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Overlay::Commands(Palette { palette, .. })) = &self.overlay
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
        if self.dialogs.is_open() {
            return;
        }
        let registry = self.command_registry(self.dark, cx);
        self.show_palette(registry, page, window, cx);
    }

    /// Shows `registry` in the palette, opened at its list `page`.
    pub(crate) fn show_palette(
        &mut self,
        registry: Registry<Handler>,
        page: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dialogs.is_open() {
            return;
        }
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
                    let active = model.themes.active_name(model.appearance.theme(dark), dark);
                    palette.update(cx, |palette, cx| palette.set_current(&active, cx));
                    return;
                }
                model.dismiss_overlay(cx);
                model.focus_active(window, cx);
                model.focus_owner.settle();
                if let CommandPaletteEvent::Selected(handler) = event {
                    handler(model, window, cx);
                }
            },
        ));
        palette.focus_handle(cx).focus(window);
        self.open_overlay(Overlay::Commands(Palette { palette, dark }), cx);
        cx.notify();
    }
}
