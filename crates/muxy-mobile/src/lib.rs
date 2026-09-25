//! Rust core for the Muxy mobile apps, exposed to Swift and Kotlin through `UniFFI`.
//!
//! An app pairs once from a scanned link, keeps the returned credential in secure
//! storage, and connects with it afterwards. Calls block, so apps make them off
//! the main thread; connection events arrive on an SDK thread.

#![allow(unsafe_code, reason = "UniFFI generates the C ABI exports")]
#![allow(
    clippy::needless_pass_by_value,
    reason = "exported functions receive owned values from Swift and Kotlin"
)]

mod connection;
mod error;
mod files;
mod git;
mod keys;
mod mouse;
mod records;
mod scrollback;
mod terminal;
mod terminals;

use muxy_client::Client;
use muxy_protocol::{MAX_DEVICE_NAME, PairingInvite};

pub use connection::{Connection, ConnectionEvent, ConnectionListener};
pub use error::MobileError;
pub use files::{FileContent, FileEntry, FileInfo, FilesAction, FilesReply};
pub use git::{
    GitAction, GitBaseSwitch, GitBranch, GitChangesPreview, GitChecks, GitCommit, GitDiff,
    GitDiffKind, GitDiffRequest, GitDiffRow, GitFile, GitFileStatus, GitLineStat, GitMergeMethod,
    GitPreviewFile, GitPullRequest, GitPullRequestAction, GitPullRequestFilter, GitPushDestination,
    GitRawDiff, GitRef, GitRefKind, GitReply, GitRepoInfo, GitStatus, GitSummary, GitWorktree,
    WorktreeAction, WorktreeRemoval,
};
pub use keys::{Key, Modifiers};
pub use mouse::{MouseButton, ScrollDirection};
pub use records::{
    Activity, ActivityEvent, ActivityKind, Agent, AgentState, ClientKind, Cursor, CursorShape,
    Line, PairingLink, Project, Screen, ServerCredential, Session, SessionStatus, Span, Style,
    TerminalColor, Underline,
};
pub use scrollback::Scrollback;
pub use terminal::Terminal;

uniffi::setup_scaffolding!();

/// Checks a scanned code before pairing, and shows where it points.
#[uniffi::export]
pub fn parse_pairing_link(link: String) -> Result<PairingLink, MobileError> {
    let invite = invite(&link)?;
    Ok(PairingLink {
        hosts: invite.hosts,
        port: invite.port,
    })
}

/// Pairs this device with the server that showed the link.
#[uniffi::export]
pub fn pair(link: String, device_name: String) -> Result<ServerCredential, MobileError> {
    let invite = invite(&link)?;
    let (client, paired) = Client::pair(&invite, &device_label(&device_name))?;
    client.disconnect();
    Ok(ServerCredential {
        server_id: paired.server.to_string(),
        server_name: paired.name,
        hosts: invite.hosts,
        port: invite.port,
        fingerprint: invite.fingerprint.to_vec(),
        device_id: paired.credential.device.to_string(),
        token: paired.credential.token.to_vec(),
    })
}

fn invite(link: &str) -> Result<PairingInvite, MobileError> {
    PairingInvite::parse_link(link.trim()).map_err(|_| MobileError::InvalidLink)
}

/// Fits the name the phone reports to what the server shows for devices.
fn device_label(name: &str) -> String {
    let mut label = String::new();
    for character in name
        .trim()
        .chars()
        .filter(|character| !character.is_control())
    {
        if label.len() + character.len_utf8() > MAX_DEVICE_NAME {
            break;
        }
        label.push(character);
    }
    let label = label.trim_end();
    if label.is_empty() {
        "Phone".into()
    } else {
        label.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_names_fit_the_server_rules() {
        assert_eq!(device_label("  Saeed's iPhone\n"), "Saeed's iPhone");
        assert_eq!(device_label("\u{7}"), "Phone");
        let long = device_label(&"é".repeat(100));
        assert!(long.len() <= MAX_DEVICE_NAME);
        assert!(muxy_protocol::validate_device_name(&long).is_ok());
    }

    #[test]
    fn links_are_validated_before_pairing() {
        assert!(matches!(
            parse_pairing_link("https://example.com".into()),
            Err(MobileError::InvalidLink)
        ));
    }
}
