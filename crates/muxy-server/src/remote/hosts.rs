use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use muxy_protocol::MAX_PAIRING_HOSTS;

/// Addresses a phone can try, most direct first. Routing lookups send no packets.
pub(super) fn candidates() -> Vec<String> {
    let mut hosts = Vec::new();
    // A documentation address resolves through the default route.
    if let Some(address) = route("192.0.2.1:9") {
        hosts.push(address.to_string());
    }
    if let Some(IpAddr::V4(address)) = route("100.100.100.100:53")
        && is_tailscale(address)
    {
        hosts.push(address.to_string());
    }
    if let Some(name) = local_name() {
        hosts.push(name);
    }
    hosts.dedup();
    hosts.truncate(MAX_PAIRING_HOSTS);
    hosts
}

/// The machine's name without a local-network suffix, for display on the phone.
pub(super) fn display_name() -> String {
    let name = node_name();
    let name = name.strip_suffix(".local").unwrap_or(&name);
    if name.is_empty() {
        "Muxy".into()
    } else {
        name.into()
    }
}

fn route(target: &str) -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(target).ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_loopback() && !address.is_unspecified()).then_some(address)
}

fn is_tailscale(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    first == 100 && (64..128).contains(&second)
}

fn local_name() -> Option<String> {
    let name = node_name();
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
    {
        return None;
    }
    Some(if name.contains('.') {
        name
    } else {
        format!("{name}.local")
    })
}

fn node_name() -> String {
    rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_tailscale_range_counts_as_tailscale() {
        assert!(is_tailscale(Ipv4Addr::new(100, 64, 0, 1)));
        assert!(is_tailscale(Ipv4Addr::new(100, 127, 255, 254)));
        assert!(!is_tailscale(Ipv4Addr::new(100, 128, 0, 1)));
        assert!(!is_tailscale(Ipv4Addr::new(192, 168, 1, 5)));
    }

    #[test]
    fn candidates_are_valid_pairing_hosts() {
        for host in candidates() {
            assert!(!host.is_empty() && host.len() <= 253, "{host}");
            assert!(
                host.bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-'),
                "{host}"
            );
        }
        assert!(!display_name().is_empty());
    }
}
