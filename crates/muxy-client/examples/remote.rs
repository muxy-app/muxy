//! A minimal paired device: pair from a `muxy://pair` link, then reconnect,
//! list projects, and type into a new shell in the server's Home project.

use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, BufRead, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::ExitCode;
use std::thread;

use muxy_client::{Client, ClientEvent, RemoteEndpoint, RunGrid};
use muxy_protocol::{DeviceCredential, PairingInvite, Size};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
type Saved = (Vec<String>, u16, [u8; 32], DeviceCredential);
const SIZE: Size = Size { cols: 80, rows: 24 };

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "remote: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [mode, link, file] if mode == "pair" => pair(link, Path::new(file)),
        [mode, file] if mode == "connect" => connect(Path::new(file)),
        _ => Err("usage: remote pair <link> <credential-file> | connect <credential-file>".into()),
    }
}

fn pair(link: &str, file: &Path) -> Result {
    let invite =
        PairingInvite::parse_link(link).map_err(|code| format!("invalid link: {code:?}"))?;
    // The file will hold the device token: create it private before pairing.
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(file)?;
    let (_client, paired) = match Client::pair(&invite, "Example device") {
        Ok(paired) => paired,
        Err(error) => {
            let _ = fs::remove_file(file);
            return Err(error.into());
        }
    };
    let saved: Saved = (
        invite.hosts,
        invite.port,
        invite.fingerprint,
        paired.credential,
    );
    output.write_all(&serde_json::to_vec(&saved)?)?;
    writeln!(io::stdout(), "paired with {}", paired.name)?;
    Ok(())
}

fn connect(file: &Path) -> Result {
    let (hosts, port, fingerprint, credential): Saved = serde_json::from_slice(&fs::read(file)?)?;
    let endpoint = RemoteEndpoint {
        hosts,
        port,
        fingerprint,
    };
    let client = Client::connect_remote(&endpoint, credential)?;
    let catalog = client.catalog()?;
    for project in &catalog.projects {
        writeln!(io::stdout(), "project: {}", project.name.escape_debug())?;
    }
    let home = catalog
        .projects
        .iter()
        .find(|project| project.home)
        .ok_or("the server has no Home project")?;
    let events = client.events().ok_or("event stream already taken")?;
    let directory = Path::new(std::ffi::OsStr::from_bytes(&home.directory.0));
    let session = client.create_session(directory, SIZE)?;
    let mut attachment = client.attach(session.id, SIZE)?;
    let channel = attachment.channel;
    let input = client.clone();
    thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        let mut line = Vec::new();
        while stdin
            .read_until(b'\n', &mut line)
            .is_ok_and(|read| read > 0)
        {
            if input.send_input(channel, &line).is_err() {
                break;
            }
            line.clear();
        }
    });
    for event in events {
        match event {
            ClientEvent::Frame {
                channel: received,
                frame,
            } if received == channel => {
                attachment.grid.apply(&frame);
                redraw(&attachment.grid)?;
                client.ack(channel, frame.seq)?;
            }
            ClientEvent::SessionEnded { session: ended, .. } if ended == session.id => {
                return Ok(());
            }
            ClientEvent::Disconnected | ClientEvent::ServerRestarting => {
                return Err("disconnected from server".into());
            }
            _ => {}
        }
    }
    Ok(())
}

fn redraw(grid: &RunGrid) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(b"\x1b[2J\x1b[H")?;
    for index in 0..grid.rows.len() {
        writeln!(stdout, "{}", grid.row_text(index))?;
    }
    stdout.flush()
}
