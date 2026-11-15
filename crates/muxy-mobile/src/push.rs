use muxy_protocol::PushToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum PushPlatform {
    #[doc = " Apple Push Notification service, on iOS."]
    Apns,
    #[doc = " Firebase Cloud Messaging, on Android."]
    Fcm,
}

#[doc = " Which APNs servers an iOS build's token belongs to."]
#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum PushEnvironment {
    #[doc = " Debug builds."]
    Sandbox,
    #[doc = " `TestFlight` and App Store builds."]
    Production,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum PushStatus {
    #[doc = " The relay accepted the token."]
    Active,
    #[doc = " The server couldn't reach the relay yet and keeps trying; the key already works."]
    Pending,
    #[doc = " Apple or Google rejected the token. Get a fresh one, and check the APNs environment."]
    Refused,
    #[doc = " Push notifications are off on this server. There is no key."]
    Disabled,
    #[doc = " A status from a newer server."]
    Other,
}

#[doc = " The server's answer to `registerPush`."]
#[derive(Clone, Eq, PartialEq, uniffi::Record)]
pub struct PushRegistration {
    pub status: PushStatus,
    #[doc = " 32 bytes. Store it with the server's credential, replacing the one"]
    #[doc = " kept before; notifications from this server decrypt with it."]
    pub key: Option<Vec<u8>>,
}

impl std::fmt::Debug for PushRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PushRegistration")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

pub(crate) fn token(
    platform: PushPlatform,
    token: String,
    environment: Option<PushEnvironment>,
) -> PushToken {
    PushToken {
        platform: match platform {
            PushPlatform::Apns => muxy_protocol::PushPlatform::Apns,
            PushPlatform::Fcm => muxy_protocol::PushPlatform::Fcm,
        },
        token,
        environment: environment.map(|environment| match environment {
            PushEnvironment::Sandbox => muxy_protocol::PushEnvironment::Sandbox,
            PushEnvironment::Production => muxy_protocol::PushEnvironment::Production,
        }),
    }
}

impl From<muxy_protocol::PushRegistration> for PushRegistration {
    fn from(registration: muxy_protocol::PushRegistration) -> Self {
        Self {
            status: match registration.status {
                muxy_protocol::PushStatus::Active => PushStatus::Active,
                muxy_protocol::PushStatus::Pending => PushStatus::Pending,
                muxy_protocol::PushStatus::Refused => PushStatus::Refused,
                muxy_protocol::PushStatus::Disabled => PushStatus::Disabled,
                muxy_protocol::PushStatus::Unrecognized(_) => PushStatus::Other,
            },
            key: registration.key.map(|key| key.to_vec()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_name_the_service_and_the_apns_environment() {
        let apns = token(
            PushPlatform::Apns,
            "ab".repeat(32),
            Some(PushEnvironment::Sandbox),
        );
        assert_eq!(apns.platform, muxy_protocol::PushPlatform::Apns);
        assert_eq!(
            apns.environment,
            Some(muxy_protocol::PushEnvironment::Sandbox)
        );
        assert_eq!(apns.validate(), Ok(()));
        let fcm = token(PushPlatform::Fcm, "f".repeat(140), None);
        assert_eq!(fcm.platform, muxy_protocol::PushPlatform::Fcm);
        assert_eq!(fcm.validate(), Ok(()));
    }

    #[test]
    fn registrations_carry_the_key_and_show_newer_statuses_neutrally() {
        let active = PushRegistration::from(muxy_protocol::PushRegistration {
            status: muxy_protocol::PushStatus::Active,
            key: Some([7; 32]),
        });
        assert_eq!(active.status, PushStatus::Active);
        assert_eq!(active.key, Some(vec![7; 32]));
        assert!(!format!("{active:?}").contains('7'));
        let newer = PushRegistration::from(muxy_protocol::PushRegistration {
            status: muxy_protocol::PushStatus::Unrecognized(8),
            key: None,
        });
        assert_eq!(newer.status, PushStatus::Other);
    }
}
