import Foundation

enum WorkspaceContext: Hashable {
    case local
    case ssh(SSHDestination)

    var isRemote: Bool {
        if case .ssh = self { return true }
        return false
    }

    var sshDestination: SSHDestination? {
        if case let .ssh(destination) = self { return destination }
        return nil
    }

    var cacheKeyPrefix: String {
        switch self {
        case .local:
            "local"
        case let .ssh(destination):
            "ssh:\(destination.target):\(destination.port ?? 22)"
        }
    }
}
