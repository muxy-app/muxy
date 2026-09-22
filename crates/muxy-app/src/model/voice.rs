use super::AppModel;
use crate::views::voice::{VoiceEvent, VoicePanel};
use gpui::{
    AppContext, Context, Entity, Focusable, IntoElement, ParentElement, Styled, Subscription,
    Window, div,
};
use muxy_app_core::{PaneId, composer::DraftId};
use muxy_protocol::{ChannelId, Modes};

struct Target {
    project: DraftId,
    pane: PaneId,
    channel: ChannelId,
    generation: u64,
    modes: Modes,
}

#[derive(Default)]
pub(crate) struct VoiceRuntime {
    pub(crate) view: Option<Entity<VoicePanel>>,
    pub(crate) closing: Option<Entity<VoicePanel>>,
    exit: Option<gpui::Task<()>>,
    target: Option<Target>,
    subscription: Option<Subscription>,
}

impl AppModel {
    pub(crate) fn toggle_voice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if self.voice.view.is_some() {
            self.close_voice(cx);
            return;
        }
        if let Some(view) = &self.composer.view {
            view.update(cx, crate::views::composer::Composer::toggle_voice);
            view.focus_handle(cx).focus(window);
            return;
        }
        let target = self.active_pane().and_then(|pane| {
            let view = self.grids.get(&pane)?.view.read(cx);
            Some(Target {
                project: self.composer_key(),
                pane,
                channel: view.channel()?,
                generation: self.generation,
                modes: view.grid.as_ref()?.modes,
            })
        });
        let Some(target) = target else {
            self.set_banner_error(Some(
                "Open a live terminal before starting dictation.".into(),
            ));
            cx.notify();
            return;
        };
        self.dismiss_overlay(cx);
        self.voice.closing = None;
        self.voice.exit = None;
        let view = cx.new(|cx| {
            VoicePanel::new(
                self.settings.composer.language.clone(),
                self.settings.composer.voice_auto_send,
                self.theme.clone(),
                self.metrics,
                cx,
            )
        });
        self.voice.subscription = Some(cx.subscribe(&view, |model, _, event, cx| match event {
            VoiceEvent::Cancel => model.close_voice(cx),
            VoiceEvent::Finish(text) => model.finish_voice(text, cx),
        }));
        view.focus_handle(cx).focus(window);
        self.voice.view = Some(view);
        self.voice.target = Some(target);
        self.focus_requested = false;
        cx.notify();
    }

    pub(super) fn close_voice(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self.voice.view.take() {
            view.update(cx, VoicePanel::dismiss);
            self.voice.closing = Some(view);
            self.voice.exit = Some(cx.spawn(async move |model, cx| {
                cx.background_executor()
                    .timer(crate::views::voice::TRANSITION)
                    .await;
                let _ = model.update(cx, |model, cx| {
                    model.voice.closing = None;
                    cx.notify();
                });
            }));
        }
        self.voice.target = None;
        self.voice.subscription = None;
        self.focus_requested = true;
        cx.notify();
    }

    pub(super) fn sync_voice(&mut self, cx: &mut Context<Self>) {
        let Some(target) = &self.voice.target else {
            return;
        };
        if !self.voice_target_current(target, cx) || self.active_pane() != Some(target.pane) {
            self.close_voice(cx);
        }
    }

    fn voice_target_current(&self, target: &Target, cx: &gpui::App) -> bool {
        self.generation == target.generation
            && self.composer_key() == target.project
            && self
                .grids
                .get(&target.pane)
                .is_some_and(|pane| pane.view.read(cx).channel() == Some(target.channel))
    }

    fn finish_voice(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(target) = self.voice.target.take() else {
            return;
        };
        if !self.voice_target_current(&target, cx) {
            self.close_voice(cx);
            return;
        }
        let mut bytes = crate::views::terminal::clipboard::paste(text, target.modes);
        if self.settings.composer.voice_auto_send {
            bytes.push(b'\r');
        }
        self.close_voice(cx);
        let worker = self.work.clone();
        cx.spawn(async move |model, cx| {
            if let Err(error) =
                super::composer::deliver(&worker, target.generation, target.channel, bytes).await
            {
                let _ = model.update(cx, |model, cx| {
                    model.set_banner_error(Some(format!("Dictation delivery failed: {error}")));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn voice_panel(&self) -> gpui::AnyElement {
        let Some(view) = self.voice.view.as_ref().or(self.voice.closing.as_ref()) else {
            return div().into_any_element();
        };
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .relative()
                    .child(view.clone())
                    .child(self.webview_occlusion()),
            )
            .into_any_element()
    }
}
