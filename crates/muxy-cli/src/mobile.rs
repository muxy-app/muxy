//! `muxy mobile`: let phones connect, pair them, and revoke them.

use std::collections::HashSet;
use std::error::Error;
use std::io::{self, Write};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use muxy_app_core::qr::{QUIET_ZONE, QrCode};
use muxy_client::{Client, ClientEvent};
use muxy_protocol::{ListenerStatus, RemoteAccessSettings, RemoteAccessState};

use crate::args::Mobile;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

pub(crate) fn run(command: Mobile, client: &Client) -> Result {
    match command {
        Mobile::Status => print_state(&client.read_remote_access()?),
        Mobile::Enable { port } => {
            let current = client.read_remote_access()?.settings;
            let state = client.write_remote_access(RemoteAccessSettings {
                enabled: true,
                port: port.unwrap_or(current.port),
            })?;
            print_state(&state)?;
            match state.status {
                ListenerStatus::Failed(message) => {
                    Err(format!("mobile access could not start: {message}").into())
                }
                ListenerStatus::Listening
                | ListenerStatus::Disabled
                | ListenerStatus::Unrecognized(_) => Ok(()),
            }
        }
        Mobile::Disable => {
            let current = client.read_remote_access()?.settings;
            print_state(&client.write_remote_access(RemoteAccessSettings {
                enabled: false,
                ..current
            })?)
        }
        Mobile::Pair => pair(client),
        Mobile::Revoke { device } => revoke(client, &device),
    }
}

fn print_state(state: &RemoteAccessState) -> Result {
    let mut stdout = io::stdout().lock();
    match (&state.status, state.settings.enabled) {
        (ListenerStatus::Listening, _) => writeln!(
            stdout,
            "Mobile access: on, listening on port {}",
            state.settings.port
        )?,
        (ListenerStatus::Failed(message), _) => {
            writeln!(stdout, "Mobile access: not listening: {message}")?;
        }
        (ListenerStatus::Disabled, true) => writeln!(stdout, "Mobile access: starting")?,
        (ListenerStatus::Disabled, false) => writeln!(stdout, "Mobile access: off")?,
        (ListenerStatus::Unrecognized(_), _) => writeln!(
            stdout,
            "Mobile access: status unknown to this version of muxy"
        )?,
    }
    if state.devices.is_empty() {
        writeln!(stdout, "No paired devices.")?;
        return Ok(());
    }
    writeln!(stdout, "Paired devices:")?;
    let now = now();
    for device in &state.devices {
        let seen = if device.connected {
            "connected".to_owned()
        } else {
            device.last_seen.map_or_else(
                || "never connected".to_owned(),
                |seen| format!("last seen {}", ago(now.saturating_sub(seen))),
            )
        };
        let id = device.id.to_string();
        writeln!(
            stdout,
            "  {}  {}  {seen}",
            &id[..8],
            device.name.escape_debug()
        )?;
    }
    Ok(())
}

fn revoke(client: &Client, prefix: &str) -> Result {
    let state = client.read_remote_access()?;
    let matches: Vec<_> = state
        .devices
        .iter()
        .filter(|device| device.id.to_string().starts_with(prefix))
        .collect();
    match matches.as_slice() {
        [device] => {
            client.revoke_device(device.id)?;
            writeln!(io::stdout(), "Revoked {}.", device.name.escape_debug())?;
            Ok(())
        }
        [] => Err(format!("no paired device starts with {prefix}").into()),
        _ => Err(format!("more than one device starts with {prefix}; use more characters").into()),
    }
}

/// Shows a pairing code until a new device pairs, the code expires, or Ctrl-C
/// closes the connection, which cancels the code.
fn pair(client: &Client) -> Result {
    let events = client.events().ok_or("event stream already taken")?;
    let known: HashSet<_> = client
        .read_remote_access()?
        .devices
        .iter()
        .map(|device| device.id)
        .collect();
    let offer = client.start_pairing()?;
    let link = offer.invite.to_link();
    let code = QrCode::encode(&link).ok_or("the pairing link does not fit in a QR code")?;
    let mut stdout = io::stdout().lock();
    print_code(&mut stdout, &code)?;
    writeln!(
        stdout,
        "\nScan with the Muxy app, or open this link on the phone:\n{link}\n\nWaiting for the phone. The code expires in 5 minutes; Ctrl-C cancels it."
    )?;
    stdout.flush()?;
    drop(stdout);
    let started = Instant::now();
    let lifetime = Duration::from_secs(offer.expires_at.saturating_sub(now()));
    loop {
        match events.recv_timeout(lifetime.saturating_sub(started.elapsed())) {
            Ok(ClientEvent::RemoteAccessChanged { .. }) => {
                let state = client.read_remote_access()?;
                if let Some(device) = state
                    .devices
                    .iter()
                    .find(|device| !known.contains(&device.id))
                {
                    writeln!(io::stdout(), "Paired {}.", device.name.escape_debug())?;
                    return Ok(());
                }
                if state.pairing_expires_at.is_none() {
                    return Err("pairing was cancelled".into());
                }
            }
            Ok(ClientEvent::Disconnected | ClientEvent::ServerRestarting)
            | Err(RecvTimeoutError::Disconnected) => {
                return Err("disconnected from the server".into());
            }
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => return Err("the pairing code expired".into()),
        }
    }
}

/// Two module rows per line, dark on light regardless of the terminal theme.
fn print_code(output: &mut impl Write, code: &QrCode) -> io::Result<()> {
    let span = code.size() + 2 * QUIET_ZONE;
    let dark = |x: usize, y: usize| {
        x >= QUIET_ZONE && y >= QUIET_ZONE && code.dark(x - QUIET_ZONE, y - QUIET_ZONE)
    };
    for y in (0..span).step_by(2) {
        write!(output, "\x1b[30;47m")?;
        for x in 0..span {
            let glyph = match (dark(x, y), dark(x, y + 1)) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            };
            write!(output, "{glyph}")?;
        }
        writeln!(output, "\x1b[0m")?;
    }
    Ok(())
}

fn ago(seconds: u64) -> String {
    match seconds {
        0..60 => "just now".into(),
        60..3_600 => format!("{} min ago", seconds / 60),
        3_600..86_400 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} d ago", seconds / 86_400),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_print_as_half_blocks_inside_a_quiet_zone() -> io::Result<()> {
        let code = QrCode::encode("muxy://pair?v=1").ok_or(io::ErrorKind::InvalidData)?;
        let mut output = Vec::new();
        print_code(&mut output, &code)?;
        let text = String::from_utf8(output).map_err(|_| io::ErrorKind::InvalidData)?;
        let lines: Vec<_> = text.lines().collect();
        let span = code.size() + 2 * QUIET_ZONE;
        assert_eq!(lines.len(), span.div_ceil(2));
        assert!(lines.iter().all(|line| line.starts_with("\x1b[30;47m")));
        assert!(
            lines[0]
                .trim_start_matches("\x1b[30;47m")
                .starts_with("    ")
        );
        assert!(text.contains('█'));
        Ok(())
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(ago(5), "just now");
        assert_eq!(ago(125), "2 min ago");
        assert_eq!(ago(7_200), "2 h ago");
        assert_eq!(ago(200_000), "2 d ago");
    }
}
