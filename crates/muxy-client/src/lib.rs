//! Shared client connection, handshake, routing, and synchronization.
//!
//! This crate translates shared protocol traffic without owning project,
//! tab, pane, UI, or desktop update policy. Local startup launches an explicit executable.
//! Servers on other computers are reached through a bridge, `muxy stdio`,
//! which ssh runs there.

mod typed;

mod async_request;
mod asynchronous;
mod attachment;
mod backoff;
pub use async_request::Request;
pub mod bridge;
mod client;
mod error;
mod error_tail;
mod events;
#[cfg(test)]
mod fake;
mod files;
mod git;
mod grid;
mod handshake;
pub mod input;
mod requests;
mod ssh;

pub use attachment::{AttachedTerminal, Attachment};
pub use backoff::Backoff;
pub use bridge::Start;
pub use client::Client;
pub use error::{ClientError, RemoteReason};
pub use error_tail::ErrorTail;
pub use events::{ClientEvent, ClipboardWrite, take_attached};
pub use git::AddedWorktree;
pub use grid::{RunGrid, ScreenLinks, ScreenPrompts};
pub use ssh::{Askpass, ProxyCommand, SshTarget};

mod catalog;

pub mod local;

mod remote;
pub use remote::{RemoteEndpoint, accepts_connections};
