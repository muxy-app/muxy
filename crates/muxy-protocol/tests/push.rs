use muxy_protocol::{
    ErrorCode, Message, PushEnvironment, PushPlatform, PushRegistration, PushStatus, PushToken,
    ReplyBody, RequestBody, RequestId,
};

fn apns(token: String, environment: Option<PushEnvironment>) -> PushToken {
    PushToken {
        platform: PushPlatform::Apns,
        token,
        environment,
    }
}

fn fcm(token: String, environment: Option<PushEnvironment>) -> PushToken {
    PushToken {
        platform: PushPlatform::Fcm,
        token,
        environment,
    }
}

fn register(token: PushToken) -> Result<(), ErrorCode> {
    Message::Request {
        id: RequestId(1),
        body: RequestBody::RegisterPush(token),
    }
    .validate()
}

fn registered(status: PushStatus, key: Option<[u8; 32]>) -> Result<(), ErrorCode> {
    Message::Reply {
        id: RequestId(1),
        body: ReplyBody::PushRegistered(PushRegistration { status, key }),
    }
    .validate()
}

#[test]
fn apns_tokens_are_hex_of_the_relays_length_with_an_environment() {
    let production = Some(PushEnvironment::Production);
    for token in [
        "a".repeat(64),
        "F".repeat(200),
        "0123456789abcdefABCDEF".repeat(4),
    ] {
        assert_eq!(register(apns(token, production)), Ok(()));
    }
    assert_eq!(
        register(apns("a".repeat(64), Some(PushEnvironment::Sandbox))),
        Ok(())
    );
    for token in [
        "a".repeat(63),
        "a".repeat(201),
        format!("{}g", "a".repeat(63)),
    ] {
        assert_eq!(
            register(apns(token, production)),
            Err(ErrorCode::BadRequest)
        );
    }
    for environment in [None, Some(PushEnvironment::Unrecognized(5))] {
        assert_eq!(
            register(apns("a".repeat(64), environment)),
            Err(ErrorCode::BadRequest)
        );
    }
}

#[test]
fn fcm_tokens_use_the_relays_characters_and_no_environment() {
    for token in [
        "a".repeat(32),
        "x".repeat(4096),
        "APA91b:abc_DEF-1.2".repeat(2),
    ] {
        assert_eq!(register(fcm(token, None)), Ok(()));
    }
    for token in [
        "a".repeat(31),
        "a".repeat(4097),
        format!("{} ", "a".repeat(32)),
    ] {
        assert_eq!(register(fcm(token, None)), Err(ErrorCode::BadRequest));
    }
    assert_eq!(
        register(fcm("a".repeat(32), Some(PushEnvironment::Production))),
        Err(ErrorCode::BadRequest)
    );
}

#[test]
fn platforms_this_build_does_not_know_are_refused() {
    let token = PushToken {
        platform: PushPlatform::Unrecognized(7),
        token: "a".repeat(64),
        environment: None,
    };
    assert_eq!(register(token), Err(ErrorCode::BadRequest));
}

#[test]
fn a_registration_carries_a_key_unless_push_is_off() {
    let key = Some([1; 32]);
    for status in [PushStatus::Active, PushStatus::Pending, PushStatus::Refused] {
        assert_eq!(registered(status, key), Ok(()));
        assert_eq!(registered(status, None), Err(ErrorCode::BadRequest));
    }
    assert_eq!(registered(PushStatus::Disabled, None), Ok(()));
    assert_eq!(
        registered(PushStatus::Disabled, key),
        Err(ErrorCode::BadRequest)
    );
    assert_eq!(registered(PushStatus::Unrecognized(9), None), Ok(()));
    assert_eq!(registered(PushStatus::Unrecognized(9), key), Ok(()));
}

#[test]
fn tokens_and_keys_stay_out_of_debug_output() {
    let token = apns("ab".repeat(32), Some(PushEnvironment::Production));
    assert!(!format!("{token:?}").contains("abab"));
    let registration = PushRegistration {
        status: PushStatus::Active,
        key: Some([0xab; 32]),
    };
    let debug = format!("{registration:?}");
    assert!(!debug.contains("171") && !debug.contains("ab"), "{debug}");
}
