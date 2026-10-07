use muxy_protocol::SessionId;
use serde::{Deserialize, Serialize};

use crate::PaneId;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    pub id: PaneId,
    pub title: String,
    pub content: PaneContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<muxy_protocol::SandboxSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaneContent {
    Terminal { session: Option<SessionId> },
    Settings,
    Webview(crate::webview::WebviewDescriptor),
}
