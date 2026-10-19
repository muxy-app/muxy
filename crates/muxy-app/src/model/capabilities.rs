use gpui::Context;
use muxy_core::capability::{Capabilities, Capability, Feature};

use super::AppModel;

fn backend(feature: Feature) -> Capability {
    match feature {
        Feature::ExtensionScripts => muxy_ui::javascript::CAPABILITY,
        Feature::ExtensionWebviews => muxy_ui::webview::CAPABILITY,
        Feature::Voice => muxy_ui::voice::CAPABILITY,
        Feature::QuickTerminal => muxy_ui::quick_terminal::platform::CAPABILITY,
        Feature::Notifications => muxy_ui::notifications::CAPABILITY,
        Feature::ClipboardImages => muxy_ui::pasteboard::IMAGE_SUBMISSION,
        Feature::CommandLineTool => crate::cli_install::CAPABILITY,
        Feature::Updates => crate::updater::CAPABILITY,
        Feature::AppHiding => Capability::of(cfg!(target_os = "macos")),
        Feature::BrowserPanes | Feature::SshRemotes => Capability::of(!cfg!(windows)),
    }
}

/// What this build has backends for. Services refine the table as they start.
pub(crate) fn backends() -> Capabilities {
    let mut capabilities = Capabilities::default();
    for feature in Feature::ALL {
        capabilities.set(feature, backend(feature));
    }
    capabilities
}

impl AppModel {
    pub(crate) fn supports(&self, feature: Feature) -> bool {
        self.capabilities.supports(feature)
    }

    /// Whether `feature` can run. Otherwise reports why, before any work starts.
    pub(crate) fn require(&mut self, feature: Feature, cx: &mut Context<Self>) -> bool {
        let Some(reason) = self.capabilities.reason(feature) else {
            return true;
        };
        self.fail(reason.into_owned(), cx);
        false
    }

    /// Leaves an app-wide command to its own handler where `feature` can run.
    pub(crate) fn forward(&mut self, feature: Feature, cx: &mut Context<Self>) {
        if self.require(feature, cx) {
            cx.propagate();
        }
    }
}
