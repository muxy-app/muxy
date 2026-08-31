use crate::client::{RendererClient, RendererEvent};
use crate::transport::{random_nonce, write_frame, write_structured};
use muxy_proto::session::codec::FrameKind;
use muxy_proto::session::{AttachRequest, BuildMode, Resize, SessionId, WindowSize};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub fn run(socket_path: &Path, session_id: SessionId, build_mode: BuildMode) -> io::Result<()> {
    let signals = block_resize_signal()?;
    let size = terminal_size(std::io::stdin().as_raw_fd());
    let nonce = random_nonce()?;
    let generation = u64::from_be_bytes(nonce[..8].try_into().unwrap());
    let mut renderer = RendererClient::connect(
        socket_path,
        build_mode,
        AttachRequest {
            session_id,
            attachment_generation: generation,
            size,
        },
    )?;
    let attachment_generation = renderer.attached.attachment_generation;
    let writer = Arc::new(Mutex::new(renderer.try_clone_stream()?));
    spawn_stdin(Arc::clone(&writer))?;
    spawn_resize(Arc::clone(&writer), signals, attachment_generation)?;
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    while let Some(event) = renderer.next_event()? {
        match event {
            RendererEvent::Output { bytes, .. } => {
                stdout.write_all(&bytes)?;
                stdout.flush()?;
            }
            RendererEvent::Exited(_) => return Ok(()),
        }
    }
    Ok(())
}

fn spawn_stdin(writer: Arc<Mutex<UnixStream>>) -> io::Result<()> {
    std::thread::Builder::new()
        .name("session-attach-input".into())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut stdin = stdin.lock();
            let mut bytes = vec![0; muxy_proto::session::MAX_STREAM_CHUNK_BYTES];
            loop {
                let count = match stdin.read(&mut bytes) {
                    Ok(0) | Err(_) => return,
                    Ok(count) => count,
                };
                let result = writer.lock().map_err(|_| ()).and_then(|mut stream| {
                    write_frame(&mut stream, FrameKind::Input, 0, &bytes[..count]).map_err(|_| ())
                });
                if result.is_err() {
                    return;
                }
            }
        })?;
    Ok(())
}

fn spawn_resize(
    writer: Arc<Mutex<UnixStream>>,
    mut signals: libc::sigset_t,
    attachment_generation: u64,
) -> io::Result<()> {
    std::thread::Builder::new()
        .name("session-attach-resize".into())
        .spawn(move || {
            let mut resize_generation = 0u64;
            loop {
                let mut signal = 0;
                if unsafe { libc::sigwait(&raw mut signals, &raw mut signal) } != 0 {
                    return;
                }
                resize_generation = match resize_generation.checked_add(1) {
                    Some(value) => value,
                    None => return,
                };
                let resize = Resize {
                    attachment_generation,
                    resize_generation,
                    size: terminal_size(libc::STDIN_FILENO),
                };
                let result = writer.lock().map_err(|_| ()).and_then(|mut stream| {
                    write_structured(&mut stream, FrameKind::Resize, 0, &resize).map_err(|_| ())
                });
                if result.is_err() {
                    return;
                }
            }
        })?;
    Ok(())
}

fn block_resize_signal() -> io::Result<libc::sigset_t> {
    let mut signals = unsafe { std::mem::zeroed::<libc::sigset_t>() };
    if unsafe { libc::sigemptyset(&raw mut signals) } != 0
        || unsafe { libc::sigaddset(&raw mut signals, libc::SIGWINCH) } != 0
        || unsafe {
            libc::pthread_sigmask(libc::SIG_BLOCK, &raw const signals, std::ptr::null_mut())
        } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(signals)
}

fn terminal_size(fd: libc::c_int) -> WindowSize {
    let mut size = unsafe { std::mem::zeroed::<libc::winsize>() };
    if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &raw mut size) } == 0 {
        WindowSize::new(size.ws_col, size.ws_row).create_or_fallback()
    } else {
        WindowSize::new(0, 0).create_or_fallback()
    }
}
