use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, ParentElement, Styled,
    Subscription, Task, Window, div,
};
use muxy_app_core::{
    PaneId,
    composer::{
        ComposerDraft, ComposerStore, DraftId, SAVE_DEBOUNCE,
        image_storage::prepare_image_source,
        submission::{
            ImageSubmissionStrategy, SubmissionPlan, SubmissionSegment, SubmissionSnapshot,
            plan_submission,
        },
    },
};
use muxy_protocol::{ChannelId, Modes};
use muxy_ui::panel::{PanelHost, PanelId};

use super::AppModel;
use crate::{
    boot::Work,
    views::{
        composer::{Composer, ComposerEvent, PANEL},
        native_modal::NativeModal,
        overlays::Overlay,
    },
};

pub(crate) struct ComposerRuntime {
    pub(crate) view: Option<Entity<Composer>>,
    pub(crate) closing: Option<Entity<Composer>>,
    exit: Option<Task<()>>,
    key: Option<DraftId>,
    pane: Option<PaneId>,
    store: Arc<Mutex<ComposerStore>>,
    host: PanelHost,
    subscription: Option<Subscription>,
    save: Option<Task<()>>,
    submission: Option<Task<()>>,
    pub(super) preferences_dirty: bool,
}

impl ComposerRuntime {
    pub(super) fn new(store: ComposerStore) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            view: None,
            closing: None,
            exit: None,
            key: None,
            pane: None,
            host: PanelHost::default(),
            subscription: None,
            save: None,
            submission: None,
            preferences_dirty: false,
        }
    }
}

impl AppModel {
    pub(super) fn composer_key(&self) -> DraftId {
        let project = self.state.current_project();
        DraftId::for_project(project.server_id, project.id)
    }

