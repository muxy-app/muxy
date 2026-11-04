use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

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

/// Whether this runs in a login over SSH, away from the computer's screen.
pub(crate) fn over_ssh() -> bool {
    ["SSH_CONNECTION", "SSH_TTY"]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
}

pub(crate) type Terminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>;

#[derive(Clone, Debug, Default)]
pub(crate) struct Redraw {
    pending: Arc<AtomicBool>,
    sender: Option<mpsc::SyncSender<HostEvent>>,
}

impl Redraw {
    pub(crate) fn request(&self) {
        if !self.pending.swap(true, Ordering::AcqRel)
            && let Some(sender) = &self.sender
        {
            let _ = sender.try_send(HostEvent::Wake);
        }
    }

    pub(crate) fn take(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }
}

#[derive(Debug)]
enum HostEvent {
    Input(io::Result<event::Event>),
    Wake,
}

pub(crate) struct Events {
    pub redraw: Redraw,
    receiver: Receiver<HostEvent>,
    stop: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    paused: Receiver<()>,
    resume: mpsc::SyncSender<()>,
}

impl Events {
    pub(crate) fn start() -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(128);
        let redraw = Redraw {
            sender: Some(sender.clone()),
            ..Redraw::default()
        };
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let pause = Arc::new(AtomicBool::new(false));
        let pausing = Arc::clone(&pause);
        let (ready, paused) = mpsc::sync_channel(1);
        let (resume, resuming) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("muxy-tui-host-input".into())
            .spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    let event = match event::poll(Duration::from_millis(100)) {
                        Ok(false) => {
                            if pausing.load(Ordering::Acquire)
                                && (ready.send(()).is_err() || resuming.recv().is_err())
                            {
                                break;
                            }
                            continue;
                        }
                        Ok(true) => event::read(),
                        Err(error) => Err(error),
                    };
                    let failed = event.is_err();
                    if pausing.load(Ordering::Acquire) && !failed {
                        continue;
                    }
                    if sender.send(HostEvent::Input(event)).is_err() || failed {
                        break;
                    }
                }
            })?;
        Ok(Self {
            redraw,
            receiver,
            stop,
            pause,
            paused,
            resume,
        })
    }

    pub(crate) fn next(&self, timeout: Duration) -> io::Result<Option<event::Event>> {
        match self.receiver.recv_timeout(timeout) {
            Ok(HostEvent::Input(event)) => event.map(Some),
            Ok(HostEvent::Wake) | Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(io::Error::other("Terminal input closed"))
            }
        }
    }

    fn pause(&self) -> io::Result<()> {
        self.pause.store(true, Ordering::Release);
        while self.receiver.try_recv().is_ok() {}
        if let Err(error) = self.paused.recv_timeout(Duration::from_secs(1)) {
            self.resume();
            return Err(io::Error::other(format!(
                "Terminal input could not pause: {error}"
            )));
        }
        while self.receiver.try_recv().is_ok() {}
        Ok(())
    }

    fn resume(&self) {
        if self.pause.swap(false, Ordering::AcqRel) {
            let _ = self.resume.try_send(());
        }
    }
}

impl Drop for Events {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.resume();
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

    pub(crate) fn resume_if_needed(&mut self, events: &Events) -> io::Result<bool> {
        if !self.suspend.swap(false, Ordering::AcqRel) {
            return Ok(false);
        }
        execute!(io::stdout(), event::DisableMouseCapture)?;
        events.pause()?;
        restore();
        signal_hook::low_level::raise(SIGSTOP)?;
        self.activate()?;
        events.resume();
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

fn discard_pending_input() {
    let input = io::stdin();
    let started = Instant::now();
    let mut quiet = started;
    while started.elapsed() < Duration::from_millis(100) {
        match rustix::io::ioctl_fionread(&input) {
            Ok(0) if quiet.elapsed() >= Duration::from_millis(10) => break,
            Ok(0) => {}
            Ok(_) => {
                if rustix::termios::tcflush(&input, rustix::termios::QueueSelector::IFlush).is_err()
                {
                    return;
                }
                quiet = Instant::now();
            }
            Err(_) => return,
        }
        thread::sleep(Duration::from_millis(1));
    }
    let _ = rustix::termios::tcflush(input, rustix::termios::QueueSelector::IFlush);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events() -> (Events, mpsc::SyncSender<HostEvent>, mpsc::SyncSender<()>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        let (ready, paused) = mpsc::sync_channel(1);
        let (resume, _) = mpsc::sync_channel(1);
        let events = Events {
            redraw: Redraw {
                sender: Some(sender.clone()),
                ..Redraw::default()
            },
            receiver,
            stop: Arc::default(),
            pause: Arc::default(),
            paused,
            resume,
        };
        (events, sender, ready)
    }

    #[test]
    fn redraw_requests_coalesce_and_wake_again_after_take() -> io::Result<()> {
        let (events, _, _) = events();
        events.redraw.request();
        events.redraw.request();
        assert!(events.next(Duration::ZERO)?.is_none());
        assert!(events.receiver.try_recv().is_err());
        assert!(events.redraw.take());
        assert!(!events.redraw.take());
        events.redraw.request();
        assert!(matches!(events.receiver.try_recv(), Ok(HostEvent::Wake)));
        assert!(events.redraw.take());
        Ok(())
    }

    #[test]
    fn redraw_survives_a_full_event_channel() -> io::Result<()> {
        let (events, sender, _) = events();
        sender
            .try_send(HostEvent::Input(Ok(event::Event::FocusGained)))
            .map_err(io::Error::other)?;
        events.redraw.request();
        assert_eq!(
            events.next(Duration::ZERO)?,
            Some(event::Event::FocusGained)
        );
        assert!(events.redraw.take());
        assert!(!events.redraw.take());
        Ok(())
    }

    #[test]
    fn redraw_survives_pausing_and_draining_host_events() -> io::Result<()> {
        let (events, _, ready) = events();
        events.redraw.request();
        ready.send(()).map_err(io::Error::other)?;
        events.pause()?;
        assert!(events.receiver.try_recv().is_err());
        assert!(events.redraw.take());
        assert!(!events.redraw.take());
        events.resume();
        events.redraw.request();
        assert!(matches!(events.receiver.try_recv(), Ok(HostEvent::Wake)));
        Ok(())
    }

    #[test]
    fn default_redraw_retains_requests_without_a_receiver() {
        let redraw = Redraw::default();
        let other = redraw.clone();
        other.request();
        assert!(redraw.take());
        assert!(!other.take());
    }
}
