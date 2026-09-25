use muxy_protocol::{ClientKind, ErrorCode, RequestBody};

use crate::ServerError;

/// What a connection may ask for. A paired device can already reach a shell, so
/// these limits guard access management and the server's lifecycle, not data.
pub(super) fn permit(device: bool, body: &RequestBody) -> Result<(), ServerError> {
    match body {
        RequestBody::Authenticate(_) | RequestBody::Pair(_) => Err(ServerError::new(
            ErrorCode::BadRequest,
            "authentication is only the first request of a network connection",
        )),
        RequestBody::IdentifyClient(ClientKind::Mobile) if !device => Err(ServerError::new(
            ErrorCode::BadRequest,
            "only paired devices identify as mobile",
        )),
        RequestBody::ReadRemoteAccess
        | RequestBody::WriteRemoteAccess(_)
        | RequestBody::StartPairing
        | RequestBody::CancelPairing
        | RequestBody::RevokeDevice(_)
        | RequestBody::StopServer
        | RequestBody::StopServerIfIdle
        | RequestBody::WriteServerSettings(_)
        | RequestBody::Exec(_)
        | RequestBody::CancelExec(_)
        | RequestBody::IdentifyClient(ClientKind::Desktop | ClientKind::Tui | ClientKind::Cli) => {
            local_only(device)
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
        | RequestBody::Detach(_)
        | RequestBody::Resize { .. }
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
        | RequestBody::WriteInput { .. } => Ok(()),
    }
}

fn local_only(device: bool) -> Result<(), ServerError> {
    if device {
        Err(ServerError::new(
            ErrorCode::Unauthorized,
            "not available to paired devices",
        ))
    } else {
        Ok(())
    }
}