    pub(crate) fn toggle_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if self.composer.view.is_some() {
            self.close_composer(cx);
            return;
        }
        self.close_voice(cx);
        self.dismiss_overlay(cx);
        self.open_composer(cx);
        if let Some(view) = &self.composer.view {
            view.focus_handle(cx).focus(window);
        }
        self.focus_requested = false;
    }

    fn open_composer(&mut self, cx: &mut Context<Self>) {
        self.composer.closing = None;
        self.composer.exit = None;
        let key = self.composer_key();
        let (draft, error) = {
            let store = self
                .composer
                .store
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            (
                store.draft(&key).cloned().unwrap_or_default(),
                (!store.load_status().is_ready()).then(|| store.load_status().warnings.join("; ")),
            )
        };
        let view = cx.new(|cx| {
            Composer::new(
                draft,
                self.settings.composer.clone(),
                self.theme.clone(),
                self.metrics,
                cx,
            )
        });
        view.update(cx, |view, _| {
            view.error = error;
            view.sending = self.composer.submission.is_some();
        });
        self.composer.host.place(view.read(cx).placement());
        self.composer.subscription = Some(cx.subscribe(&view, |model, _, event, cx| match event {
            ComposerEvent::Changed => model.composer_changed(cx),
            ComposerEvent::Close => model.close_composer(cx),
            ComposerEvent::Submit(enter) => model.submit_composer(*enter, cx),
            ComposerEvent::Attach => model.choose_composer_files(cx),
            ComposerEvent::Files(paths) => model.attach_composer_files(paths.clone(), cx),
            ComposerEvent::Image(bytes) => model.attach_composer_image(bytes.clone(), cx),
            ComposerEvent::Menu => model.composer_menu(cx),
            ComposerEvent::Preferences(settings) => {
                if model.settings.composer.presentation != settings.presentation
                    && let Some(view) = &model.composer.view
                {
                    view.update(cx, |view, cx| {
                        view.visibility = muxy_ui::motion::VisibilityMotion::showing(
                            crate::views::composer::TRANSITION,
                            cx.background_executor().clone(),
                        );
                    });
                }
                model.settings.composer = settings.clone();
                model.composer.preferences_dirty = true;
                if let Some(view) = &model.composer.view {
                    model.composer.host.place(view.read(cx).placement());
                }
                model.save_composer(cx);
                model.sync_preferences(cx);
                cx.notify();
            }
        }));
        self.composer.key = Some(key);
        self.composer.pane = self.active_pane();
        self.composer.view = Some(view);
        cx.notify();
    }

    pub(crate) fn close_composer(&mut self, cx: &mut Context<Self>) {
        self.dismiss_composer(self.settings.composer.clear_on_close, cx);
    }

    fn dismiss_composer(&mut self, clear: bool, cx: &mut Context<Self>) {
        if let Some(view) = &self.composer.view {
            view.update(cx, Composer::cancel_voice);
        }
        if clear && let Some(key) = &self.composer.key {
            let result = self
                .composer
                .store
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .edit_content(key.clone(), String::new(), Vec::new());
            if let Err(error) = result {
                self.composer_error(error.to_string(), cx);
            }
        }
        self.save_composer(cx);
        self.composer.host.remove(&PanelId::new(PANEL));
        if let Some(view) = self.composer.view.take()
            && view.read(cx).settings.presentation
                == muxy_app_core::settings::ComposerPresentation::Floating
        {
            view.update(cx, |view, cx| {
                view.visibility.hide();
                cx.notify();
            });
            self.composer.closing = Some(view);
            self.composer.exit = Some(cx.spawn(async move |model, cx| {
                cx.background_executor()
                    .timer(crate::views::composer::TRANSITION)
                    .await;
                let _ = model.update(cx, |model, cx| {
                    model.composer.closing = None;
                    cx.notify();
                });
            }));
        }
        self.composer.subscription = None;
        self.composer.key = None;
        self.composer.pane = None;
        self.focus_requested = true;
        cx.notify();
    }

    pub(super) fn sync_composer(&mut self, cx: &mut Context<Self>) {
        if self.composer.view.is_none() {
            return;
        }
        if self.composer.key.as_ref() != Some(&self.composer_key()) {
            self.close_composer(cx);
            self.open_composer(cx);
        } else if self.composer.pane != self.active_pane() {
            if let Some(view) = &self.composer.view {
                view.update(cx, Composer::cancel_voice);
            }
            self.composer.pane = self.active_pane();
        }
    }

    fn composer_changed(&mut self, cx: &mut Context<Self>) {
        let (Some(key), Some(view)) = (&self.composer.key, &self.composer.view) else {
            return;
        };
        let draft = view.read(cx).draft.clone();
        let result = {
            let mut store = self
                .composer
                .store
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            store
                .edit_content(key.clone(), draft.text, draft.file_attachments)
                .map(|_| store.draft(key).cloned().unwrap_or_default())
        };
        match result {
            Ok(draft) => view.update(cx, |view, cx| {
                view.draft = draft;
                cx.notify();
            }),
            Err(error) => self.composer_error(error.to_string(), cx),
        }
        self.save_composer(cx);
    }

    pub(super) fn save_composer(&mut self, cx: &mut Context<Self>) {
        let store = self.composer.store.clone();
        self.composer.save = Some(cx.spawn(async move |model, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            let result = cx
                .background_executor()
                .spawn(async move {
                    store
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .flush()
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = model.update(cx, |model, cx| {
                let result = result.and_then(|_| {
                    if model.composer.preferences_dirty {
                        model
                            .settings
                            .save_composer(&model.path.with_file_name("settings.toml"))
                            .map_err(|error| error.to_string())?;
                        model.composer.preferences_dirty = false;
                    }
                    Ok(())
                });
                if let Err(error) = result {
                    model.composer_error(format!("Could not save composer: {error}"), cx);
                }
            });
        }));
    }

    pub(super) fn flush_composer(&mut self, cx: &mut Context<Self>) -> Task<()> {
        self.composer.save = None;
        if let Some(view) = &self.composer.view {
            view.update(cx, Composer::cancel_voice);
        }
        let store = self.composer.store.clone();
        let settings = self.settings.clone();
        let path = self.path.with_file_name("settings.toml");
        let preferences = self.composer.preferences_dirty;
        cx.background_executor().spawn(async move {
            if let Err(error) = store.lock().unwrap_or_else(PoisonError::into_inner).flush() {
                let _ = writeln!(
                    std::io::stderr(),
                    "Could not flush composer drafts: {error}"
                );
            }
            if preferences && let Err(error) = settings.save_composer(&path) {
                let _ = writeln!(
                    std::io::stderr(),
                    "Could not save composer preferences: {error}"
                );
            }
        })
    }

    fn composer_error(&mut self, error: String, cx: &mut Context<Self>) {
        if let Some(view) = &self.composer.view {
            view.update(cx, |view, cx| {
                view.error = Some(error);
                cx.notify();
            });
        } else {
            self.fail(error, cx);
        }
    }

    fn choose_composer_files(&mut self, cx: &mut Context<Self>) {
        let key = self.composer.key.clone();
        let result = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: Some("Attach".into()),
        });
        cx.spawn(async move |model, cx| {
            let result = result.await;
            let _ = model.update(cx, |model, cx| {
                if model.composer.key != key {
                    return;
                }
                match result {
                    Ok(Ok(Some(paths))) => model.attach_composer_files(paths, cx),
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => model.composer_error(error.to_string(), cx),
                    Err(error) => model.composer_error(error.to_string(), cx),
                }
            });
        })
        .detach();
    }

    fn attach_composer_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if let Some(view) = &self.composer.view {
            view.update(cx, |view, cx| {
                for path in paths {
                    let Some(path) = path.to_str().filter(|_| path.is_absolute()) else {
                        view.error = Some("Attachment paths must be absolute UTF-8 paths".into());
                        continue;
                    };
                    if !view
                        .draft
                        .file_attachments
                        .iter()
                        .any(|existing| existing == path)
                    {
                        view.draft.file_attachments.push(path.into());
                    }
                }
                cx.notify();
            });
            self.composer_changed(cx);
        }
    }

    fn attach_composer_image(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let (Some(key), Some(view)) = (self.composer.key.clone(), self.composer.view.as_ref())
        else {
            return;
        };
        let selection = view.read(cx).input.read(cx).selected_range();
        let revision = self
            .composer
            .store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .draft_revision(&key);
        let store = self.composer.store.clone();
        cx.spawn(async move |model, cx| {
            let target = key.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let source = prepare_image_source(bytes).map_err(|error| error.to_string())?;
                    let mut store = store.lock().unwrap_or_else(PoisonError::into_inner);
                    if store.draft_revision(&target) != revision {
                        return Err("Draft changed while preparing the image. Paste again.".into());
                    }
                    let (_, revision) = store
                        .attach_prepared_image(target.clone(), &source, selection)
                        .map_err(|error| error.to_string())?;
                    Ok((revision, store.draft(&target).cloned().unwrap_or_default()))
                })
                .await;
            let _ = model.update(cx, |model, cx| {
                if model.composer.key.as_ref() != Some(&key) {
                    return;
                }
                match result {
                    Ok((revision, draft)) => {
                        if model
                            .composer
                            .store
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .draft_revision(&key)
                            == revision
                            && let Some(view) = &model.composer.view
                        {
                            view.update(cx, |view, cx| view.set_draft(draft, cx));
                        }
                    }
                    Err(error) => model.composer_error(error, cx),
                }
            });
        })
        .detach();
    }

    pub(crate) fn composer_content(
        &self,
        content: gpui::AnyElement,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(view) = &self.composer.view else {
            return content;
        };
        view.update(cx, |view, _| {
            view.workspace_inset = self.sidebar_width();
            let project = self.state.current_project();
            let parent = project
                .parent_id
                .and_then(|id| self.state.project(id))
                .unwrap_or(project);
            view.context_label = format!("{} / {}", parent.name, project.name);
        });
        if self.settings.composer.presentation
            == muxy_app_core::settings::ComposerPresentation::Floating
        {
            return content;
        }
        let dimension = gpui::px(view.read(cx).sizing(window).layout().dimension() + 1.0);
        let Some(placement) = self.composer.host.placement(&PanelId::new(PANEL)) else {
            return content;
        };
        let right = placement.position == muxy_ui::panel::PanelPosition::Right;
        let pinned = placement.mode == muxy_ui::panel::PanelMode::Pinned;
        let body = div()
            .relative()
            .flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .when(!right, Styled::flex_col)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .child(content),
            );
        if pinned {
            return body.child(view.clone()).into_any_element();
        }
        body.child(
            div()
                .id("floating-composer")
                .absolute()
                .right_0()
                .bottom_0()
                .when(right, |frame| frame.top_0().w(dimension))
                .when(!right, |frame| frame.left_0().h(dimension))
                .on_mouse_down_out(cx.listener(|model, _, _, cx| {
                    if model.overlay.is_none() {
                        model.close_composer(cx);
                    }
                }))
                .child(view.clone()),
        )
        .into_any_element()
    }

    pub(crate) fn floating_composer(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(view) = self
            .composer
            .view
            .as_ref()
            .or(self.composer.closing.as_ref())
        else {
            return div().into_any_element();
        };
        if self.settings.composer.presentation
            != muxy_app_core::settings::ComposerPresentation::Floating
        {
            return div().into_any_element();
        }
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .opacity(view.read(cx).visibility.sample(window))
            .bg(gpui::black().opacity(0.62))
            .occlude()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|model, _, _, cx| model.close_composer(cx)),
            )
            .child(view.clone())
            .into_any_element()
    }

    fn composer_menu(&mut self, cx: &mut Context<Self>) {
        let Some(view) = &self.composer.view else {
            return;
        };
        let choices = view.read(cx).menu_items();
        let anchor = view.read(cx).menu_anchor.get();
        let Ok(receiver) = self.window.update(cx, |_, window, _| {
            muxy_ui::native_menu::show(
                choices.iter().map(|(item, _)| item.clone()).collect(),
                anchor,
                window,
            )
        }) else {
            return;
        };
        let view = view.downgrade();
        cx.spawn(async move |model, cx| {
            let selected = receiver.recv().await.ok().flatten();
            let _ = model.update(cx, |model, cx| {
                let Some(active) = &model.composer.view else {
                    return;
                };
                if active.downgrade() != view {
                    return;
                }
                if let Some((_, command)) = selected.and_then(|index| choices.get(index)) {
                    active.update(cx, |view, cx| view.menu_action(*command, cx));
                }
                let focus = active.focus_handle(cx);
                let _ = model.window.update(cx, |_, window, _| focus.focus(window));
                model.focus_requested = false;
            });
        })
        .detach();
    }

    pub(crate) fn composer_language(&mut self, cx: &mut Context<Self>) {
        let key = self.composer.key.clone();
        let mut receivers = None;
        let modal = cx.new(|cx| {
            let (modal, result, queries) = NativeModal::new(
                muxy_app_core::modal::ModalOptions {
                    placeholder: "Choose dictation language…".into(),
                    empty_label: "No on-device speech languages available".into(),
                    ..Default::default()
                },
                self.theme.clone(),
                self.metrics,
                cx,
            );
            receivers = Some((result, queries));
            modal
        });
        let Some((result, _queries)) = receivers else {
            return;
        };
        let token = modal.read(cx).token();
        let weak = modal.downgrade();
        let focus = modal.focus_handle(cx);
        let window = self.window;
        let _ = window.update(cx, |_, window, _| {
            window.activate_window();
            focus.focus(window);
        });
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Native(modal));
        cx.notify();
        cx.spawn(async move |model, cx| {
            let languages = cx.background_executor().spawn(async { muxy_ui::voice::languages() }).await;
            let _ = weak.update(cx, |modal, cx| {
                modal.feed(token, languages.into_iter().map(|language| muxy_app_core::modal::ModalItem::new(language.id, language.name)).collect(), cx);
                modal.finish(token, cx);
            });
            let choice = result.recv().await.ok().flatten();
            let _ = model.update(cx, |model, cx| {
                if matches!(&model.overlay, Some(Overlay::Native(modal)) if modal.read(cx).token().id == token.id) { model.dismiss_overlay(cx); }
                if model.composer.key != key { return; }
                if let Some(choice) = choice {
                    model.settings.composer.language = choice.id;
                    model.composer.preferences_dirty = true;
                    if let Some(view) = &model.composer.view { view.update(cx, |view, _| view.settings = model.settings.composer.clone()); }
                    model.save_composer(cx);
                    model.sync_preferences(cx);
                }
                if model.overlay.is_none() && let Some(view) = &model.composer.view {
                    let focus = view.focus_handle(cx);
                    let _ = model.window.update(cx, |_, window, _| focus.focus(window));
                    model.focus_requested = false;
                }
            });
        }).detach();
    }
}

