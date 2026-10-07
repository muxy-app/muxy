use super::*;
use muxy_terminal::Terminal;
use muxy_terminal::pty::{Pty, PtyEvent, PtySize, ReaderHandle, SpawnRequest};
use std::sync::mpsc::{self, Receiver};

pub(super) struct Tui {
    pty: Pty,
    reader: Option<ReaderHandle>,
    events: Receiver<PtyEvent>,
    screen: Terminal,
    exited: bool,
}

impl Tui {
    pub(super) fn start(binary: &Path, profile: &Path) -> Result<Self> {
        let pty = Pty::spawn(SpawnRequest {
            clear_env: false,
            program: binary.to_owned(),
            args: Vec::new(),
            cwd: profile.to_owned(),
            env: vec![
                ("MUXY_DIR".into(), profile.as_os_str().into()),
                ("TERM".into(), "xterm-256color".into()),
            ],
            size: PtySize {
                cols: 100,
                rows: 26,
            },
        })?;
        let (sender, events) = mpsc::channel();
        let reader = pty.start_reader(sender)?;
        Ok(Self {
            pty,
            reader: Some(reader),
            events,
            screen: Terminal::new(
                Size {
                    cols: 100,
                    rows: 26,
                },
                1024 * 1024,
            )?,
            exited: false,
        })
    }

    pub(super) fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.pty.write(bytes)?;
        Ok(())
    }

    fn pump(&mut self) -> Result<()> {
        if let Ok(PtyEvent::Output(bytes)) = self.events.recv_timeout(Duration::from_millis(20)) {
            self.screen.feed(&bytes);
            let response = self.screen.take_pty_output();
            if !response.is_empty() {
                self.pty.write(&response)?;
            }
        }
        Ok(())
    }

    pub(super) fn output(&mut self, expected: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.pump()?;
            let lines: Vec<String> = self
                .screen
                .screen()?
                .into_iter()
                .map(|row| row.runs.into_iter().map(|run| run.text).collect())
                .collect();
            if lines
                .iter()
                .any(|line| line.contains(expected) && !line.contains("printf"))
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!("Bundled TUI missed {expected}: {lines:?}").into());
            }
        }
    }

    fn signal(&self, signal: &str) -> Result<()> {
        run(Command::new("kill").args(["-s", signal, &self.pty.child_pid().to_string()]))?;
        Ok(())
    }

    pub(super) fn suspend(&mut self) -> Result<()> {
        self.signal("TSTP")?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.pump()?;
            let status = Command::new("ps")
                .args(["-o", "stat=", "-p", &self.pty.child_pid().to_string()])
                .output()?;
            if String::from_utf8(status.stdout)?.contains('T') {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("TUI did not suspend".into());
            }
        }
    }

    pub(super) fn resume(&mut self) -> Result<()> {
        self.signal("CONT")?;
        self.output("Ended")
    }

    pub(super) fn exit(&mut self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.pump()?;
            if let Some(status) = self.pty.try_wait() {
                self.exited = true;
                assert_eq!(status.code, Some(0));
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("TUI did not detach".into());
            }
        }
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        if !self.exited {
            let _ = self.pty.kill();
            let _ = self.pty.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
