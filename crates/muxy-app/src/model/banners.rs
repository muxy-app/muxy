use super::AppModel;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BannerKind {
    Error = 0,
    Configuration = 1,
}

impl AppModel {
    pub(crate) fn visible_banner(&self) -> Option<(BannerKind, &str)> {
        [
            (BannerKind::Error, self.error.as_deref()),
            (
                BannerKind::Configuration,
                self.configuration_error.as_deref(),
            ),
        ]
        .into_iter()
        .find_map(|(kind, message)| {
            message
                .filter(|message| {
                    !message.trim().is_empty()
                        && self.dismissed_banners[kind as usize].as_deref() != Some(*message)
                })
                .map(|message| (kind, message))
        })
    }

    pub(crate) fn dismiss_banner(&mut self, kind: BannerKind) {
        self.dismissed_banners[kind as usize].clone_from(match kind {
            BannerKind::Error => &self.error,
            BannerKind::Configuration => &self.configuration_error,
        });
    }

    pub(super) fn set_banner_error(&mut self, error: Option<String>) {
        if self.error != error {
            self.dismissed_banners[BannerKind::Error as usize] = None;
        }
        self.error = error;
    }

    pub(super) fn set_configuration_error(&mut self, error: Option<String>) {
        if self.configuration_error != error {
            self.dismissed_banners[BannerKind::Configuration as usize] = None;
        }
        self.configuration_error = error;
    }
}