#[derive(Clone)]
enum Step {
    Text(String),
    Image(Vec<u8>),
}

fn prepare(plan: &SubmissionPlan, store: &mut ComposerStore) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for segment in &plan.segments {
        match segment {
            SubmissionSegment::Text(text) => steps.push(Step::Text(text.clone())),
            SubmissionSegment::LocalPath(path) => {
                if !std::path::Path::new(path)
                    .try_exists()
                    .map_err(|error| error.to_string())?
                {
                    return Err(format!("Attached file is missing: {path}"));
                }
                let bytes =
                    crate::views::terminal::clipboard::paths(&[path.into()], Modes::default())
                        .ok_or("Invalid attachment path")?;
                steps.push(Step::Text(
                    String::from_utf8(bytes).map_err(|error| error.to_string())?,
                ));
            }
            SubmissionSegment::CopiedImage { filename, .. } => {
                let images = store
                    .image_storage()
                    .ok_or("Composer image storage is unavailable")?;
                let png = images
                    .normalize_png(filename)
                    .map_err(|error| error.to_string())?;
                if plan.image_strategy == ImageSubmissionStrategy::Clipboard {
                    steps.push(Step::Image(png));
                } else {
                    let bytes = crate::views::terminal::clipboard::paths(
                        &[images
                            .path_for(filename)
                            .map_err(|error| error.to_string())?],
                        Modes::default(),
                    )
                    .ok_or("Invalid image path")?;
                    steps.push(Step::Text(
                        String::from_utf8(bytes).map_err(|error| error.to_string())?,
                    ));
                }
            }
        }
    }
    if steps
        .iter()
        .map(|step| match step {
            Step::Text(text) => text.len() + 16,
            Step::Image(_) => 1,
        })
        .sum::<usize>()
        > muxy_protocol::MAX_INPUT - 2
    {
        return Err("Composer text exceeds the 1 MiB submission limit".into());
    }
    if plan.image_strategy == ImageSubmissionStrategy::InlinePath {
        store.retain_submitted_images(plan.segments.iter().filter_map(|segment| match segment {
            SubmissionSegment::CopiedImage { filename, .. } => Some(filename.clone()),
            _ => None,
        }));
    }
    Ok(steps)
}

