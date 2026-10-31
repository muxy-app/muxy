use muxy_protocol::SessionId;
use serde::{Deserialize, Serialize};

use crate::PaneId;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    pub id: PaneId,
    pub title: String,
    pub content: PaneContent,
    /// The terminal started with the app's proxy.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub proxy: bool,
}

impl Pane {
    /// The session a terminal pane shows, once it has one.
    pub fn session(&self) -> Option<SessionId> {
        match self.content {
            PaneContent::Terminal { session } => session,
            PaneContent::Settings
            | PaneContent::Webview(_)
            | PaneContent::Browser(_)
            | PaneContent::Servers => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaneContent {
    Terminal {
        session: Option<SessionId>,
    },
    Settings,
    Webview(crate::webview::WebviewDescriptor),
    Browser(crate::browser::BrowserPane),
    /// The Devices tab: the servers this computer connects to, and the devices
    /// paired with it.
    Servers,
}
