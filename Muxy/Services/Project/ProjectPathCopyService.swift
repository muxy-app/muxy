import Foundation
import os

private let projectPathCopyLogger = Logger(subsystem: "app.muxy", category: "ProjectPathCopyService")

enum ProjectPathResolutionError: LocalizedError {
    case emptyPath
    case remoteCommandFailed(String)
    case invalidRemotePath

    var errorDescription: String? {
        switch self {
        case .emptyPath:
            "The project path is empty."
        case let .remoteCommandFailed(message):
            message.isEmpty ? "The remote project path could not be resolved." : message
        case .invalidRemotePath:
            "The remote project returned an invalid absolute path."
        }
    }
}

struct ProjectPathResolver {
    typealias RemoteCommandRunner = @Sendable (SSHDestination, String) async throws -> GitProcessResult

    private let runRemoteCommand: RemoteCommandRunner

    init(
        runRemoteCommand: @escaping RemoteCommandRunner = { destination, command in
            try await SSHCommandRunner.run(destination: destination, remoteCommand: command)
        }
    ) {
        self.runRemoteCommand = runRemoteCommand
    }

    func absolutePath(for path: String, in workspaceContext: WorkspaceContext) async throws -> String {
        guard !path.isEmpty else { throw ProjectPathResolutionError.emptyPath }

        switch workspaceContext {
        case .local:
            let expandedPath = (path as NSString).expandingTildeInPath
            return URL(fileURLWithPath: expandedPath).standardizedFileURL.path
        case let .ssh(destination):
            if path.hasPrefix("/") {
                return (path as NSString).standardizingPath
            }

            let command = "cd \(RemoteCommandBuilder.quoteRemotePath(path)) && pwd -P"
            let result = try await runRemoteCommand(destination, command)
            guard result.status == 0 else {
                let message = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
                throw ProjectPathResolutionError.remoteCommandFailed(message)
            }

            let output = result.stdout.trimmingCharacters(in: .newlines)
            guard output.hasPrefix("/"), !output.contains(where: \.isNewline) else {
                throw ProjectPathResolutionError.invalidRemotePath
            }
            return (output as NSString).standardizingPath
        }
    }
}

enum ProjectPathCopyService {
    @MainActor
    static func copy(
        path: String,
        workspaceContext: WorkspaceContext,
        resolver: ProjectPathResolver = ProjectPathResolver()
    ) async {
        do {
            let absolutePath = try await resolver.absolutePath(for: path, in: workspaceContext)
            PathClipboard.copy(absolutePath)
        } catch {
            projectPathCopyLogger.error("Failed to copy project path: \(error.localizedDescription)")
            ToastState.shared.show(title: "Could not copy project path", body: error.localizedDescription)
        }
    }
}