impl AppModel {
    fn composer_target_current(
        &self,
        key: &DraftId,
        pane: PaneId,
        channel: ChannelId,
        generation: u64,
        cx: &gpui::App,
    ) -> bool {
        self.generation == generation
            && &self.composer_key() == key
            && self
                .grids
                .get(&pane)
                .is_some_and(|pane| pane.view.read(cx).channel() == Some(channel))
    }

    fn composer_targets(&self, cx: &gpui::App) -> Result<Vec<(PaneId, ChannelId, Modes)>, String> {
        let requested = if self.settings.composer.broadcast {
            self.visible_panes()
        } else {
            self.active_pane().into_iter().collect()
        };
        let mut sessions = std::collections::HashSet::new();
        let mut targets = Vec::new();
        for id in requested {
            let Some((session, channel, modes)) = self.pane_session(id).and_then(|session| {
                let pane = self.grids.get(&id)?.view.read(cx);
                Some((session, pane.channel()?, pane.grid.as_ref()?.modes))
            }) else {
                return Err("Every target must have a live terminal before sending.".into());
            };
            if sessions.insert(session) {
                targets.push((id, channel, modes));
            }
        }
        Ok(targets)
    }

    fn composer_plan(
        &self,
        key: &DraftId,
        view: &Entity<Composer>,
        targets: &[(PaneId, ChannelId, Modes)],
        enter: bool,
        cx: &gpui::App,
    ) -> Option<(u64, SubmissionPlan)> {
        let composer = view.read(cx);
        if composer.recording() {
            return None;
        }
        let revision = self
            .composer
            .store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .draft_revision(key);
        let plan = plan_submission(SubmissionSnapshot {
            text: composer.draft.text.clone(),
            revision,
            selected_text: Some(composer.input.read(cx).selected_text().to_owned()),
            file_paths: composer.draft.file_attachments.clone(),
            image_attachments: composer.draft.image_attachments.clone(),
            append_return: enter,
            image_strategy: composer.settings.image_strategy,
            target_pane_ids: targets.iter().map(|(id, _, _)| id.to_string()).collect(),
        });
        (!plan.is_empty()).then_some((revision, plan))
    }

