import Foundation

enum TerminalSessionBacking: Hashable {
    case direct
    case local(UUID)
    case remoteTmux(RemoteTmuxSession)

    var localSessionID: UUID? {
        guard case let .local(sessionID) = self else { return nil }
        return sessionID
    }

    static func resolve(
        paneID: UUID,
        sessionID: UUID?,
        workspaceContext: WorkspaceContext,
        usesLocalPersistentSession: Bool,
        remoteSessionMode: SSHRemoteSessionMode? = nil,
        remoteTmuxDestination: SSHDestination? = nil
    ) -> TerminalSessionBacking {
        let resolvedSessionID = sessionID ?? paneID
        if usesLocalPersistentSession {
            return .local(resolvedSessionID)
        }
        guard let currentDestination = workspaceContext.sshDestination else { return .direct }
        let resolvedRemoteSessionMode = remoteSessionMode ?? currentDestination.remoteSessionMode
        guard resolvedRemoteSessionMode == .tmux else { return .direct }
        let destination = remoteTmuxDestination ?? currentDestination
        return .remoteTmux(RemoteTmuxSession(id: resolvedSessionID, destination: destination))
    }
}
