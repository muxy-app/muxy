#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

use std::io;

pub(crate) struct Requests {
    #[cfg(unix)]
    signals: signal_hook::iterator::Signals,
    #[cfg(windows)]
    console: windows::Console,
}

pub(crate) struct Listening {
    #[cfg(unix)]
    handle: signal_hook::iterator::Handle,
    #[cfg(unix)]
    thread: std::thread::JoinHandle<()>,
    #[cfg(windows)]
    session: windows::Session,
    #[cfg(windows)]
    _console: windows::Console,
}

impl Requests {
    pub(crate) fn new() -> io::Result<Self> {
        #[cfg(unix)]
        {
            use signal_hook::consts::{SIGINT, SIGTERM};
            Ok(Self {
                signals: signal_hook::iterator::Signals::new([SIGTERM, SIGINT])?,
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {
                console: windows::Console::install()?,
            })
        }
    }

    pub(crate) fn listen(self, stop: impl Fn() + Send + Sync + 'static) -> io::Result<Listening> {
        #[cfg(unix)]
        {
            let mut signals = self.signals;
            let handle = signals.handle();
            let thread = std::thread::Builder::new()
                .name("server-signals".into())
                .spawn(move || {
                    if signals.forever().next().is_some() {
                        stop();
                    }
                })?;
            Ok(Listening { handle, thread })
        }
        #[cfg(windows)]
        {
            windows::Console::listen(std::sync::Arc::new(stop));
            Ok(Listening {
                session: windows::Session::start()?,
                _console: self.console,
            })
        }
    }
}

impl Listening {
    pub(crate) fn close(self) -> io::Result<()> {
        #[cfg(unix)]
        {
            self.handle.close();
            self.thread
                .join()
                .map_err(|_| io::Error::other("signal thread panicked"))
        }
        #[cfg(windows)]
        {
            self.session.close()
        }
    }
}
