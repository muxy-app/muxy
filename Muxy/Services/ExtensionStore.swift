import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionStore")

@MainActor
@Observable
final class ExtensionStore {
    static let shared = ExtensionStore()

    struct ExtensionStatus: Identifiable, Equatable {
        let id: String
        let muxyExtension: MuxyExtension
        var isRunning: Bool
        var lastError: String?
        var logs: [String]
    }

    struct LoadFailure: Identifiable, Equatable {
        let id = UUID()
        let directory: URL
        let message: String
    }

    private(set) var statuses: [ExtensionStatus] = []
    private(set) var loadFailures: [LoadFailure] = []

    private var processes: [String: Process] = [:]
    private let maxLogLines = 200
    private let rootDirectoryURL: URL

    private init(rootDirectory: URL = ExtensionStore.defaultRootDirectory) {
        rootDirectoryURL = rootDirectory
    }

    static var defaultRootDirectory: URL {
        let home = FileManager.default.homeDirectoryForCurrentUser
        return home.appendingPathComponent(".config/muxy/extensions", isDirectory: true)
    }

    var rootDirectory: URL { rootDirectoryURL }

    func startAll() {
        loadFromDisk()
        for index in statuses.indices where statuses[index].muxyExtension.manifest.enabled {
            startExtension(at: index)
        }
        publishSnapshot()
    }

    func stopAll() {
        for status in statuses where status.isRunning {
            stopProcess(extensionID: status.id)
        }
        publishSnapshot()
    }

    func reload() {
        stopAll()
        startAll()
    }

    func setEnabled(_ enabled: Bool, for extensionID: String) {
        guard let index = statuses.firstIndex(where: { $0.id == extensionID }) else { return }
        let original = statuses[index].muxyExtension.manifest

        let updatedManifest = ExtensionManifest(
            name: original.name,
            version: original.version,
            description: original.description,
            entrypoint: original.entrypoint,
            events: original.events,
            commands: original.commands,
            permissions: original.permissions,
            aiProvider: original.aiProvider,
            enabled: enabled
        )

        let updatedExtension = MuxyExtension(
            id: statuses[index].muxyExtension.id,
            directory: statuses[index].muxyExtension.directory,
            manifest: updatedManifest
        )

        statuses[index] = ExtensionStatus(
            id: updatedExtension.id,
            muxyExtension: updatedExtension,
            isRunning: statuses[index].isRunning,
            lastError: statuses[index].lastError,
            logs: statuses[index].logs
        )

        if enabled, !statuses[index].isRunning {
            startExtension(at: index)
        } else if !enabled, statuses[index].isRunning {
            stopProcess(extensionID: extensionID)
        }
        publishSnapshot()
    }

    func extensionHasPermission(id: String, permission: ExtensionPermission) -> Bool {
        guard let status = statuses.first(where: { $0.id == id }) else { return false }
        return status.muxyExtension.manifest.permissions.contains(permission)
    }

    func snapshotForSocketServer() -> NotificationSocketServer.ExtensionSnapshot {
        var entries: [String: NotificationSocketServer.ExtensionSnapshotEntry] = [:]
        for status in statuses where status.muxyExtension.manifest.enabled {
            let manifest = status.muxyExtension.manifest
            entries[status.id] = NotificationSocketServer.ExtensionSnapshotEntry(
                allowedEvents: Set(manifest.events),
                commandEvents: Set(manifest.commands.map(\.eventName)),
                permissions: Set(manifest.permissions)
            )
        }
        return NotificationSocketServer.ExtensionSnapshot(entries: entries)
    }

    private func publishSnapshot() {
        NotificationSocketServer.shared.applyExtensionSnapshot(snapshotForSocketServer())
    }

    static func buildSnapshotForTesting(from extensions: [MuxyExtension]) -> NotificationSocketServer.ExtensionSnapshot {
        var entries: [String: NotificationSocketServer.ExtensionSnapshotEntry] = [:]
        for ext in extensions where ext.manifest.enabled {
            let manifest = ext.manifest
            entries[ext.id] = NotificationSocketServer.ExtensionSnapshotEntry(
                allowedEvents: Set(manifest.events),
                commandEvents: Set(manifest.commands.map(\.eventName)),
                permissions: Set(manifest.permissions)
            )
        }
        return NotificationSocketServer.ExtensionSnapshot(entries: entries)
    }

    struct PaletteCommandBinding: Equatable {
        let muxyExtension: MuxyExtension
        let command: ExtensionPaletteCommand
    }

    func paletteCommands() -> [PaletteCommandBinding] {
        statuses
            .filter(\.muxyExtension.manifest.enabled)
            .flatMap { status in
                status.muxyExtension.manifest.commands.map { PaletteCommandBinding(muxyExtension: status.muxyExtension, command: $0) }
            }
    }

