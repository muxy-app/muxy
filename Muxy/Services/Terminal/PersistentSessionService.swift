import Foundation
import MuxySessionProtocol
import os

private let logger = Logger(subsystem: "app.muxy", category: "PersistentSession")

enum SessionCommandActivity: Equatable, Sendable {
    case idle
    case running
}

@MainActor
final class PersistentSessionService {
    static let shared = PersistentSessionService()

    static let defaultShellIntegrationFeatures = "cursor,title"

    private static let metadataVariables: [String: String] = [
        SessionMetadataKey.project: "MUXY_SESSION_PROJECT",
        SessionMetadataKey.worktree: "MUXY_SESSION_WORKTREE",
        SessionMetadataKey.tab: "MUXY_SESSION_TAB",
        SessionMetadataKey.title: "MUXY_SESSION_TITLE",
    ]

    private var resolvedSocketPath: String?
    private var descriptors: [UUID: SessionDescriptor] = [:]
    private var refreshingSessionIDs: Set<UUID> = []

    private init() {}

    var socketPath: String? {
        if let resolvedSocketPath {
            return resolvedSocketPath
        }
        do {
            let path = try PersistentSessionPaths.resolveSocketPath()
            resolvedSocketPath = path
            return path
        } catch {
            logger.error("unable to resolve the session socket path: \(String(describing: error))")
            return nil
        }
    }

    var existingSocketPath: String? {
        let candidates = [
            PersistentSessionPaths.preferredSocketPath(
                appSupportDirectory: MuxyFileStorage.appSupportDirectory(create: false)
            ),
            PersistentSessionPaths.fallbackSocketPath(userID: getuid()),
        ]
        return candidates.first { FileManager.default.fileExists(atPath: $0) }
    }

    var binaryPath: String? {
        PersistentSessionPaths.binaryURL()?.path
    }

    var isAvailable: Bool {
        socketPath != nil && binaryPath != nil
    }

    nonisolated static func sessionIdentifier(_ sessionID: UUID) -> SessionIdentifier? {
        SessionIdentifier(uuidString: sessionID.uuidString)
    }

    func attachCommand() -> String? {
        guard let binaryPath else { return nil }
        return ShellEscaper.escape(binaryPath) + " attach"
    }

    func launchEnvironment(
        sessionID: UUID,
        workingDirectory: String,
        command: String?,
        metadata: [(key: String, value: String)] = [],
        shell: String = UserShell.path(),
        resourcesDirectory: String? = GhosttyService.bundledResourcesPath()
    ) -> [(key: String, value: String)] {
        guard let socketPath, let binaryPath else { return [] }
        var entries: [(key: String, value: String)] = [
            (key: "MUXY_SESSION_ID", value: sessionID.uuidString),
            (key: "MUXY_SESSION_SOCKET", value: socketPath),
            (key: "MUXY_SESSION_BINARY", value: binaryPath),
            (key: "MUXY_SESSION_SHELL", value: shell),
            (key: "MUXY_SESSION_CWD", value: workingDirectory),
            (key: "MUXY_SESSION_COMMAND", value: command ?? ""),
            (key: "GHOSTTY_SHELL_FEATURES", value: shellIntegrationFeatures()),
        ]
        if let resourcesDirectory {
            entries.append((key: "MUXY_SESSION_RESOURCES", value: resourcesDirectory))
        }
        for entry in metadata {
            guard let variable = Self.metadataVariables[entry.key], !entry.value.isEmpty else { continue }
            entries.append((key: variable, value: entry.value))
        }
        return entries
    }

    private func shellIntegrationFeatures() -> String {
        let configured = MuxyConfig.shared.configValue(for: "shell-integration-features")
        guard let configured, !configured.isEmpty else { return Self.defaultShellIntegrationFeatures }
        return configured
    }

    func noteSurfaceMaterialized(sessionID: UUID) {
        refreshDescriptor(sessionID: sessionID)
    }

    func foregroundProcessID(sessionID: UUID) -> Int32? {
        guard let descriptor = descriptors[sessionID] else {
            refreshDescriptor(sessionID: sessionID)
            return nil
        }
        guard let processID = TerminalTTYForegroundProcess.processID(ttyDevice: descriptor.ttyDevice) else {
            descriptors.removeValue(forKey: sessionID)
            return nil
        }
        return processID
    }

    func isRunningCommand(sessionID: UUID) -> Bool {
        commandActivity(sessionID: sessionID) == .running
    }

    func commandActivity(sessionID: UUID) -> SessionCommandActivity? {
        guard let descriptor = descriptors[sessionID] else {
            refreshDescriptor(sessionID: sessionID)
            return nil
        }
        guard let foregroundProcessID = TerminalTTYForegroundProcess.processID(ttyDevice: descriptor.ttyDevice) else {
            descriptors.removeValue(forKey: sessionID)
            refreshDescriptor(sessionID: sessionID)
            return nil
        }
        return TerminalTTYForegroundProcess.isRunningCommand(
            foregroundProcessID: foregroundProcessID,
            shellProcessID: descriptor.shellProcessID
        ) ? .running : .idle
    }

    func lookup(sessionID: UUID) async -> PersistentSessionLookup {
        guard let socketPath = existingSocketPath, let identifier = Self.sessionIdentifier(sessionID) else {
            return .unreachable
        }
        let client = PersistentSessionControlClient(socketPath: socketPath)
        return await Task.detached(priority: .utility) {
            client.lookup(identifier: identifier)
        }.value
    }

    func endSession(sessionID: UUID) {
        descriptors.removeValue(forKey: sessionID)
        guard let socketPath = existingSocketPath, let identifier = Self.sessionIdentifier(sessionID) else { return }
        let client = PersistentSessionControlClient(socketPath: socketPath)
        Task.detached(priority: .utility) {
            client.kill(identifier: identifier)
        }
    }

    func endAllSessions() async {
        descriptors.removeAll()
        guard let socketPath = existingSocketPath else { return }
        let client = PersistentSessionControlClient(socketPath: socketPath)
        _ = await Task.detached(priority: .utility) {
            client.killAll()
        }.value
    }

    func liveSessions() async -> [SessionDescriptor] {
        guard let socketPath = existingSocketPath else { return [] }
        let client = PersistentSessionControlClient(socketPath: socketPath)
        return await Task.detached(priority: .utility) {
            client.list()
        }.value
    }

    private func refreshDescriptor(sessionID: UUID) {
        guard let socketPath = existingSocketPath,
              let identifier = Self.sessionIdentifier(sessionID),
              !refreshingSessionIDs.contains(sessionID)
        else { return }
        refreshingSessionIDs.insert(sessionID)
        let client = PersistentSessionControlClient(socketPath: socketPath)
        Task { [weak self] in
            let descriptor = await Task.detached(priority: .utility) {
                client.info(identifier: identifier)
            }.value
            guard let self else { return }
            refreshingSessionIDs.remove(sessionID)
            guard let descriptor else { return }
            descriptors[sessionID] = descriptor
        }
    }
}
