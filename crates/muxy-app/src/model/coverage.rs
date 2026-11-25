use muxy_app_core::settings::ComposerPresentation;

use super::AppModel;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Coverage {
    overlay: Option<OverlayLayer>,
    dialog: bool,
    composer: Option<ComposerLayer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayLayer {
    Marked,
    Unmarked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComposerLayer {
    Floating,
    PanelClosing,
    LoosePanel,
}

impl Coverage {
    pub(crate) fn modal(self) -> bool {
        self.overlay.is_some() || self.dialog
    }

    pub(crate) fn hides_terminal_scroll(self) -> bool {
        self.modal() || self.composer.is_some()
    }

    pub(crate) fn hides_webviews(self) -> bool {
        self.modal() || self.composer == Some(ComposerLayer::Floating)
    }

    pub(crate) fn hides_overlay_webviews(self) -> bool {
        self.dialog
    }

    pub(crate) fn hides_spinners(self) -> bool {
        self.overlay == Some(OverlayLayer::Unmarked)
            || self.composer == Some(ComposerLayer::Floating)
    }

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
        let overlay = self
            .overlay
            .as_ref()
            .map(|overlay| {
                if overlay.mark(self).is_some() {
                    OverlayLayer::Marked
                } else {
                    OverlayLayer::Unmarked
                }
            })
            .or_else(|| {
                self.settings_picker_open()
                    .then_some(OverlayLayer::Unmarked)
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
