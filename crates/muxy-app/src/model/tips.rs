use std::hash::{BuildHasher, RandomState};
use std::time::Duration;

use gpui::{Context, Task, Window};
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::motion::VisibilityMotion;
use muxy_ui::tr;

use super::AppModel;
use super::banners::TOAST_TRANSITION;

pub(crate) const FIRST_TIP_DELAY: Duration = Duration::from_secs(10 * 60);
pub(crate) const TIP_INTERVAL: Duration = Duration::from_secs(2 * 60 * 60);
/// How soon to offer a tip again after a dialog, menu or other window held it back.
pub(crate) const TIP_RETRY: Duration = Duration::from_secs(60);
pub(crate) const TIP_DURATION: Duration = Duration::from_secs(30);

/// Built-in tips, in the order the toast shows them.
/// Each one must describe current behavior and call customizable shortcuts defaults.
pub(crate) const TIPS: &[&str] = &[
    tr_key!(
        "Quitting Muxy never ends your terminals. To keep them when closing tabs or panes too, set Settings → General → Closing terminals → When closing tabs or panes to Detach."
    ),
    tr_key!(
        "Right-click a terminal and choose Detach Terminal to put it away without ending it. By default, ⌘⌥T opens Existing Terminals to bring it back."
    ),
    tr_key!(
        "By default, ⌘D splits right, ⌘⇧D splits down, ⌘⌥ plus an arrow moves between panes, and ⌘⇧↩ maximizes or restores the focused pane."
    ),
    tr_key!(
        "By default, ⌘⇧P opens the command palette. Choose Switch Project… to jump to any project or worktree by name."
    ),
    tr_key!(
        "Use the filter at the top of the expanded sidebar to show one workspace or Focus Current Project. Right-click a project → Workspaces to add it to one."
    ),
    tr_key!(
        "Choose Tab Focused from the layout menu at the top of the expanded sidebar to list every project's tabs there. By default, ⌘1–⌘9 then select tabs across projects."
    ),
    tr_key!(
        "In a Git project, right-click the project and choose Worktrees → Show Worktrees, then New Worktree…, to work on a branch in its own folder with its own tabs."
    ),
    tr_key!(
        "By default, ⌘I opens Composer to draft prompts with files or images, and ⌘↩ sends it. Send to All Split Panes in its menu targets every split."
    ),
    tr_key!(
        "Record a shortcut in Settings → Quick Terminal → Shortcut to open a floating Home shell over any app."
    ),
    tr_key!(
        "To pair a phone or another computer, open the Devices tab, turn on Allow other devices, and click Show Pairing Code, or run muxy mobile enable and then muxy mobile pair."
    ),
    tr_key!(
        "Working over SSH? Run muxy on that machine for tabs and splits in the terminal, and press Ctrl-B then ? to see its keys."
    ),
    tr_key!(
        "Muxy recognizes AI agents like Claude Code and Codex, marks panes, tabs, and projects when they're working or need you, and notifies you when they're in the background."
    ),
    tr_key!(
        "In a Git project, click Commit in the status bar. Choose the AI CLI, whether to include unstaged changes, and whether to push. The CLI writes the message and Muxy commits."
    ),
    tr_key!(
        "Browse extensions in Settings → Extensions. Each one lists the permissions it requests and runs only after you enable it."
    ),
    tr_key!(
        "Click Server in the status bar to connect, restart, or stop the background server. Restarting or stopping ends every terminal on this Mac."
    ),
    tr_key!(
        "By default, ⌘⌃← and ⌘⌃→ go back and forward through tabs you've visited, across projects."
    ),
];

pub(crate) struct TipToast {
    pub(crate) motion: VisibilityMotion,
    leaving: bool,
}

/// The tip toast. Tips follow [`TIPS`] from a random place chosen once per launch.
pub(crate) struct Tips {
    index: usize,
    pub(crate) toast: Option<TipToast>,
    schedule: Option<Task<()>>,
    /// Hides the toast. Replacing it cancels a pending hide.
    hide: Option<Task<()>>,
}

impl Default for Tips {
    fn default() -> Self {
        Self::starting_at(RandomState::new().hash_one(()))
    }
}

impl Tips {
    fn starting_at(seed: u64) -> Self {
        Self {
            index: usize::try_from(seed % TIPS.len() as u64).unwrap_or_default(),
            toast: None,
            schedule: None,
            hide: None,
        }
    }

    pub(crate) fn current(&self) -> gpui::SharedString {
        translate(TIPS[self.index])
    }
}

impl AppModel {
    pub(super) fn start_tips(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tips.schedule = Some(cx.spawn_in(window, async move |model, cx| {
            let mut wait = FIRST_TIP_DELAY;
            loop {
                cx.background_executor().timer(wait).await;
                let Ok(next) = model.update_in(cx, |model, window, cx| model.offer_tip(window, cx))
                else {
                    break;
                };
                wait = next;
            }
        }));
    }

    /// Shows the next tip unless something needs the user's attention, and
    /// returns how long to wait before offering another.
    fn offer_tip(&mut self, window: &Window, cx: &mut Context<Self>) -> Duration {
        if !self.appearance.tips_visible {
            return TIP_INTERVAL;
        }
        if !window.is_window_active()
            || self.dialogs.is_open()
            || self.overlay.is_some()
            || self.problem_toast.is_some()
            || self.tips.toast.is_some()
        {
            return TIP_RETRY;
        }
        self.show_tip(cx);
        TIP_INTERVAL
    }

    pub(crate) fn show_tip(&mut self, cx: &mut Context<Self>) {
        self.tips.index = (self.tips.index + 1) % TIPS.len();
        self.tips.toast = Some(TipToast {
            motion: VisibilityMotion::showing(TOAST_TRANSITION, cx.background_executor().clone()),
            leaving: false,
        });
        self.start_tip_timer(cx);
        cx.notify();
    }

    fn start_tip_timer(&mut self, cx: &mut Context<Self>) {
        self.tips.hide = Some(cx.spawn(async move |model, cx| {
            cx.background_executor().timer(TIP_DURATION).await;
            let _ = model.update(cx, Self::hide_tip);
        }));
    }

    /// The pointer on the toast holds it; leaving gives it its full time again.
    pub(crate) fn hover_tip(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.tips.toast.as_ref().is_none_or(|toast| toast.leaving) {
            return;
        }
        if hovered {
            self.tips.hide = None;
        } else {
            self.start_tip_timer(cx);
        }
    }

    pub(crate) fn hide_tip(&mut self, cx: &mut Context<Self>) {
        let Some(toast) = &mut self.tips.toast else {
            return;
        };
        toast.motion.hide();
        toast.leaving = true;
        self.tips.hide = Some(cx.spawn(async move |model, cx| {
            cx.background_executor().timer(TOAST_TRANSITION).await;
            let _ = model.update(cx, |model, cx| {
                model.tips.toast = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn turn_off_tips(&mut self, cx: &mut Context<Self>) {
        self.appearance.tips_visible = false;
        self.save_appearance(cx);
        self.hide_tip(cx);
        self.show_toast(
            tr!("Tips turned off").to_string(),
            Some(translate(TIPS_OFF_MESSAGE).to_string()),
            cx,
        );
    }
}

const TIPS_OFF_MESSAGE: &str = tr_key!(
    "You can show tips again in Settings → Appearance → Interface by turning on Show tips."
);
