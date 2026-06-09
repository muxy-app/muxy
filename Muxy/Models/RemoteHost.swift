import Foundation

struct RemoteHost: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var host: String
    var port: UInt16
    var user: String
    var identityFile: String?
    var useKeychain: Bool
    var additionalArgs: [String]
    var createdAt: Date
    var updatedAt: Date

    init(
        id: UUID = UUID(),
        name: String,
        host: String,
        port: UInt16 = 22,
        user: String,
        identityFile: String? = nil,
        useKeychain: Bool = false,
        additionalArgs: [String] = []
    ) {
        self.id = id
        self.name = name
        self.host = host
        self.port = port
        self.user = user
        self.identityFile = identityFile
        self.useKeychain = useKeychain
        self.additionalArgs = additionalArgs
        self.createdAt = Date()
        self.updatedAt = Date()
    }

    var displaySummary: String {
        "\(user)@\(host):\(port)"
    }
}

extension RemoteHost {
    static func controlPathBase() -> String {
        let cacheDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".cache/muxy/ssh-control")
        return cacheDir.path
    }

    func controlPath() -> String {
        let escaped = "\(user)@\(host):\(port)"
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: ":", with: "_")
        return "\(Self.controlPathBase())/\(escaped)"
    }

    func sshCommandArgs(remotePath: String?) -> [String] {
        var args = ["ssh"]

        args.append(contentsOf: [
            "-o", "ControlMaster=auto",
            "-o", "ControlPath=\(controlPath())",
            "-o", "ControlPersist=10m",
            "-p", "\(port)",
        ])

        if let identityFile {
            args.append(contentsOf: ["-i", identityFile])
        }

        if useKeychain {
            args.append(contentsOf: ["-o", "PreferredAuthentications=keyboard-interactive,password,publickey"])
        }

        args.append(contentsOf: additionalArgs)

        args.append("\(user)@\(host)")

        if let remotePath {
            args.append("-t")
            args.append("cd \(ShellEscaper.escape(remotePath)); exec $SHELL -l")
        }

        return args
    }

    func sshCommandString(remotePath: String?) -> String {
        sshCommandArgs(remotePath: remotePath).map { arg in
            if arg.contains(" ") || arg.contains(";") {
                return "'\(arg.replacingOccurrences(of: "'", with: "'\\''"))'"
            }
            return arg
        }.joined(separator: " ")
    }
}
