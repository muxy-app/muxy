#[path = "support/printable.rs"]
mod support;

use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;

use muxy_client::{Client, ClientEvent, RemoteEndpoint, RunGrid};
use muxy_protocol::{DeviceCredential, DeviceKind, PairingInvite, Size};
use support::printable;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
type Saved = (Vec<String>, u16, [u8; 32], DeviceCredential);
const SIZE: Size = Size { cols: 80, rows: 24 };
const CREDENTIAL: &str = "credential.json";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "remote: {}", printable(&error.to_string()));
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [mode, folder] if mode == "pair" => pair(Path::new(folder)),
        [mode, folder] if mode == "connect" => connect(Path::new(folder)),
        _ => Err("usage: remote pair <folder> < link | connect <folder>".into()),
    }
}

fn pair(folder: &Path) -> Result {
    let file = credential_file(folder)?;
    let link = read_link()?;
    let invite =
        PairingInvite::parse_link(&link).map_err(|code| format!("invalid link: {code:?}"))?;
    let (_client, paired) = Client::pair(&invite, "Example device", DeviceKind::Phone)?;
    let saved: Saved = (
        invite.hosts,
        invite.port,
        invite.fingerprint,
        paired.credential,
    );
    let mut output = muxy_core::files::private_file()
        .write(true)
        .create_new(true)
        .open(&file)?;
    output.write_all(&serde_json::to_vec(&saved)?)?;
    writeln!(io::stdout(), "paired with {}", printable(&paired.name))?;
    Ok(())
}

fn credential_file(folder: &Path) -> Result<PathBuf> {
    match fs::read_dir(folder) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err(
                    format!("{} is not empty: choose a new folder", folder.display()).into(),
                );
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    muxy_core::files::private_dir(folder)?;
    Ok(folder.join(CREDENTIAL))
}

fn read_link() -> Result<String> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        write!(io::stderr(), "Pairing link: ")?;
        io::stderr().flush()?;
    }
    let mut link = String::new();
    stdin.lock().read_line(&mut link)?;
    let link = link.trim();
    if link.is_empty() {
        return Err("no pairing link on standard input".into());
    }
    Ok(link.to_owned())
}

fn connect(folder: &Path) -> Result {
    let (hosts, port, fingerprint, credential): Saved =
        serde_json::from_slice(&fs::read(folder.join(CREDENTIAL))?)?;
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
    let directory = home.directory.as_path();
    let session = client.create_session(directory, SIZE)?;
    let channel = client.attach(0, session.id, SIZE)?;
    let mut attachment = muxy_client::take_attached(&events, 0)?;
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
                let seq = attachment.terminal.apply(&frame);
                redraw(&attachment.terminal.grid)?;
                client.ack(channel, seq)?;
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
    for index in 0..grid.rows.len().min(usize::from(grid.size.rows)) {
        writeln!(stdout, "{}", printable(&grid.row_text(index)))?;
    }
    stdout.flush()
}
