use muxy_protocol::{AttachSnapshot, ChannelId, ForegroundProcess, ScreenFrame, ServerPath};

use crate::RunGrid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    pub terminal: AttachedTerminal,
    pub server_input: bool,
    pub title: String,
    pub directory: ServerPath,
    pub process: Option<ForegroundProcess>,
}

impl Attachment {
    pub(crate) fn new(
        snapshot: AttachSnapshot,
        process: Option<ForegroundProcess>,
        server_input: bool,
    ) -> Self {
        Self {
            terminal: AttachedTerminal::new(snapshot.channel, RunGrid::from_snapshot(&snapshot)),
            server_input,
            title: snapshot.title,
            directory: snapshot.directory,
            process,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedTerminal {
    pub grid: RunGrid,
    channel: ChannelId,
    seq: u64,
}

impl AttachedTerminal {
    pub fn new(channel: ChannelId, grid: RunGrid) -> Self {
        Self {
            grid,
            channel,
            seq: 0,
        }
    }

    pub fn channel(&self) -> ChannelId {
        self.channel
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn apply(&mut self, frame: &ScreenFrame) -> u64 {
        self.grid.apply(frame);
        self.seq = frame.seq;
        frame.seq
    }
}
