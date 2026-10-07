use std::ffi::OsString;
use std::fs;
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use muxy_client::Client;
use muxy_terminal::pty::{ExitStatus, Pty, PtyEvent, PtySize, ReaderHandle, SpawnRequest};
use muxy_terminal::{Size, Terminal};
use serde_json::Value;

pub(super) type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
const SIZE: Size = Size {
    cols: 100,
    rows: 26,
};

pub(super) struct Fixture {
    pub directory: tempfile::TempDir,
}

impl Fixture {
    pub(super) fn new() -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("muxy-tui-")
            .tempdir_in("/tmp")?;
        fs::create_dir(directory.path().join("home"))?;
        fs::write(
            directory.path().join("server.toml"),
            "default_shell = \"/bin/sh\"\nshell_integration = false\n",
        )?;
        fs::write(directory.path().join("shell-env"), "PS1='tui-test> '\n")?;
        Ok(Self { directory })
    }

    pub(super) fn environment(&self) -> Vec<(OsString, OsString)> {
        vec![
            (
                "MUXY_SERVER_BIN".into(),
                super::support::binary()
                    .with_file_name("muxy-server")
                    .into(),
            ),
            ("MUXY_DIR".into(), self.directory.path().as_os_str().into()),
            (
                "HOME".into(),
                self.directory.path().join("home").into_os_string(),
            ),
            (
                "ENV".into(),
                self.directory.path().join("shell-env").into_os_string(),
            ),
            ("TERM".into(), "xterm-256color".into()),
            ("SHELL".into(), "/bin/sh".into()),
        ]
    }

    pub(super) fn command(&self) -> Command {
        let mut command = Command::new(super::support::binary());
        command.envs(self.environment());
        command
    }

    pub(super) fn client(&self) -> Result<Client> {
        let actual = self.directory.path().join("actual.sock");
        Ok(Client::connect(&if actual.exists() {
            actual
        } else {
            self.directory.path().join("server.sock")
        })?)
    }

    pub(super) fn state(&self) -> Result<Value> {
        let mut state: Value =
            serde_json::from_slice(&fs::read(self.directory.path().join("tui-state.json"))?)?;
        state
            .as_object_mut()
            .ok_or("TUI state object")?
            .remove("catalog_revision");
        Ok(state)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(client) = Client::connect_with_timeout(
            &self.directory.path().join("server.sock"),
            Duration::from_millis(200),
        ) {
            let _ = client.stop_server();
            let deadline = Instant::now() + Duration::from_secs(5);
            while self.directory.path().join("server.sock").exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

pub(super) struct Tui<'a> {
    fixture: &'a Fixture,
    pub pty: Pty,
    reader: Option<ReaderHandle>,
    events: Receiver<PtyEvent>,
    pub screen: Terminal,
    pub raw: Vec<u8>,
    status: Option<ExitStatus>,
    output_closed: bool,
}

impl<'a> Tui<'a> {
    pub(super) fn start(fixture: &'a Fixture, extra: &[(&str, &str)]) -> Result<Self> {
        Self::launch(fixture, extra, &[])
    }

    /// Starts `muxy` with `arguments`, such as `--host`.
    pub(super) fn launch(
        fixture: &'a Fixture,
        extra: &[(&str, &str)],
        arguments: &[&str],
    ) -> Result<Self> {
        let mut env = fixture.environment();
        env.extend(
            extra
                .iter()
                .map(|(key, value)| ((*key).into(), (*value).into())),
        );
        env.push(("MUXY_TEST_TUI_BIN".into(), super::support::binary().into()));
        let mut args = vec![
            "-c".into(),
            "stty -g > \"$MUXY_DIR/termios-$$\"; tty > \"$MUXY_DIR/tty-$$\"; exec \"$MUXY_TEST_TUI_BIN\" \"$@\"".into(),
            "sh".into(),
        ];
        args.extend(arguments.iter().map(Into::into));
        let pty = Pty::spawn(SpawnRequest {
            clear_env: false,
            program: "/bin/sh".into(),
            args,
            cwd: fixture.directory.path().to_owned(),
            env,
            size: PtySize {
                cols: SIZE.cols,
                rows: SIZE.rows,
            },
        })?;
        let (sender, events) = mpsc::channel();
        let reader = pty.start_reader(sender)?;
        Ok(Self {
            fixture,
            pty,
            reader: Some(reader),
            events,
            screen: Terminal::new(SIZE, 1024 * 1024)?,
            raw: Vec::new(),
            status: None,
            output_closed: false,
        })
    }

    pub(super) fn write(&mut self, bytes: &[u8]) -> Result {
        self.pty.write(bytes)?;
        Ok(())
    }

    pub(super) fn pump(&mut self) -> Result {
        match self.events.recv_timeout(Duration::from_millis(20)) {
            Ok(PtyEvent::Output(bytes)) => {
                self.screen.feed(&bytes);
                self.raw.extend(bytes);
                let response = self.screen.take_pty_output();
                if !response.is_empty() {
                    self.pty.write(&response)?;
                }
            }
            Ok(PtyEvent::Closed) => self.output_closed = true,
            Err(_) => {}
        }
        Ok(())
    }

    pub(super) fn wait(&mut self, mut predicate: impl FnMut(&mut Self) -> Result<bool>) -> Result {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            self.pump()?;
            if predicate(self)? {
                return Ok(());
            }
            if let Some(status) = self.pty.try_wait() {
                self.status = Some(status);
                break;
            }
        }
        let text = self.text()?;
        Err(format!(
            "TUI wait failed; status={:?}; screen={:?}; state={:?}",
            self.status,
            text,
            self.fixture.state()
        )
        .into())
    }

    pub(super) fn text(&mut self) -> Result<Vec<String>> {
        Ok(self
            .screen
            .screen()?
            .into_iter()
            .map(|row| row.runs.into_iter().map(|run| run.text).collect())
            .collect())
    }

    pub(super) fn output(&mut self, text: &str) -> Result {
        self.wait(|tui| {
            Ok(tui
                .text()?
                .iter()
                .any(|row| row.contains(text) && !row.contains("printf")))
        })
    }

    pub(super) fn cells(&mut self) -> Result<Vec<Vec<String>>> {
        Ok(self
            .screen
            .screen()?
            .into_iter()
            .map(|row| {
                let mut cells = Vec::new();
                for run in row.runs {
                    if run.text.is_ascii() && run.text.len() == usize::from(run.width) {
                        cells.extend(run.text.chars().map(|character| character.to_string()));
                    } else if run.width > 0 {
                        cells.push(run.text);
                        cells.extend(std::iter::repeat_n(
                            String::new(),
                            usize::from(run.width - 1),
                        ));
                    }
                }
                cells.resize(usize::from(SIZE.cols), " ".into());
                cells
            })
            .collect())
    }

    pub(super) fn ready(&mut self) -> Result {
        self.output("tui-test>")
    }

    pub(super) fn pick(&mut self, picker: u8, index: usize, expected: &str) -> Result {
        self.write(&[0x02, picker])?;
        self.output(expected)?;
        for _ in 0..index {
            self.write(b"\x1b[B")?;
        }
        self.write(b"\r")
    }

    pub(super) fn tabs(&self) -> Result<Vec<Value>> {
        let state = self.fixture.state()?;
        let active = state["active"].as_str().ok_or("active project")?;
        Ok(state["projects"][active]["tabs"]
            .as_array()
            .ok_or("tabs")?
            .clone())
    }

    pub(super) fn active_tab(&self) -> Result<Value> {
        let state = self.fixture.state()?;
        let active = state["active"].as_str().ok_or("active project")?;
        let project = &state["projects"][active];
        let index = project["active"].as_u64().ok_or("active tab")?;
        Ok(project["tabs"][usize::try_from(index)?].clone())
    }

    pub(super) fn exit(&mut self) -> Result<ExitStatus> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            self.pump()?;
            self.status = self.status.or_else(|| self.pty.try_wait());
            if self.output_closed
                && let Some(status) = self.status
            {
                return Ok(status);
            }
        }
        Err("TUI did not exit".into())
    }

    pub(super) fn detach(&mut self) -> Result {
        self.write(b"\x02d")?;
        assert_eq!(self.exit()?.code, Some(0));
        assert!(self.raw.windows(8).any(|bytes| bytes == b"\x1b[?1049l"));
        self.assert_restored()?;
        Ok(())
    }

    pub(super) fn assert_restored(&self) -> Result {
        let tty = fs::read_to_string(
            self.fixture
                .directory
                .path()
                .join(format!("tty-{}", self.pty.child_pid())),
        )?;
        let file = fs::File::open(tty.trim())?;
        let output = Command::new("stty").arg("-g").stdin(file).output()?;
        assert!(output.status.success(), "{:?}", output.stderr);
        let before = fs::read_to_string(
            self.fixture
                .directory
                .path()
                .join(format!("termios-{}", self.pty.child_pid())),
        )?;
        assert_eq!(String::from_utf8(output.stdout)?.trim(), before.trim());
        Ok(())
    }

    pub(super) fn signal(&self, signal: &str) -> Result {
        let status = Command::new("kill")
            .args(["-s", signal, &self.pty.child_pid().to_string()])
            .status()?;
        assert!(status.success());
        Ok(())
    }

    pub(super) fn stopped(&self) -> Result<bool> {
        let output = Command::new("ps")
            .args(["-o", "stat=", "-p", &self.pty.child_pid().to_string()])
            .output()?;
        Ok(String::from_utf8(output.stdout)?.contains('T'))
    }
}

impl Drop for Tui<'_> {
    fn drop(&mut self) {
        if self.status.is_none() {
            let _ = self.pty.kill();
            let _ = self.pty.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
