use muxy_protocol::{ClientKind, DeviceKind, ErrorCode, RequestBody};

use crate::ServerError;

/// What a connection may ask for: `device` is the paired device on the other
/// end of a network connection. A paired device can already reach a shell, so
/// these limits guard access management and the server's lifecycle, not data.
/// A paired computer may do what a desktop app on this computer does, except
/// manage which devices may connect.
pub(super) fn permit(device: Option<DeviceKind>, body: &RequestBody) -> Result<(), ServerError> {
    match body {
        RequestBody::Authenticate(_) | RequestBody::Pair(_) => Err(ServerError::new(
            ErrorCode::BadRequest,
            "authentication is only the first request of a network connection",
        )),
        RequestBody::IdentifyClient(ClientKind::Unrecognized(_)) => Err(ServerError::new(
            ErrorCode::Unsupported,
            "this server doesn't know the client kind",
        )),
        RequestBody::ReadRemoteAccess
        | RequestBody::WriteRemoteAccess(_)
        | RequestBody::StartPairing
        | RequestBody::StartPairingWithHosts(_)
        | RequestBody::CancelPairing
        | RequestBody::RevokeDevice(_) => local_only(device),
        RequestBody::StopServer
        | RequestBody::StopServerIfIdle
        | RequestBody::WriteServerSettings(_)
        | RequestBody::Exec(_)
        | RequestBody::CancelExec(_)
        | RequestBody::IdentifyClient(ClientKind::Desktop | ClientKind::Tui | ClientKind::Cli) => {
            desktop_only(device)
        }
        RequestBody::IdentifyClient(ClientKind::Mobile)
        | RequestBody::CancelCreation(_)
        | RequestBody::ListSessions
        | RequestBody::ReadCatalog { .. }
        | RequestBody::MutateProject(_)
        | RequestBody::ListProjectSessions { .. }
        | RequestBody::CreateSession { .. }
        | RequestBody::EndSession(_)
        | RequestBody::Attach { .. }
        | RequestBody::AttachWithoutResize(_)
        | RequestBody::Detach(_)
        | RequestBody::Resize { .. }
        | RequestBody::ClearScreen(_)
        | RequestBody::Ping
        | RequestBody::ReadSavedScreen(_)
        | RequestBody::DiscardSession(_)
        | RequestBody::HistoryPage { .. }
        | RequestBody::SavedHistoryPage { .. }
        | RequestBody::Search { .. }
        | RequestBody::SetTerminalColors(_)
        | RequestBody::ReadServerSettings
        | RequestBody::SyncSessionReferences { .. }
        | RequestBody::CloseSession { .. }
        | RequestBody::Git(_)
        | RequestBody::ReadActivity
        | RequestBody::AcknowledgeActivity(_)
        | RequestBody::ClaimActivity(_)
        | RequestBody::Files(_)
        | RequestBody::ListFolders(_)
        | RequestBody::Upload(_)
        | RequestBody::WriteInput { .. } => Ok(()),
    }
}

/// Only connections from this computer.
fn local_only(device: Option<DeviceKind>) -> Result<(), ServerError> {
    match device {
        None => Ok(()),
        Some(_) => Err(ServerError::new(
            ErrorCode::Unauthorized,
            "not available to paired devices",
        )),
    }
}

/// Connections from this computer or from a paired computer.
fn desktop_only(device: Option<DeviceKind>) -> Result<(), ServerError> {
    match device {
        None | Some(DeviceKind::Computer) => Ok(()),
        Some(DeviceKind::Phone | DeviceKind::Unrecognized(_)) => Err(ServerError::new(
            ErrorCode::Unauthorized,
            "not available to paired phones",
        )),
    }
}

#[cfg(test)]
mod tests {
    use muxy_protocol::{ClientKind, DeviceId, DeviceKind, RequestBody};

    use super::permit;

    fn managing_access() -> [RequestBody; 3] {
        [
            RequestBody::ReadRemoteAccess,
            RequestBody::StartPairing,
            RequestBody::RevokeDevice(DeviceId::new()),
        ]
    }

    fn desktop_work() -> [RequestBody; 3] {
        [
            RequestBody::IdentifyClient(ClientKind::Desktop),
            RequestBody::CancelExec(1),
            RequestBody::StopServer,
        ]
    }

    #[test]
    fn this_computer_may_ask_for_anything() {
        for body in managing_access().into_iter().chain(desktop_work()) {
            assert!(permit(None, &body).is_ok(), "{body:?}");
        }
    }

    #[test]
    fn a_paired_computer_works_like_a_desktop_but_cannot_manage_access() {
        let computer = Some(DeviceKind::Computer);
        for body in desktop_work() {
            assert!(permit(computer, &body).is_ok(), "{body:?}");
        }
        for body in managing_access() {
            assert!(permit(computer, &body).is_err(), "{body:?}");
        }
    }

    #[test]
    fn phones_and_unknown_devices_keep_a_phones_rights() {
        for kind in [DeviceKind::Phone, DeviceKind::Unrecognized(9)] {
            for body in managing_access().into_iter().chain(desktop_work()) {
                assert!(permit(Some(kind), &body).is_err(), "{kind:?} {body:?}");
            }
            assert!(permit(Some(kind), &RequestBody::ListSessions).is_ok());
            assert!(permit(Some(kind), &RequestBody::IdentifyClient(ClientKind::Mobile)).is_ok());
        }
    }
}
