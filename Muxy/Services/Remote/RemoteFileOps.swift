import Foundation

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
                """
                if [ -e "$1" ] || [ -L "$1" ]; then exit 0; fi
                case "$1" in
                  */*) parent=${1%/*}; [ -n "$parent" ] || parent=/ ;;
                  *) parent=. ;;
                esac
                [ ! -e "$parent" ] && exit 1
                [ -x "$parent" ] && exit 1
                exit 2
                """,
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
        let quoted = RemoteCommandBuilder.quoteRemotePath(path)
        let quotedParent = RemoteCommandBuilder.quoteRemotePath(parentPath(of: path))
        let result = try? await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: presenceCommand(path: quoted, parent: quotedParent)
        )
        return result?.status != 1
    }

    func exists(at path: String, timeout: TimeInterval) async throws -> Bool {
        let quoted = RemoteCommandBuilder.quoteRemotePath(path)
        let quotedParent = RemoteCommandBuilder.quoteRemotePath(parentPath(of: path))
        let result = try await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: presenceCommand(path: quoted, parent: quotedParent),
            timeout: timeout
        )
        return result.status != 1
    }

    private func parentPath(of path: String) -> String {
        let parent = NSString(string: path).deletingLastPathComponent
        return parent.isEmpty ? "." : parent
    }

    private func presenceCommand(path: String, parent: String) -> String {
        "if [ -e \(path) ] || [ -L \(path) ]; then exit 0; fi; [ ! -e \(parent) ] && exit 1; [ -x \(parent) ] && exit 1; exit 2"
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
