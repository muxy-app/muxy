import Foundation

enum NativeSSHAuthentication: Equatable {
    case privateKey(path: String)
    case password(String)
}

struct NativeSSHConnectionConfiguration: Equatable {
    let hostID: UUID
    let name: String
    let host: String
    let port: Int
    let user: String
    let remotePath: String
    let authentication: NativeSSHAuthentication?
    let command: String?

    var remoteExecCommand: String? {
        guard let command else { return nil }
        return "\(pathCommand); \(command)"
    }

    var initialShellInput: String {
        "\(pathCommand)\n"
    }

    private var pathCommand: String {
        "cd \(ShellEscaper.escape(remotePath))"
    }

    static func make(
        host: RemoteHost,
        remoteConfig: RemoteProjectConfig,
        command: String? = nil
    ) -> NativeSSHConnectionConfiguration {
        NativeSSHConnectionConfiguration(
            hostID: host.id,
            name: host.name,
            host: host.host,
            port: Int(host.port),
            user: host.user,
            remotePath: remoteConfig.remotePath,
            authentication: authentication(for: host),
            command: command
        )
    }

    static func authentication(for host: RemoteHost) -> NativeSSHAuthentication? {
        if let identityFile = host.identityFile?.trimmingCharacters(in: .whitespacesAndNewlines),
           !identityFile.isEmpty
        {
            return .privateKey(path: identityFile)
        }
        if host.useKeychain,
           let password = KeychainSSHHelper.getPassword(host: host.host, user: host.user),
           !password.isEmpty
        {
            return .password(password)
        }
        return nil
    }
}
