use muxy_protocol::wire::WireError;
use muxy_protocol::{
    CONTROL, ChannelId, Feature, Message, SUPPORTED, ServerInfo, validate_versions,
};

use crate::ClientError;

pub(crate) fn hello() -> Message {
    Message::Hello {
        versions: SUPPORTED.to_vec(),
    }
}

/// The server's identity and the features it offers, once a shared version is found.
pub(crate) fn accept(
    received: Result<(ChannelId, Message), WireError>,
) -> Result<(ServerInfo, Vec<Feature>), ClientError> {
    match received {
        Ok((
            CONTROL,
            Message::HelloReply {
                versions,
                server,
                features,
            },
        )) => {
            validate_versions(&versions).map_err(ClientError::Invalid)?;
            if versions.iter().any(|version| SUPPORTED.contains(version)) {
                Ok((server, features))
            } else {
                Err(ClientError::VersionUnsupported)
            }
        }
        // Servers from before V2 answer in a framing this build doesn't read.
        Ok((CONTROL, Message::VersionUnsupported))
        | Err(WireError::UnsupportedVersion(_) | WireError::Decode(_)) => {
            Err(ClientError::VersionUnsupported)
        }
        Ok((CONTROL, Message::Fatal(error))) => Err(ClientError::Protocol(error.message)),
        Ok((channel, message)) => Err(ClientError::Protocol(format!(
            "expected HelloReply on control, got {message:?} on channel {}",
            channel.0
        ))),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use muxy_protocol::{ErrorCode, ErrorReply, Version};

    use super::*;

    fn reply(versions: Vec<Version>) -> (ChannelId, Message) {
        (
            CONTROL,
            Message::HelloReply {
                versions,
                server: ServerInfo::current(),
                features: vec![Feature(7)],
            },
        )
    }

    #[test]
    fn accepts_a_shared_version_and_rejects_everything_else() {
        assert!(
            matches!(accept(Ok(reply(vec![Version(99), muxy_protocol::CURRENT]))), Ok((_, features)) if features == [Feature(7)])
        );
        assert!(matches!(
            accept(Ok(reply(vec![Version(u16::MAX)]))),
            Err(ClientError::VersionUnsupported)
        ));
        assert!(matches!(
            accept(Ok(reply(vec![]))),
            Err(ClientError::Invalid(ErrorCode::BadRequest))
        ));
        assert!(matches!(
            accept(Ok((CONTROL, Message::VersionUnsupported))),
            Err(ClientError::VersionUnsupported)
        ));
        assert!(matches!(
            accept(Ok((CONTROL, Message::Fatal(ErrorReply { code: ErrorCode::BadRequest, message: "bad".into() })))),
            Err(ClientError::Protocol(message)) if message == "bad"
        ));
        assert!(matches!(
            accept(Ok((ChannelId(1), Message::VersionUnsupported))),
            Err(ClientError::Protocol(_))
        ));
        assert!(matches!(
            accept(Err(WireError::Closed)),
            Err(ClientError::Disconnected)
        ));
    }

    #[test]
    fn servers_from_before_v2_report_an_update() {
        // A pre-V2 server rejects the V2 hello in its own version-1 framing.
        let legacy_fatal = [9, 0, 0, 0, 1, 0, 0, 0, 0, 0, 8, 1, 1];
        let error = accept(muxy_protocol::wire::Decoder::new(legacy_fatal.as_slice()).next())
            .expect_err("a version-1 frame is unreadable");
        assert!(matches!(error, ClientError::VersionUnsupported));
        assert!(error.to_string().contains("can't talk to each other"));
    }
}
