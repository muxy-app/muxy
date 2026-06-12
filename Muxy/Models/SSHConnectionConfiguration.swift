import Foundation
import MuxySSH

struct SSHConnectionConfiguration: Equatable, SSHConnectionConfigurable {
    let host: String
    let port: Int
    let user: String
    let remotePath: String
    let authentication: SSHAuthentication?
    let command: String?

    var remoteExecCommand: String? {
        guard let command else { return nil }
        return "\(pathCommand); \(command)"
    }

    var initialShellInput: String {
        guard command == nil else { return "" }
        return "\(pathCommand)\n"
    }

    var localSurfaceWorkingDirectory: String {
        NSHomeDirectory()
    }

    private var pathCommand: String {
        "cd \(RemoteCommandBuilder.quoteRemotePath(remotePath))"
    }
    static func make(
        destination: SSHDestination,
        remotePath: String? = nil,
        command: String? = nil
    ) -> SSHConnectionConfiguration {
        let resolved = ResolvedSSHDestination.resolve(destination)
        return SSHConnectionConfiguration(
            host: resolved.host,
            port: resolved.port,
            user: resolved.user,
            remotePath: remotePath ?? destination.remoteRoot,
            authentication: authentication(for: destination, resolved: resolved),
            command: command
        )
    }

    private static func authentication(
        for destination: SSHDestination,
        resolved: ResolvedSSHDestination
    ) -> SSHAuthentication? {
        switch destination.authenticationMethod {
        case .automatic:
            if let identityFile = resolved.identityFile {
                return .privateKey(path: identityFile)
            }
            return nil
        case .privateKey:
            guard let identityFile = resolved.identityFile else { return nil }
            return .privateKey(path: identityFile)
        case .password:
            guard let password = KeychainSSHHelper.getPassword(
                host: destination.host,
                user: resolved.user,
                port: UInt16(max(0, min(resolved.port, Int(UInt16.max))))
            ), !password.isEmpty else {
                return nil
            }
            return .password(password)
        }
    }
}

private struct ResolvedSSHDestination {
    let host: String
    let port: Int
    let user: String
    let identityFile: String?

    static func resolve(_ destination: SSHDestination) -> ResolvedSSHDestination {
        let parsedHost = SSHConfigParser.parse().first { $0.name == destination.host }
        let resolvedHost = parsedHost?.hostName ?? destination.host
        let resolvedPort = destination.port ?? parsedHost.map { Int($0.port) } ?? 22
        let resolvedUser = destination.user ?? parsedHost?.user ?? NSUserName()
        let resolvedIdentityFile = destination.identityFile ?? parsedHost?.identityFile
        return ResolvedSSHDestination(
            host: resolvedHost,
            port: resolvedPort,
            user: resolvedUser,
            identityFile: resolvedIdentityFile
        )
    }
}
