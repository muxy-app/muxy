import Foundation

enum TerminalProcessExitPolicy {
    enum Disposition: Equatable {
        case closePane
        case recoverRemoteConnection
        case recoverPersistentSession(UUID)
    }

    static func disposition(isRemote: Bool, persistentSessionID: UUID?) -> Disposition {
        if isRemote {
            return .recoverRemoteConnection
        }
        if let persistentSessionID {
            return .recoverPersistentSession(persistentSessionID)
        }
        return .closePane
    }
}
