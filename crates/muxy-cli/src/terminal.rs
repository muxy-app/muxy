use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use ratatui::crossterm::{cursor, event, execute, terminal};
use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGSTOP, SIGTERM, SIGTSTP};

pub(crate) fn require_interactive() -> io::Result<()> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        Ok(())
    } else {
        Err(io::Error::other(
            "interactive mode requires terminal stdin and stdout",
        ))
    }
}

pub(crate) type Terminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>;

pub(crate) struct Events {
    receiver: Receiver<io::Result<event::Event>>,
    stop: Arc<AtomicBool>,
}

impl Events {
    pub(crate) fn start() -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(128);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        thread::Builder::new()
            .name("muxy-tui-host-input".into())
            .spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    let event = match event::poll(Duration::from_millis(16)) {
                        Ok(false) => continue,
                        Ok(true) => event::read(),
                        Err(error) => Err(error),
                    };
                    let failed = event.is_err();
                    if sender.send(event).is_err() || failed {
                        break;
                    }
                }
            })?;
        Ok(Self { receiver, stop })
    }

    pub(crate) fn next(&self) -> io::Result<Option<event::Event>> {
        match self.receiver.recv_timeout(Duration::from_millis(16)) {
            Ok(event) => event.map(Some),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(io::Error::other("Terminal input closed"))
            }
        }
    }
}

impl Drop for Events {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Crossterm 0.29's Mio reader can spin on a revoked TTY. Never join it:
        // the main loop still restores the terminal and exits this CLI process.
    }
}

pub(crate) struct Host {
    pub terminal: Terminal,
    pub stop: Arc<AtomicBool>,
    suspend: Arc<AtomicBool>,
    _signals: Signals,
}

impl Host {
    pub(crate) fn enter() -> io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let suspend = Arc::new(AtomicBool::new(false));
        let mut signals = Signals::default();
        for signal in [SIGHUP, SIGINT, SIGTERM] {
            signals
                .0
                .push(signal_hook::flag::register(signal, Arc::clone(&stop))?);
        }
        signals
            .0
            .push(signal_hook::flag::register(SIGTSTP, Arc::clone(&suspend))?);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            previous(info);
        }));
        let mut host = Self {
            terminal: Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout()))?,
            stop,
            suspend,
            _signals: signals,
        };
        host.activate()?;
        Ok(host)
    }

    fn activate(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(
            io::stdout(),
            terminal::EnterAlternateScreen,
            cursor::Hide,
            event::EnableBracketedPaste,
            event::EnableFocusChange,
            event::EnableMouseCapture
        )?;
        self.terminal.clear()
    }

    pub(crate) fn resume_if_needed(&mut self) -> io::Result<bool> {
        if !self.suspend.swap(false, Ordering::AcqRel) {
            return Ok(false);
        }
        restore();
        signal_hook::low_level::raise(SIGSTOP)?;
        self.activate()?;
        Ok(true)
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        restore();
    }
}

#[derive(Default)]
struct Signals(Vec<signal_hook::SigId>);

impl Drop for Signals {
    fn drop(&mut self) {
        for signal in self.0.drain(..) {
            signal_hook::low_level::unregister(signal);
        }
    }
}

fn restore() {
    let _ = execute!(
        io::stdout(),
        event::DisableMouseCapture,
        event::DisableFocusChange,
        event::DisableBracketedPaste,
        cursor::SetCursorStyle::DefaultUserShape,
        cursor::Show,
        terminal::LeaveAlternateScreen
    );
    let _ = io::stdout().flush();
    discard_pending_input();
    let _ = terminal::disable_raw_mode();
}

/// Reads away mouse reports already on their way, so none reach the shell
/// once the terminal stops reporting.
fn discard_pending_input() {
    for _ in 0..64 {
        if !event::poll(Duration::from_millis(5)).unwrap_or(false) || event::read().is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_restores_terminal_settings_and_output_modes() -> Result<(), Box<dyn std::error::Error>>
    {
        if std::env::var_os("MUXY_TEST_HOST_PANIC").is_some() {
            let _host = Host::enter()?;
            panic!("deliberate terminal cleanup test");
        }
        let output = std::process::Command::new("python3")
            .args(["-c", include_str!("terminal/panic.py")])
            .arg(std::env::current_exe()?)
            .env("TERM", "xterm-256color")
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}
