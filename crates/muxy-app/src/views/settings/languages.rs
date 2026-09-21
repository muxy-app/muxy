use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Subscription, Task, Window,
};
use muxy_ui::picker::{Picker, PickerConfig, PickerEvent, PickerItem, PickerRow, PickerStatus};
use muxy_ui::theme::{Metrics, Theme};
use muxy_ui::voice::Language;

pub(crate) enum LanguageEvent {
    Selected(String),
    Dismiss,
}

pub(crate) struct LanguagePicker {
    picker: Entity<Picker>,
    active: String,
    languages: Option<Vec<Language>>,
    load: Option<Task<()>>,
    _subscription: Subscription,
}

impl EventEmitter<LanguageEvent> for LanguagePicker {}

impl Focusable for LanguagePicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.picker.read(cx).input().focus_handle(cx)
    }
}

impl LanguagePicker {
    pub(crate) fn new(
        active: String,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let picker = cx.new(|cx| {
            let mut picker = Picker::new(
                PickerConfig::dropdown("language-browser", "Search languages…"),
                theme,
                metrics,
                cx,
            );
            picker.set_status(
                PickerStatus::Loading("Loading on-device languages…".into()),
                cx,
            );
            picker
        });
        let subscription = cx.subscribe(&picker, |browser: &mut Self, _, event, cx| match event {
            PickerEvent::QueryChanged { query, .. } => browser.sync_picker(query, cx),
            PickerEvent::Confirmed(selection) | PickerEvent::SecondaryConfirmed(selection) => {
                if selection.id == "system-language" {
                    cx.emit(LanguageEvent::Selected(String::new()));
                } else if let Some(language) = browser.languages.as_ref().and_then(|languages| {
                    languages
                        .iter()
                        .find(|language| language.id == selection.id.as_ref())
                }) {
                    cx.emit(LanguageEvent::Selected(language.id.clone()));
                }
            }
            PickerEvent::Dismissed => cx.emit(LanguageEvent::Dismiss),
            _ => {}
        });
        let load = cx.spawn(async move |browser, cx| {
            let languages = cx
                .background_executor()
                .spawn(async { muxy_ui::voice::languages() })
                .await;
            let _ = browser.update(cx, |browser, cx| browser.set_languages(languages, cx));
        });
        Self {
            picker,
            active,
            languages: None,
            load: Some(load),
            _subscription: subscription,
        }
    }

    pub(crate) fn set_languages(&mut self, languages: Vec<Language>, cx: &mut Context<Self>) {
        self.load = None;
        self.languages = Some(languages);
        let query = self.picker.read(cx).query().to_owned();
        self.sync_picker(&query, cx);
    }

    pub(crate) fn set_appearance(&self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        self.picker
            .update(cx, |picker, cx| picker.set_appearance(theme, metrics, cx));
    }

    fn sync_picker(&self, query: &str, cx: &mut Context<Self>) {
        let Some(languages) = &self.languages else {
            return;
        };
        let query = query.trim().to_lowercase();
        let mut items = Vec::new();
        if "system language".contains(&query) {
            let mut row = PickerRow::new("system-language", "System language");
            row.current = self.active.is_empty();
            items.push(PickerItem::Row(row));
        }
        for language in languages {
            if language.name.to_lowercase().contains(&query)
                || language.id.to_lowercase().contains(&query)
            {
                let mut row = PickerRow::new(language.id.clone(), language.name.clone());
                row.current = language.id == self.active;
                items.push(PickerItem::Row(row));
            }
        }
        let status = if items.is_empty() {
            PickerStatus::Empty("No matching languages".into())
        } else {
            PickerStatus::Ready
        };
        self.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
        });
    }
}

impl Render for LanguagePicker {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.picker.clone()
    }
}
