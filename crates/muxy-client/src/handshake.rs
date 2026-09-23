use muxy_protocol::wire::WireError;
use muxy_protocol::{CONTROL, ChannelId, Message, SUPPORTED, Version, validate_versions};

use crate::ClientError;

pub(crate) fn hello() -> Message {
    Message::Hello {
        versions: SUPPORTED.to_vec(),
        compatibility: muxy_protocol::COMPATIBILITY,
    }
}

pub(crate) fn accept(
    received: Result<(ChannelId, Message), WireError>,
) -> Result<(Version, muxy_protocol::ServerInfo), ClientError> {
    match received {
        Ok((CONTROL, Message::HelloReply { versions, server })) => {
            if server.build.compatibility != muxy_protocol::COMPATIBILITY {
                return Err(ClientError::VersionUnsupported);
            }
            validate_versions(&versions).map_err(ClientError::Invalid)?;
            versions
                .into_iter()
                .filter(|version| SUPPORTED.contains(version))
                .max()
                .map(|version| (version, server))
                .ok_or(ClientError::VersionUnsupported)
        }
        Ok((CONTROL, Message::VersionUnsupported)) | Err(WireError::Decode(_)) => {
            Err(ClientError::VersionUnsupported)
        }
        Ok((CONTROL, Message::Fatal(error)))
            if error.code == muxy_protocol::ErrorCode::BadRequest
                && error.message.starts_with("invalid wire payload:") =>
        {
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
    use muxy_protocol::{ErrorCode, ErrorReply};

    use super::*;

    #[test]
    fn accepts_a_common_version_and_rejects_everything_else() {
        assert!(
            matches!(accept(Ok((CONTROL, Message::HelloReply { versions: vec![Version(99), muxy_protocol::V1], server: muxy_protocol::ServerInfo::current() }))), Ok((version, _)) if version == muxy_protocol::V1)
        );
        assert!(
            accept(Ok((
                CONTROL,
                Message::HelloReply {
                    versions: vec![Version(u16::MAX), muxy_protocol::V1],
                    server: muxy_protocol::ServerInfo::current()
                }
            )))
            .is_ok()
        );
        assert!(matches!(
            accept(Ok((
                CONTROL,
                Message::HelloReply {
                    versions: vec![Version(u16::MAX)],
                    server: muxy_protocol::ServerInfo::current()
                }
            ))),
            Err(ClientError::VersionUnsupported)
        ));
        assert!(matches!(
            accept(Ok((
                CONTROL,
                Message::HelloReply {
                    versions: vec![],
                    server: muxy_protocol::ServerInfo::current()
                }
            ))),
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
    fn beta_mismatches_report_a_server_update_without_hiding_other_failures() {
        for compatibility in [
            muxy_protocol::COMPATIBILITY - 1,
            muxy_protocol::COMPATIBILITY + 1,
        ] {
            let mut server = muxy_protocol::ServerInfo::current();
            server.build.compatibility = compatibility;
            assert!(matches!(
                accept(Ok((
                    CONTROL,
                    Message::HelloReply {
                        versions: SUPPORTED.to_vec(),
                        server
                    }
                ))),
                Err(ClientError::VersionUnsupported)
            ));
        }
        let legacy_reply = [9, 0, 0, 0, 1, 0, 0, 0, 0, 0, 4, 1, 1];
        assert!(matches!(
            accept(muxy_protocol::wire::Decoder::new(legacy_reply.as_slice()).next()),
            Err(ClientError::VersionUnsupported)
        ));
        let rejected = Message::Fatal(ErrorReply {
            code: ErrorCode::BadRequest,
            message: "invalid wire payload: The original data was not well encoded".into(),
        });
        let error =
            accept(Ok((CONTROL, rejected))).expect_err("old server rejects the changed hello");
        assert!(matches!(error, ClientError::VersionUnsupported));
        assert!(
            error
                .to_string()
                .contains("app and server use incompatible beta builds")
        );
        assert!(
            error
                .to_string()
                .contains("Restarting the server will end active terminal sessions")
        );
        assert!(matches!(
            accept(Ok((
                CONTROL,
                Message::Fatal(ErrorReply {
                    code: ErrorCode::BadRequest,
                    message: "expected Hello on control".into()
                })
            ))),
            Err(ClientError::Protocol(_))
        ));
    }
}
