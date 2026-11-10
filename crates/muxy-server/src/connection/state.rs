use std::collections::HashMap;

use muxy_protocol::{
    ClientKind, DeviceId, DeviceKind, SessionClient, SessionId, TerminalColors, Topic,
};

/// Invalidations a connection asked for by reading the matching state.
#[allow(
    clippy::struct_excessive_bools,
    reason = "Each watch is an independent subscription"
)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Watches {
    pub(super) catalog: bool,
    pub(super) activity: bool,
    pub(super) remote_access: bool,
    /// Every session's metadata, whether or not the connection shows it.
    pub(super) metadata: bool,
}

impl Watches {
    pub(super) fn includes(self, topic: Topic) -> bool {
        match topic {
            Topic::Catalog | Topic::Sessions => self.catalog,
            Topic::Activity => self.activity,
            Topic::RemoteAccess => self.remote_access,
            Topic::Unrecognized(_) => false,
        }
    }
}

/// How screens sent on a connection carry image pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ImageDelivery {
    /// Each screen that changes an image carries the pixels on show.
    #[default]
    Send,
    /// The app reads image pixels itself, so screens leave them out.
    Defer,
    /// Each channel is sent the pixels on show once, as its screens go out.
    Stream,
}

/// Who is on the other end of a connection, and what it asked for.
#[derive(Debug, Default)]
pub(super) struct ConnectionState {
    pub(super) client: SessionClient,
    /// The paired device on the other end of a network connection.
    pub(super) device: Option<(DeviceId, DeviceKind)>,
    pub(super) watches: Watches,
    pub(super) colors: Option<TerminalColors>,
    pub(super) images: ImageDelivery,
    pub(super) session_colors: HashMap<SessionId, TerminalColors>,
}

impl ConnectionState {
    /// A computer says which app it is once connected, as local apps do.
    pub(super) fn for_device(device: DeviceId, kind: DeviceKind) -> Self {
        let mut state = Self {
            device: Some((device, kind)),
            ..Self::default()
        };
        if kind != DeviceKind::Computer {
            state.client.kind = ClientKind::Mobile;
        }
        state
    }

    pub(super) fn colors_for(&self, session: SessionId) -> Option<&TerminalColors> {
        self.session_colors.get(&session).or(self.colors.as_ref())
    }
}
