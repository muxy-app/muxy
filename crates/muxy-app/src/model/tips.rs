use std::hash::{BuildHasher, RandomState};
use std::rc::Rc;

use gpui::{Context, Window};
use muxy_ui::popover::PopoverAnchor;

use super::AppModel;
use crate::views::overlays::Overlay;

/// Built-in sidebar tips, in the order the previous and next buttons follow.
/// Each one must describe current behavior and call customizable shortcuts defaults.
pub(crate) const TIPS: &[&str] = &[
    "Quitting Muxy never ends your terminals. To keep them when closing tabs or panes too, set Settings → General → Closing terminals → When closing tabs or panes to Detach.",
    "Right-click a terminal and choose Detach Terminal to put it away without ending it. By default, ⌘⌥T opens Existing Terminals to bring it back.",
    "By default, ⌘D splits right, ⌘⇧D splits down, ⌘⌥ plus an arrow moves between panes, and ⌘⇧↩ maximizes or restores the focused pane.",
    "By default, ⌘⇧P opens the command palette. Choose Switch Project… to jump to any project or worktree by name.",
    "Use the filter at the top of the expanded sidebar to show one workspace or Focus Current Project. Right-click a project → Workspaces to add it to one.",
    "Choose Tab Focused from the layout menu at the top of the expanded sidebar to list every project's tabs there. By default, ⌘1–⌘9 then select tabs across projects.",
    "In a Git project, right-click the project and choose Worktrees → Show Worktrees, then New Worktree…, to work on a branch in its own folder with its own tabs.",
    "By default, ⌘I opens Composer to draft prompts with files, images, or dictation, and ⌘↩ sends it. Send to All Split Panes in its menu targets every split.",
    "Record a shortcut in Settings → Quick Terminal → Shortcut to open a floating Home shell over any app.",
    "To pair a phone, turn on Settings → Mobile → Allow mobile devices and click Show Pairing Code, or run muxy mobile enable and then muxy mobile pair.",
    "Working over SSH? Run muxy on that machine for tabs and splits in the terminal, and press Ctrl-B then ? to see its keys.",
    "Muxy recognizes AI agents like Claude Code and Codex, marks panes, tabs, and projects when they're working or need you, and notifies you when they're in the background.",
    "In a Git project, click Commit in the status bar. After you confirm, your AI CLI writes the message and Muxy stages all changes, commits, and pushes. Choose the CLI in Settings → AI.",
    "Browse extensions in Settings → Extensions. Each one lists the permissions it requests and runs only after you enable it.",
    "Click Server in the status bar to connect, restart, or stop the background server. Restarting or stopping ends every terminal on this Mac.",
    "By default, ⌘⌃← and ⌘⌃→ go back and forward through tabs you've visited, across projects.",
];

/// The tip on show. It starts at a random tip once per launch and never rotates by itself.
#[derive(Debug)]
pub(crate) struct Tips {
    index: usize,
    /// The collapsed sidebar's tip button, which the tip popover opens from.
    pub(crate) anchor: PopoverAnchor,
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
            anchor: Rc::default(),
        }
    }

    pub(crate) fn current(&self) -> &'static str {
        TIPS[self.index]
    }

    /// The current tip's one-based position in [`TIPS`].
    pub(crate) fn position(&self) -> usize {
        self.index + 1
    }

    pub(crate) fn step(&mut self, forward: bool) {
        self.index = if forward {
            (self.index + 1) % TIPS.len()
        } else {
            (self.index + TIPS.len() - 1) % TIPS.len()
        };
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TipPlacement {
    /// A card at the bottom of the expanded sidebar.
    Card,
    /// A button at the bottom of the icon-only sidebar that opens the tip in a popover.
    Button,
}

impl AppModel {
    /// Where the built-in sidebar shows tips, or `None` when it shows none.
    pub(crate) fn tip_placement(&self) -> Option<TipPlacement> {
        if !self.appearance.tips_visible || self.extension_sidebar_active() {
            None
        } else if self.appearance.sidebar_expanded {
            Some(TipPlacement::Card)
        } else if self.sidebar_width() > 0.0 {
            Some(TipPlacement::Button)
        } else {
            None
        }
    }

    pub(crate) fn step_tip(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.tips.step(forward);
        cx.notify();
    }

    pub(crate) fn toggle_tip_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if matches!(self.overlay, Some(Overlay::Tip)) {
            self.dismiss_overlay(cx);
            return;
        }
        self.dismiss_overlay(cx);
        self.overlay = Some(Overlay::Tip);
        self.overlay_focus.focus(window);
        cx.notify();
    }

    pub(crate) fn confirm_hide_tips(&mut self, cx: &mut Context<Self>) {
        self.confirm(
            HIDE_TITLE,
            HIDE_MESSAGE.to_owned(),
            HIDE_ACTION,
            cx,
            |model, cx| {
                model.appearance.tips_visible = false;
                model.save_appearance(cx);
            },
        );
    }
}

const HIDE_TITLE: &str = "Hide Tips?";
const HIDE_MESSAGE: &str =
    "You can show tips again in Settings → Appearance → Interface by turning on Show tips.";
pub(crate) const HIDE_ACTION: &str = "Hide Tips";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tip_is_trimmed_non_empty_text() {
        assert!(!TIPS.is_empty());
        for tip in TIPS {
            assert!(!tip.is_empty());
            assert_eq!(tip.trim(), *tip);
        }
    }

    #[test]
    fn stepping_wraps_in_both_directions() {
        let mut tips = Tips::starting_at(0);
        assert_eq!(tips.position(), 1);
        tips.step(false);
        assert_eq!(tips.position(), TIPS.len());
        assert_eq!(tips.current(), TIPS[TIPS.len() - 1]);
        tips.step(true);
        assert_eq!(tips.position(), 1);
        tips.step(true);
        assert_eq!(tips.current(), TIPS[1]);
    }

    #[test]
    fn any_seed_starts_on_a_tip() {
        for seed in [0, 1, TIPS.len() as u64, u64::MAX] {
            let tips = Tips::starting_at(seed);
            assert!((1..=TIPS.len()).contains(&tips.position()));
        }
        assert_eq!(Tips::starting_at(TIPS.len() as u64 + 2).position(), 3);
    }
}
