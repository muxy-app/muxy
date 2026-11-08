//! `muxy mobile`: let phones connect, pair them, and revoke them.

use std::collections::HashSet;
use std::error::Error;
use std::io::{self, Write};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use muxy_app_core::qr::{QUIET_ZONE, QrCode};
use muxy_client::{Client, ClientEvent};
use muxy_protocol::{Feature, ListenerStatus, PushStatus, RemoteAccessSettings, RemoteAccessState};

use crate::args::Mobile;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

pub(crate) fn run(command: Mobile, client: &Client) -> Result {
    let push = client.supports(Feature::PUSH_NOTIFICATIONS);
    let show = |state: &RemoteAccessState| print_state(state, push);
    match command {
        Mobile::Status => show(&client.read_remote_access()?),
        Mobile::Enable { port } => {
            let current = client.read_remote_access()?.settings;
            let state = client.write_remote_access(RemoteAccessSettings {
                enabled: true,
                port: port.unwrap_or(current.port),
            })?;
            show(&state)?;
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
            show(&client.write_remote_access(RemoteAccessSettings {
                enabled: false,
                ..current
            })?)
        }
        Mobile::Pair { addresses } => pair(client, addresses),
        Mobile::Revoke { device } => revoke(client, &device),
        Mobile::Push { .. } | Mobile::ResetPush { .. } if !push => {
            Err("this server can't send push notifications; update Muxy on it".into())
        }
        Mobile::Push { on } => {
            let state = client.set_push_notifications(on)?;
            writeln!(io::stdout(), "{}", switched(&state, now()))?;
            Ok(())
        }
        Mobile::ResetPush { device } => reset_push(client, &device),
    }
}

/// What switching push notifications did, in a sentence or two.
fn switched(state: &RemoteAccessState, now: u64) -> String {
    let phones = state
        .devices
        .iter()
        .filter(|device| device.push.is_some())
        .count();
    let counted = match phones {
        1 => "1 phone".to_owned(),
        count => format!("{count} phones"),
    };
    match (state.push_notifications, phones) {
        (true, 0) => {
            "Push notifications are on. Phones get them once they turn them on in the Muxy app."
                .into()
        }
        (true, 1) => "Push notifications are on. 1 phone resumes now, without reconnecting.".into(),
        (true, _) => {
            format!("Push notifications are on. {counted} resume now, without reconnecting.")
        }
        (false, 0) => "Push notifications are off. Nothing is sent to muxy.app.".into(),
        (false, _) => format!(
            "Push notifications are off. Nothing is sent to muxy.app, which forgot {counted}. \
             This computer keeps their registrations for {}, so `muxy mobile push on` resumes them.",
            kept(state.push_kept_until, now)
        ),
    }
}

fn kept(until: Option<u64>, now: u64) -> String {
    match until.map(|until| until.saturating_sub(now).div_ceil(86_400)) {
        Some(1) => "1 more day".into(),
        Some(days) => format!("{days} more days"),
        None => "a while".into(),
    }
}

fn reset_push(client: &Client, prefix: &str) -> Result {
    let state = client.read_remote_access()?;
    let matches: Vec<_> = state
        .devices
        .iter()
        .filter(|device| device.id.to_string().starts_with(prefix))
        .collect();
    match matches.as_slice() {
        [device] if device.push.is_none() => Err(format!(
            "{} has no push registration to reset",
            device.name.escape_debug()
        )
        .into()),
        [device] => {
            client.reset_push(device.id)?;
            writeln!(
                io::stdout(),
                "Reset notifications for {}. It gets a new key the next time it connects; until then, this computer sends it nothing.",
                device.name.escape_debug()
            )?;
            Ok(())
        }
        [] => Err(format!("no paired device starts with {prefix}").into()),
        _ => Err(format!("more than one device starts with {prefix}; use more characters").into()),
    }
}

fn print_state(state: &RemoteAccessState, push: bool) -> Result {
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
    if push {
        let paused = state
            .devices
            .iter()
            .filter(|device| device.push.is_some())
            .count();
        match (state.push_notifications, paused) {
            (true, _) => writeln!(stdout, "Push notifications: on")?,
            (false, 0) => writeln!(stdout, "Push notifications: off")?,
            (false, _) => writeln!(
                stdout,
                "Push notifications: off; {paused} paused, kept for {}",
                kept(state.push_kept_until, now())
            )?,
        }
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
        let notifications = device
            .push
            .map(|status| format!("  {}", push_label(status)))
            .unwrap_or_default();
        writeln!(
            stdout,
            "  {}  {}  {seen}{notifications}",
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
fn pair(client: &Client, addresses: Vec<String>) -> Result {
    let events = client.events().ok_or("event stream already taken")?;
    let known: HashSet<_> = client
        .read_remote_access()?
        .devices
        .iter()
        .map(|device| device.id)
        .collect();
    let offer = client.start_pairing_with(addresses)?;
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

fn push_label(status: PushStatus) -> &'static str {
    match status {
        PushStatus::Active => "notifications on",
        PushStatus::Pending => "notifications pending",
        PushStatus::Refused => "notifications refused by Apple or Google",
        PushStatus::Disabled => "notifications paused",
        PushStatus::Unrecognized(_) => "notifications unknown",
    }
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
    use muxy_protocol::{
        DeviceId, DeviceKind, ListenerStatus, PairedDevice, PushStatus, RemoteAccessSettings,
        RemoteAccessState,
    };

    use super::switched;

    fn state(on: bool, phones: usize, kept_until: Option<u64>) -> RemoteAccessState {
        RemoteAccessState {
            revision: 1,
            settings: RemoteAccessSettings::default(),
            status: ListenerStatus::Listening,
            pairing_expires_at: None,
            devices: (0..phones)
                .map(|_| PairedDevice {
                    id: DeviceId::new(),
                    name: "Phone".into(),
                    paired_at: 0,
                    last_seen: None,
                    connected: false,
                    kind: DeviceKind::Phone,
                    push: Some(if on {
                        PushStatus::Active
                    } else {
                        PushStatus::Disabled
                    }),
                })
                .collect(),
            push_notifications: on,
            push_kept_until: kept_until,
        }
    }

    #[test]
    fn switching_says_what_happens_to_the_phones() {
        let day = 86_400;
        assert_eq!(
            switched(&state(false, 2, Some(90 * day)), 0),
            "Push notifications are off. Nothing is sent to muxy.app, which forgot 2 phones. \
             This computer keeps their registrations for 90 more days, so `muxy mobile push on` resumes them."
        );
        let one = switched(&state(false, 1, Some(day / 2)), 0);
        assert!(
            one.contains("forgot 1 phone.") && one.contains("for 1 more day,"),
            "{one}"
        );
        assert_eq!(
            switched(&state(true, 1, None), 0),
            "Push notifications are on. 1 phone resumes now, without reconnecting."
        );
        assert!(switched(&state(true, 2, None), 0).contains("2 phones resume now"));
        assert!(switched(&state(false, 0, None), 0).ends_with("Nothing is sent to muxy.app."));
        assert!(switched(&state(true, 0, None), 0).contains("turn them on in the Muxy app"));
    }
}
