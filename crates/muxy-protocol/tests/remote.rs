use std::error::Error;

use muxy_protocol::{
    DeviceCredential, DeviceId, DeviceKind, ErrorCode, PairRequest, PairingInvite,
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
        kind: DeviceKind::Phone,
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
fn pairing_from_an_older_app_is_a_phone() -> Result<(), Box<dyn Error>> {
    let mut bytes = Vec::new();
    minicbor::Encoder::new(&mut bytes)
        .array(2)?
        .bytes(&[5; 16])?
        .str("Phone")?;
    let request: PairRequest = minicbor::decode(&bytes)?;
    assert_eq!(request.kind, DeviceKind::Phone);
    Ok(())
}

#[test]
fn device_kinds_from_a_newer_build_are_kept() -> Result<(), Box<dyn Error>> {
    let request = PairRequest {
        secret: [5; 16],
        name: "Robot".into(),
        kind: DeviceKind::Unrecognized(9),
    };
    let decoded: PairRequest = minicbor::decode(&minicbor::to_vec(&request)?)?;
    assert_eq!(decoded, request);
    Ok(())
}
