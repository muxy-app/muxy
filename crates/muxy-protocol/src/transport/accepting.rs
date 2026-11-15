use std::io;
use std::net::Shutdown;
use std::sync::{Mutex, PoisonError};

use rustix::event::{PollFd, PollFlags};
use rustix::fd::AsFd;

use crate::transport::LocalStream;
use crate::transport::unix::socket_pair;

#[derive(Debug)]
pub(crate) struct Accepting<S> {
    socket: Mutex<Option<S>>,
    wake: LocalStream,
    waker: LocalStream,
}

impl<S: AsFd> Accepting<S> {
    pub(crate) fn new(socket: S) -> io::Result<Self> {
        let (wake, waker) = socket_pair()?;
        Ok(Self {
            socket: Mutex::new(Some(socket)),
            wake,
            waker,
        })
    }

    pub(crate) fn accept<T>(&self, accept: impl Fn(&S) -> io::Result<T>) -> io::Result<T> {
        let guard = self
            .socket
            .lock()
            .map_err(|_| io::Error::other("listener lock poisoned"))?;
        loop {
            let socket = guard.as_ref().ok_or_else(closed)?;
            match accept(socket) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if self.wait(socket)? {
                        return Err(closed());
                    }
                }
                result => return result,
            }
        }
    }

    fn wait(&self, socket: &S) -> io::Result<bool> {
        let mut descriptors = [
            PollFd::new(socket, PollFlags::IN),
            PollFd::new(&self.wake, PollFlags::IN),
        ];
        match rustix::event::poll(&mut descriptors, None) {
            Ok(_) | Err(rustix::io::Errno::INTR) => Ok(!descriptors[1].revents().is_empty()),
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) fn close(&self) {
        let _ = self.waker.shutdown(Shutdown::Write);
        self.socket
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
}

fn closed() -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, "listener is closed")
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    use super::*;

    #[test]
    fn close_wakes_a_pending_accept() -> io::Result<()> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        socket.set_nonblocking(true)?;
        let listener = Arc::new(Accepting::new(socket)?);
        let (accepted, result) = mpsc::channel();
        let accepting = Arc::clone(&listener);
        std::thread::spawn(move || {
            let _ = accepted.send(accepting.accept(|socket| socket.accept().map(|_| ())));
        });
        while listener.socket.try_lock().is_ok() {
            std::thread::yield_now();
        }
        listener.close();
        let error = result
            .recv_timeout(Duration::from_secs(5))
            .map_err(io::Error::other)?
            .err();
        assert_eq!(
            error.map(|error| error.kind()),
            Some(io::ErrorKind::NotConnected)
        );
        Ok(())
    }
}
