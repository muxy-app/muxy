use std::collections::HashMap;

use muxy_protocol::{
    ClientKind, DeviceId, DeviceKind, SessionClient, SessionId, TerminalColors, Topic,
};

/// Invalidations a connection asked for by reading the matching state.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Watches {
    pub(super) catalog: bool,
    pub(super) activity: bool,
    pub(super) remote_access: bool,
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

/// Who is on the other end of a connection, and what it asked for.
#[derive(Debug, Default)]
pub(super) struct ConnectionState {
    pub(super) client: SessionClient,
    /// The paired device on the other end of a network connection.
    pub(super) device: Option<(DeviceId, DeviceKind)>,
    pub(super) watches: Watches,
    pub(super) colors: Option<TerminalColors>,
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
