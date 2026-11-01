//! What app content is drawn over the workspace. Native views (terminal
//! scroll, webviews, browsers, spinners) step aside under it, and drags stop.
//! A new layer is added here, once.

use muxy_app_core::settings::ComposerPresentation;

use super::AppModel;

/// The layers drawn over the workspace right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Coverage {
    overlay: Option<OverlayLayer>,
    /// A dialog above the window.
    dialog: bool,
    composer: Option<ComposerLayer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayLayer {
    /// Marks the rectangle it covers, so native views are cut out under it.
    Marked,
    Unmarked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComposerLayer {
    /// Open or closing, it dims the whole workspace.
    Floating,
    /// A panel composer animating closed.
    PanelClosing,
    /// An open panel composer that isn't pinned, drawn over the panes.
    LoosePanel,
}

impl Coverage {
    /// An overlay or a dialog owns the window: resizes and window drags
    /// stop.
    pub(crate) fn modal(self) -> bool {
        self.overlay.is_some() || self.dialog
    }

    /// Terminal scroll can't be cut out under a rectangle, so it hides under
    /// every layer except a pinned panel composer, which sits beside the panes.
    pub(crate) fn hides_terminal_scroll(self) -> bool {
        self.modal() || self.composer.is_some()
    }

    /// Webviews and browsers hide under layers that cover the whole
    /// workspace. Under a panel composer they are cut out under its rectangle
    /// instead.
    pub(crate) fn hides_webviews(self) -> bool {
        self.modal() || self.composer == Some(ComposerLayer::Floating)
    }

    /// A webview modal or extension popover is the overlay itself, so only
    /// a dialog is above it.
    pub(crate) fn hides_overlay_webviews(self) -> bool {
        self.dialog
    }

    /// Spinners are cut out under a marked overlay. A dialog sits above the
    /// window, so it hides none.
    pub(crate) fn hides_spinners(self) -> bool {
        self.overlay == Some(OverlayLayer::Unmarked)
            || self.composer == Some(ComposerLayer::Floating)
    }

    /// Pane drags stop under every layer except an open panel composer.
    pub(crate) fn blocks_pane_drag(self) -> bool {
        self.modal()
            || matches!(
                self.composer,
                Some(ComposerLayer::Floating | ComposerLayer::PanelClosing)
            )
    }
}

impl AppModel {
    pub(crate) fn coverage(&self) -> Coverage {
        let overlay = self.overlay.as_ref().map(|overlay| {
            if overlay.mark(self).is_some() {
                OverlayLayer::Marked
            } else {
                OverlayLayer::Unmarked
            }
        });
        let settings = &self.settings.composer;
        let composer = if settings.presentation == ComposerPresentation::Floating {
            (self.composer.view.is_some() || self.composer.closing.is_some())
                .then_some(ComposerLayer::Floating)
        } else if self.composer.closing.is_some() {
            Some(ComposerLayer::PanelClosing)
        } else {
            (self.composer.view.is_some() && !settings.pinned).then_some(ComposerLayer::LoosePanel)
        };
        Coverage {
            overlay,
            dialog: self.dialogs.is_open(),
            composer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ComposerLayer, Coverage, OverlayLayer};

    /// Modal, then whether it hides terminal scroll, webviews, overlay
    /// webviews and spinners, then whether it blocks pane drag.
    fn answers(coverage: Coverage) -> [bool; 6] {
        [
            coverage.modal(),
            coverage.hides_terminal_scroll(),
            coverage.hides_webviews(),
            coverage.hides_overlay_webviews(),
            coverage.hides_spinners(),
            coverage.blocks_pane_drag(),
        ]
    }

    fn composer(layer: ComposerLayer) -> Coverage {
        Coverage {
            composer: Some(layer),
            ..Coverage::default()
        }
    }

    fn overlay(layer: OverlayLayer) -> Coverage {
        Coverage {
            overlay: Some(layer),
            ..Coverage::default()
        }
    }

    #[test]
    fn each_layer_hides_and_blocks_what_it_covers() {
        let cases = [
            ("nothing", Coverage::default(), [false; 6]),
            (
                "marked overlay",
                overlay(OverlayLayer::Marked),
                [true, true, true, false, false, true],
            ),
            (
                "unmarked overlay",
                overlay(OverlayLayer::Unmarked),
                [true, true, true, false, true, true],
            ),
            (
                "dialog",
                Coverage {
                    dialog: true,
                    ..Coverage::default()
                },
                [true, true, true, true, false, true],
            ),
            (
                "floating composer",
                composer(ComposerLayer::Floating),
                [false, true, true, false, true, true],
            ),
            (
                "panel composer closing",
                composer(ComposerLayer::PanelClosing),
                [false, true, false, false, false, true],
            ),
            (
                "unpinned panel composer",
                composer(ComposerLayer::LoosePanel),
                [false, true, false, false, false, false],
            ),
        ];
        for (layer, coverage, expected) in cases {
            assert_eq!(answers(coverage), expected, "{layer}");
        }
    }
}
