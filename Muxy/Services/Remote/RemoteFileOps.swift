import Foundation

enum FilePresenceCommand {
    static let script = """
    if [ -e "$1" ] || [ -L "$1" ]; then exit 0; fi
    candidate="$1"
    while :; do
      case "$candidate" in
        */*) candidate=${candidate%/*}; [ -n "$candidate" ] || candidate=/ ;;
        *) candidate=. ;;
      esac
      if [ -L "$candidate" ] && [ ! -e "$candidate" ]; then exit 2; fi
      if [ -e "$candidate" ]; then
        [ ! -d "$candidate" ] && exit 1
        [ -x "$candidate" ] && exit 1
        exit 2
      fi
      [ "$candidate" = / ] || [ "$candidate" = . ] || continue
      exit 2
    done
    """

    static func remote(path: String) -> String {
        "set -- \(RemoteCommandBuilder.quoteRemotePath(path)); \(script)"
    }
}

protocol RemoteFileOps: Sendable {
    func makeDirectory(at path: String) async throws
    func removeItem(at path: String) async throws
    func exists(at path: String) async -> Bool
    func exists(at path: String, timeout: TimeInterval) async throws -> Bool
}

struct LocalFileOps: RemoteFileOps {
    func makeDirectory(at path: String) async throws {
        try await GitProcessRunner.offMainThrowing {
            try FileManager.default.createDirectory(
                atPath: path,
                withIntermediateDirectories: true,
                attributes: nil
            )
        }
    }

    func removeItem(at path: String) async throws {
        try await GitProcessRunner.offMainThrowing {
            try FileManager.default.removeItem(atPath: path)
        }
    }

    func exists(at path: String) async -> Bool {
        await GitProcessRunner.offMain {
            FileManager.default.fileExists(atPath: path) ||
                (try? FileManager.default.destinationOfSymbolicLink(atPath: path)) != nil
        }
    }

    func exists(at path: String, timeout: TimeInterval) async throws -> Bool {
        let result = try await SubprocessRunner.run(SubprocessRequest(
            executablePath: "/bin/sh",
            arguments: [
                "-c",
                FilePresenceCommand.script,
                "muxy-path-exists",
                path,
            ],
            timeout: timeout
        ))
        return result.status != 1
    }
}

struct SSHFileOps: RemoteFileOps {
    let destination: SSHDestination

    func makeDirectory(at path: String) async throws {
        try await run("mkdir -p \(RemoteCommandBuilder.quoteRemotePath(path))")
    }

    func removeItem(at path: String) async throws {
        try await run("rm -rf \(RemoteCommandBuilder.quoteRemotePath(path))")
    }

    func exists(at path: String) async -> Bool {
        let result = try? await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: FilePresenceCommand.remote(path: path)
        )
        return result?.status != 1
    }

    func exists(at path: String, timeout: TimeInterval) async throws -> Bool {
        let result = try await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: FilePresenceCommand.remote(path: path),
            timeout: timeout
        )
        return result.status != 1
    }

    private func run(_ remoteCommand: String) async throws {
        let result = try await SSHCommandRunner.run(destination: destination, remoteCommand: remoteCommand)
        guard result.status == 0 else {
            throw RemoteFileOpsError.commandFailed(result.stderr.isEmpty ? remoteCommand : result.stderr)
        }
    }
}

enum RemoteFileOpsError: LocalizedError {
    case commandFailed(String)

    var errorDescription: String? {
        switch self {
        case let .commandFailed(message): message
        }
    }
}

extension WorkspaceContext {
    var fileOps: any RemoteFileOps {
        switch self {
        case .local: LocalFileOps()
        case let .ssh(destination): SSHFileOps(destination: destination)
        }
    }
}
