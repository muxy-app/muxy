//! Which desktop features this build and its runtime services can provide.
//! Commands, settings, and error reports read one table instead of testing
//! the operating system.

use std::borrow::Cow;

use crate::tr;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Feature {
    ExtensionScripts,
    ExtensionWebviews,
    Voice,
    QuickTerminal,
    Notifications,
    ClipboardImages,
    CommandLineTool,
    Updates,
    AppHiding,
    BrowserPanes,
    SshRemotes,
}

impl Feature {
    pub const ALL: [Self; 11] = [
        Self::ExtensionScripts,
        Self::ExtensionWebviews,
        Self::Voice,
        Self::QuickTerminal,
        Self::Notifications,
        Self::ClipboardImages,
        Self::CommandLineTool,
        Self::Updates,
        Self::AppHiding,
        Self::BrowserPanes,
        Self::SshRemotes,
    ];

    /// Why a build without a backend for the feature cannot offer it.
    pub fn unsupported_reason(self) -> Cow<'static, str> {
        match self {
            Self::ExtensionScripts => {
                tr!("Extension scripts are not available on this platform yet.")
            }
            Self::ExtensionWebviews => {
                tr!("Extension webviews are not available on this platform yet.")
            }
            Self::Voice => tr!("Voice input is not available on this platform yet."),
            Self::QuickTerminal => {
                tr!("The Quick Terminal is not available on this platform yet.")
            }
            Self::Notifications => {
                tr!("Desktop notifications are not available on this platform yet.")
            }
            Self::ClipboardImages => {
                tr!("Clipboard image submission is not available on this platform yet.")
            }
            Self::CommandLineTool => {
                tr!("Installing the command line tool is not available on this platform yet.")
            }
            Self::Updates => tr!("Automatic updates are not available on this platform yet."),
            Self::AppHiding => tr!("Hiding apps is not available on this platform."),
            Self::BrowserPanes => tr!("Browser tabs are not available on this platform yet."),
            Self::SshRemotes => tr!("Remote servers are not available on this platform yet."),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Capability {
    #[default]
    Supported,
    /// This build has no backend for the feature.
    Unsupported,
    /// The backend exists, but a service it needs is missing right now.
    Unavailable(String),
}

impl Capability {
    pub const fn of(supported: bool) -> Self {
        if supported {
            Self::Supported
        } else {
            Self::Unsupported
        }
    }

    pub fn is_supported(&self) -> bool {
        *self == Self::Supported
    }

    /// What an entry point reports when `feature`, in this state, cannot run.
    pub fn reason(&self, feature: Feature) -> Option<Cow<'static, str>> {
        match self {
            Self::Supported => None,
            Self::Unsupported => Some(feature.unsupported_reason()),
            Self::Unavailable(reason) => Some(Cow::Owned(reason.clone())),
        }
    }
}

const _: () = assert!(Feature::ALL.len() == Feature::SshRemotes as usize + 1);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Capabilities([Capability; Feature::ALL.len()]);

impl Capabilities {
    pub fn get(&self, feature: Feature) -> &Capability {
        &self.0[feature as usize]
    }

    pub fn set(&mut self, feature: Feature, capability: Capability) {
        self.0[feature as usize] = capability;
    }

    pub fn supports(&self, feature: Feature) -> bool {
        self.get(feature).is_supported()
    }

    /// Whether this build has a backend for `feature`, whatever its services report.
    pub fn has_backend(&self, feature: Feature) -> bool {
        *self.get(feature) != Capability::Unsupported
    }

    /// Fails with the reason when `feature` cannot run.
    pub fn check(&self, feature: Feature) -> Result<(), String> {
        self.reason(feature)
            .map_or(Ok(()), |reason| Err(reason.into_owned()))
    }

    /// What an entry point reports when it cannot offer `feature`.
    pub fn reason(&self, feature: Feature) -> Option<Cow<'static, str>> {
        self.get(feature).reason(feature)
    }

    /// Why this build cannot offer `feature` at all. `None` while a backend
    /// exists, whatever its services report.
    pub fn unsupported(&self, feature: Feature) -> Option<Cow<'static, str>> {
        (!self.has_backend(feature)).then(|| feature.unsupported_reason())
    }
}