    fn submit_composer(&mut self, enter: bool, cx: &mut Context<Self>) {
        if self.composer.submission.is_some() {
            return;
        }
        let (Some(key), Some(view)) = (self.composer.key.clone(), self.composer.view.clone())
        else {
            return;
        };
        let targets = match self.composer_targets(cx) {
            Ok(targets) => targets,
            Err(error) => {
                self.composer_error(error, cx);
                return;
            }
        };
        if targets.is_empty() {
            self.composer_error("Open a live terminal before sending.".into(), cx);
            return;
        }
        let Some((revision, plan)) = self.composer_plan(&key, &view, &targets, enter, cx) else {
            return;
        };
        let clear = self.settings.composer.clear_after_sending;
        view.update(cx, |view, cx| {
            view.sending = true;
            view.error = None;
            cx.notify();
        });
        let store = self.composer.store.clone();
        let worker = self.work.clone();
        let generation = self.generation;
        self.composer.submission = Some(cx.spawn(async move |model, cx| {
            let prepared = cx.background_executor().spawn(async move { prepare(&plan, &mut store.lock().unwrap_or_else(PoisonError::into_inner)) }).await;
            let mut completed = 0;
            let result = async {
                let steps = prepared?;
                let _ = model.update(cx, |model, cx| {
                    if model.composer.key.as_ref() == Some(&key)
                        && model.settings.composer.presentation == muxy_app_core::settings::ComposerPresentation::Floating {
                        model.dismiss_composer(false, cx);
                    }
                });
                let mut clipboard = if steps.iter().any(|step| matches!(step, Step::Image(_))) { Some(muxy_ui::pasteboard::Lease::capture()?) } else { None };
                for (pane, channel, modes) in &targets {
                    let mut payload = vec![0x15];
                    for step in &steps {
                        let current = model.update(cx, |model, cx| model.composer_target_current(&key, *pane, *channel, generation, cx)).unwrap_or(false);
                        if !current { return Err("Composer target changed; sending stopped.".into()); }
                        match step {
                            Step::Text(text) => payload.extend(crate::views::terminal::clipboard::paste(text, *modes)),
                            Step::Image(png) => {
                                if !payload.is_empty() { deliver(&worker, generation, *channel, std::mem::take(&mut payload)).await?; }
                                if let Some(clipboard) = &mut clipboard { clipboard.write_png(png.clone())?; }
                                deliver(&worker, generation, *channel, vec![0x16]).await?;
                                cx.background_executor().timer(std::time::Duration::from_millis(350)).await;
                            }
                        }
                    }
                    let current = model.update(cx, |model, cx| model.composer_target_current(&key, *pane, *channel, generation, cx)).unwrap_or(false);
                    if !current { return Err("Composer target changed; sending stopped.".into()); }
                    if enter { payload.push(b'\r'); }
                    if !payload.is_empty() { deliver(&worker, generation, *channel, payload).await?; }
                    completed += 1;
                }
                Ok::<_, String>(())
            }.await;
            let _ = model.update(cx, |model, cx| {
                model.composer.submission = None;
                if let Some(view) = &model.composer.view { view.update(cx, |view, cx| { view.sending = false; cx.notify(); }); }
                match result {
                    Ok(()) if clear => {
                        let cleared = model.composer.store.lock().unwrap_or_else(PoisonError::into_inner).clear_if_revision(&key, revision);
                        match cleared {
                            Ok(true) => {
                                if model.composer.key.as_ref() == Some(&key) && let Some(view) = &model.composer.view { view.update(cx, |view, cx| view.set_draft(ComposerDraft::default(), cx)); }
                                model.save_composer(cx);
                            }
                            Ok(false) => {},
                            Err(error) => model.composer_error(error.to_string(), cx),
                        }
                    }
                    Ok(()) => {},
                    Err(error) => model.composer_error(format!("{completed}/{} terminals sent. {error} Draft preserved; check the terminal before retrying.", targets.len()), cx),
                }
            });
        }));
    }
}

pub(super) async fn deliver(
    worker: &crate::boot::Worker,
    generation: u64,
    channel: ChannelId,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let (completion, result) = async_channel::bounded(1);
    worker
        .send((
            generation,
            Work::WriteInput {
                channel,
                bytes,
                completion,
            },
        ))
        .map_err(|error| error.to_string())?;
    result
        .recv()
        .await
        .map_err(|_| "Terminal connection closed before delivery was confirmed".to_owned())?
}
