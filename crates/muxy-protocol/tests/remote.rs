use muxy_protocol::{
    DeviceCredential, DeviceId, ErrorCode, ListenerStatus, Message, PairRequest, PairedDevice,
    PairingInvite, RemoteAccessSettings, RemoteAccessState, ReplyBody, RequestBody, RequestId,
};

fn invite() -> PairingInvite {
    PairingInvite {
        hosts: vec!["192.168.1.5".into(), "studio-mac.local".into()],
        port: 7419,
        fingerprint: [0xc3; 32],
        secret: [0xab; 16],
    }
}

#[test]
fn pairing_links_round_trip_and_stay_compact() -> Result<(), ErrorCode> {
    let invite = invite();
    let link = invite.to_link();
    assert_eq!(
        link,
        format!(
            "muxy://pair?v=1&h=192.168.1.5&h=studio-mac.local&p=7419&f={}&s={}",
            "c3".repeat(32),
            "ab".repeat(16)
        )
    );
    assert_eq!(PairingInvite::parse_link(&link)?, invite);
    let uppercase = link.replace(&"c3".repeat(32), &"C3".repeat(32));
    assert_eq!(PairingInvite::parse_link(&uppercase)?, invite);
    Ok(())
}

#[test]
fn malformed_pairing_links_are_rejected() {
    let link = invite().to_link();
    let fingerprint = "c3".repeat(32);
    let secret = "ab".repeat(16);
    let too_many_hosts = "&h=host".repeat(9);
    let cases = [
        String::new(),
        "https://example.com".into(),
        link.replace("v=1", "v=2"),
        link.replace("&p=7419", ""),
        link.replace("&p=7419", "&p=80"),
        link.replace("&p=7419", "&p=+7419"),
        link.replace("&p=7419", "&p=70000"),
        link.replace(&fingerprint[..4], "zz"),
        link.replace(&secret, &secret[..30]),
        link.replace("&h=192.168.1.5", "&h=bad host"),
        link.replace("&h=192.168.1.5", "&h="),
        link.replace("&h=192.168.1.5&h=studio-mac.local", ""),
        format!("{link}&p=7420"),
        format!("{link}&x=1"),
        format!("{link}&h"),
        format!("muxy://pair?v=1{too_many_hosts}&p=7419&f={fingerprint}&s={secret}"),
        format!("{link}&h={}", "a".repeat(4096)),
    ];
    for case in cases {
        assert_eq!(
            PairingInvite::parse_link(&case),
            Err(ErrorCode::BadRequest),
            "{case}"
        );
    }
}

#[test]
fn secrets_never_appear_in_debug_output() {
    let credential = DeviceCredential {
        device: DeviceId::from_u128(1),
        token: [0x5a; 32],
    };
    let request = PairRequest {
        secret: [0x5a; 16],
        name: "Phone".into(),
    };
    for text in [
        format!("{credential:?}"),
        format!("{request:?}"),
        format!("{:?}", invite()),
    ] {
        for secret in ["90, 90", "5a5a", "171, 171", "abab"] {
            assert!(!text.contains(secret), "{text}");
        }
    }
}

#[test]
fn remote_requests_and_replies_validate_their_bounds() {
    let request = |body| Message::Request {
        id: RequestId(1),
        body,
    };
    let reply = |body| Message::Reply {
        id: RequestId(1),
        body,
    };
    for name in ["", "   ", "tab\tname", &"x".repeat(65)] {
        let pair = request(RequestBody::Pair(PairRequest {
            secret: [0; 16],
            name: name.into(),
        }));
        assert_eq!(pair.validate(), Err(ErrorCode::BadRequest), "{name:?}");
    }
    let low_port = request(RequestBody::WriteRemoteAccess(RemoteAccessSettings {
        enabled: true,
        port: 1023,
    }));
    assert_eq!(low_port.validate(), Err(ErrorCode::BadRequest));
    let device = PairedDevice {
        id: DeviceId::from_u128(1),
        name: "Phone".into(),
        paired_at: 1,
        last_seen: None,
        connected: false,
    };
    let state = |devices: usize, status| RemoteAccessState {
        revision: 1,
        settings: RemoteAccessSettings::default(),
        status,
        pairing_expires_at: None,
        devices: vec![device.clone(); devices],
    };
    assert!(
        reply(ReplyBody::RemoteAccess(state(
            64,
            ListenerStatus::Listening
        )))
        .validate()
        .is_ok()
    );
    for invalid in [
        state(65, ListenerStatus::Disabled),
        state(1, ListenerStatus::Failed("x".repeat(1025))),
    ] {
        assert_eq!(
            reply(ReplyBody::RemoteAccess(invalid)).validate(),
            Err(ErrorCode::BadRequest)
        );
    }
    let mut empty_hosts = invite();
    empty_hosts.hosts.clear();
    assert_eq!(empty_hosts.validate(), Err(ErrorCode::BadRequest));
}
