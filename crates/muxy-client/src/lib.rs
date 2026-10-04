//! Shared client connection, handshake, routing, and synchronization.
//!
//! This crate translates shared protocol traffic without owning project,
//! tab, pane, UI, or desktop update policy. Local startup launches an explicit executable.
//! Servers on other computers are reached through a bridge, `muxy stdio`,
//! which ssh runs there.

mod async_request;
mod asynchronous;
pub use async_request::Request;
pub mod bridge;
mod client;
mod error;
mod events;
mod grid;
mod handshake;
mod requests;
mod ssh;

pub use bridge::Start;
pub use client::{Attachment, Client};
pub use error::{ClientError, RemoteReason};
pub use events::ClientEvent;
pub use grid::{RunGrid, ScreenLinks, ScreenPrompts};
pub use ssh::SshTarget;

mod catalog;

pub mod local;

mod remote;
pub use remote::RemoteEndpoint;
