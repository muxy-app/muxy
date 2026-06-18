import Foundation

enum SSHImplementationResolver {
    static func mode(
        for workspaceContext: WorkspaceContext,
        sshConfiguration: SSHConnectionConfiguration?
    ) -> SSHImplementationMode? {
        if case .ssh = workspaceContext {
            return SSHImplementationMode.current
        }
        if sshConfiguration != nil {
            return .native
        }
        return nil
    }
}