    func triggerCommand(extensionID: String, commandID: String) {
        guard let command = statuses.first(where: { $0.id == extensionID })?
            .muxyExtension.manifest.commands.first(where: { $0.id == commandID })
        else { return }

        NotificationSocketServer.shared.broadcast(
            event: ExtensionEvent(
                name: command.eventName,
                payload: ["extension": extensionID, "command": commandID]
            )
        )
    }

    func declaredAIProvider(for socketTypeKey: String) -> (extensionID: String, provider: ExtensionAIProvider)? {
        for status in statuses where status.muxyExtension.manifest.enabled {
            if let provider = status.muxyExtension.manifest.aiProvider,
               provider.socketTypeKey == socketTypeKey
            {
                return (status.id, provider)
            }
        }
        return nil
    }

    private func loadFromDisk() {
        statuses = []
        loadFailures = []

        try? FileManager.default.createDirectory(
            at: rootDirectoryURL,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )

        guard let contents = try? FileManager.default.contentsOfDirectory(
            at: rootDirectoryURL,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        )
        else { return }

        var seenIDs = Set<String>()
        for url in contents.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
            var isDirectory: ObjCBool = false
            guard FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory),
                  isDirectory.boolValue
            else { continue }

            do {
                let ext = try ExtensionManifestLoader.load(from: url)
                guard !seenIDs.contains(ext.id) else {
                    loadFailures.append(LoadFailure(
                        directory: url,
                        message: ExtensionLoadError.duplicateName(ext.id).localizedDescription
                    ))
                    continue
                }
                seenIDs.insert(ext.id)
                statuses.append(ExtensionStatus(
                    id: ext.id,
                    muxyExtension: ext,
                    isRunning: false,
                    lastError: nil,
                    logs: []
                ))
            } catch {
                loadFailures.append(LoadFailure(
                    directory: url,
                    message: error.localizedDescription
                ))
                logger.error("Failed to load extension at \(url.path): \(error.localizedDescription)")
            }
        }
    }

    private func startExtension(at index: Int) {
        let status = statuses[index]
        let ext = status.muxyExtension

        let process = Process()
        process.executableURL = ext.entrypointURL
        process.currentDirectoryURL = ext.directory

        var environment = ProcessInfo.processInfo.environment
        environment["MUXY_SOCKET_PATH"] = NotificationSocketServer.socketPath
        environment["MUXY_EXTENSION_ID"] = ext.id
        process.environment = environment

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        attachLogReader(pipe: stdoutPipe, extensionID: ext.id, label: "out")
        attachLogReader(pipe: stderrPipe, extensionID: ext.id, label: "err")

        process.terminationHandler = { [weak self] terminatedProcess in
            Task { @MainActor [weak self] in
                self?.handleTermination(extensionID: ext.id, process: terminatedProcess)
            }
        }

        do {
            try process.run()
            processes[ext.id] = process
            statuses[index].isRunning = true
            statuses[index].lastError = nil
            appendLog(extensionID: ext.id, line: "[muxy] started \(ext.id) v\(ext.manifest.version)")
        } catch {
            statuses[index].lastError = error.localizedDescription
            appendLog(extensionID: ext.id, line: "[muxy] failed to start: \(error.localizedDescription)")
            logger.error("Failed to start extension \(ext.id): \(error.localizedDescription)")
        }
    }

    private func attachLogReader(pipe: Pipe, extensionID: String, label: String) {
        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty, let text = String(data: data, encoding: .utf8) else { return }
            let lines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
            Task { @MainActor [weak self] in
                for line in lines {
                    self?.appendLog(extensionID: extensionID, line: "[\(label)] \(line)")
                }
            }
        }
    }

    private func stopProcess(extensionID: String) {
        guard let process = processes.removeValue(forKey: extensionID) else { return }
        if process.isRunning {
            process.terminate()
        }
        if let index = statuses.firstIndex(where: { $0.id == extensionID }) {
            statuses[index].isRunning = false
        }
    }

    private func handleTermination(extensionID: String, process: Process) {
        processes.removeValue(forKey: extensionID)
        guard let index = statuses.firstIndex(where: { $0.id == extensionID }) else { return }
        statuses[index].isRunning = false
        let status = process.terminationStatus
        if status != 0 {
            let message = "Process exited with status \(status)"
            statuses[index].lastError = message
            appendLog(extensionID: extensionID, line: "[muxy] \(message)")
        } else {
            appendLog(extensionID: extensionID, line: "[muxy] exited cleanly")
        }
    }

    private func appendLog(extensionID: String, line: String) {
        guard let index = statuses.firstIndex(where: { $0.id == extensionID }) else { return }
        var logs = statuses[index].logs
        logs.append(line)
        if logs.count > maxLogLines {
            logs.removeFirst(logs.count - maxLogLines)
        }
        statuses[index].logs = logs
    }
}
