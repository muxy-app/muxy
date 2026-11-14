use gpui::{AnyElement, AppContext, Context, Entity, Focusable, IntoElement, Window};
use muxy_app_core::opener::{FileHandler, FileRule, OpenRequest, file_pattern, matches_pattern};
use muxy_ui::picker::{Picker, PickerAction, PickerConfig, PickerEvent, PickerItem, PickerRow};
use muxy_ui::tr;

use super::overlays::{Mark, Overlay, OverlayView};
use crate::model::{AppModel, preferences::Change};

pub(crate) struct FileChooser {
    pub(crate) picker: Entity<Picker>,
    request: OpenRequest,
    relative: String,
    handlers: Vec<FileHandler>,
    pattern: String,
    remember: bool,
}

impl OverlayView for FileChooser {
    fn render(
        &self,
        _: Option<Mark>,
        _: &AppModel,
        _: &Window,
        _: &mut Context<AppModel>,
    ) -> AnyElement {
        self.picker.clone().into_any_element()
    }
}

impl AppModel {
    pub(crate) fn open_file_chooser(
        &mut self,
        request: OpenRequest,
        relative: String,
        handlers: Vec<FileHandler>,
        cx: &mut Context<Self>,
    ) {
        if self.dialogs.is_open() {
            return;
        }
        let config = PickerConfig::new("file-opener", tr!("Open %@ with…", &relative));
        let picker = cx.new(|cx| Picker::new(config, self.theme.clone(), self.metrics, cx));
        self.focus_owner.request(&picker.focus_handle(cx));
        self.overlay_subscription =
            Some(cx.subscribe(&picker, |model, _, event, cx| match event {
                PickerEvent::Confirmed(selection) => model.choose_file_opener(&selection.id, cx),
                PickerEvent::QueryChanged { .. } => model.update_file_chooser(cx),
                PickerEvent::FooterAction(action) if action.as_ref() == "remember" => {
                    if let Some(Overlay::FileOpener(open)) = &mut model.overlay {
                        open.remember = !open.remember;
                        open.picker.update(cx, Picker::focus_query);
                    }
                    model.update_file_chooser(cx);
                }
                PickerEvent::Dismissed => model.dismiss_overlay(cx),
                _ => (),
            }));
        let mut patterns = handlers.clone();
        patterns.push(FileHandler {
            id: String::new(),
            title: String::new(),
            patterns: self
                .settings
                .openers
                .file_rules
                .iter()
                .map(|rule| rule.pattern.clone())
                .collect(),
        });
        let pattern = file_pattern(&relative, &patterns);
        self.open_overlay(
            Overlay::FileOpener(Box::new(FileChooser {
                picker,
                request,
                relative,
                handlers,
                pattern,
                remember: false,
            })),
            cx,
        );
        self.update_file_chooser(cx);
        cx.notify();
    }

    fn update_file_chooser(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::FileOpener(open)) = &self.overlay else {
            return;
        };
        let query = open.picker.read(cx).query().to_lowercase();
        let mut handlers = open.handlers.clone();
        handlers.push(FileHandler {
            id: "system.finder".into(),
            title: if open.request.context.server.is_local() {
                muxy_ui::l10n::translate(crate::opener::FILE_MANAGER).to_string()
            } else {
                tr!("Remote file actions").to_string()
            },
            patterns: Vec::new(),
        });
        let items = handlers
            .into_iter()
            .filter(|handler| {
                query
                    .split_whitespace()
                    .all(|word| handler.title.to_lowercase().contains(word))
            })
            .map(|handler| {
                let mut row = PickerRow::new(handler.id, handler.title);
                row.detail = Some(open.relative.clone().into());
                PickerItem::Row(row)
            })
            .collect();
        let label = if open.remember {
            tr!("Always use for %@: On", &open.pattern)
        } else {
            tr!("Always use for %@: Off", &open.pattern)
        };
        open.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_footer_actions(vec![PickerAction::new("remember", label)], cx);
        });
        cx.notify();
    }

    pub(crate) fn choose_file_opener(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(Overlay::FileOpener(open)) = &self.overlay else {
            return;
        };
        let (request, relative, pattern, remember) = (
            open.request.clone(),
            open.relative.clone(),
            open.pattern.clone(),
            open.remember,
        );
        let available = id == "system.finder"
            || self.extension_file_openers().iter().any(|handler| {
                handler.id == id
                    && (handler
                        .patterns
                        .iter()
                        .any(|pattern| matches_pattern(pattern, &relative))
                        || self.settings.openers.file_rules.iter().any(|rule| {
                            rule.opener == id && matches_pattern(&rule.pattern, &relative)
                        }))
            });
        self.dismiss_overlay(cx);
        if !self.file_request_valid(&request) {
            return;
        }
        if !available {
            self.route_terminal_file(request, Some(relative), cx);
            return;
        }
        if remember {
            self.change_preference(
                Change::FileRule {
                    previous: None,
                    rule: FileRule {
                        pattern,
                        opener: id.to_owned(),
                    },
                },
                cx,
            );
        }
        self.open_file_with(&request, &relative, id, cx);
    }
}
