//! Shared client connection, handshake, routing, and synchronization.
//!
//! This crate translates shared protocol traffic without owning project,
//! tab, pane, UI, or desktop update policy. Local startup launches an explicit executable.

mod async_request;
mod asynchronous;
pub use async_request::Request;
mod client;
mod error;
mod events;
mod grid;
mod handshake;
mod requests;

pub use client::{Attachment, Client};
pub use error::ClientError;
pub use events::ClientEvent;
pub use grid::{RunGrid, ScreenLinks, ScreenPrompts};

mod catalog;

pub mod local;

mod remote;
pub use remote::RemoteEndpoint;
