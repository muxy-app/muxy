import Foundation
import os
import Security

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionStore")

@MainActor
@Observable
final class ExtensionStore {
    static let shared = ExtensionStore()

    struct ExtensionStatus: Identifiable, Equatable {
        let id: String
        let muxyExtension: MuxyExtension
        var isEnabled: Bool
        var isRunning: Bool
        var lastError: String?

        var logFileURL: URL {
            ExtensionLogStore.shared.logURL(
                extensionID: id,
                directory: muxyExtension.directory
            )
        }
    }

    struct LoadFailure: Identifiable, Equatable {
        let id = UUID()
        let directory: URL
        let message: String
    }

    private(set) var statuses: [ExtensionStatus] = []
    private(set) var loadFailures: [LoadFailure] = []

    private var processes: [String: Process] = [:]
    private var tokens: [String: String] = [:]
    private var intentionalStops: Set<String> = []
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
        for index in statuses.indices where statuses[index].isEnabled {
            startExtension(at: index)
        }
        rebuildExtensionUICache()
        publishSnapshot()
    }

    func stopAll() {
        for status in statuses where status.isRunning {
            stopProcess(extensionID: status.id)
        }
        statusBarTextOverrides.removeAll()
        ExtensionIconAssetCache.shared.invalidateAll()
        for status in statuses {
            ExtensionPanelRegistry.shared.closeAll(extensionID: status.id)
        }
        rebuildExtensionUICache()
        publishSnapshot()
    }

    func reload() {
        stopAll()
        startAll()
    }

    func setEnabled(_ enabled: Bool, for extensionID: String) {
        guard let index = statuses.firstIndex(where: { $0.id == extensionID }) else { return }
        ExtensionEnabledStore.setEnabled(enabled, extensionID: extensionID)
        statuses[index].isEnabled = enabled

        if enabled, !statuses[index].isRunning {
            startExtension(at: index)
        } else if !enabled, statuses[index].isRunning {
            stopProcess(extensionID: extensionID)
        }
        if !enabled {
            statusBarTextOverrides.removeValue(forKey: extensionID)
            ExtensionIconAssetCache.shared.invalidate(extensionID: extensionID)
            ExtensionPanelRegistry.shared.closeAll(extensionID: extensionID)
        }
        rebuildExtensionUICache()
        publishSnapshot()
    }

    func extensionHasPermission(id: String, permission: ExtensionPermission) -> Bool {
        guard let muxyExtension = loadedExtension(id: id) else { return false }
        return muxyExtension.manifest.permissions.contains(permission)
    }

    func loadedExtension(id: String) -> MuxyExtension? {
        statuses.first(where: { $0.id == id && $0.isEnabled })?.muxyExtension
    }

    func snapshotForSocketServer() -> NotificationSocketServer.ExtensionSnapshot {
        var entries: [String: NotificationSocketServer.ExtensionSnapshotEntry] = [:]
        for status in statuses where status.isEnabled {
            let manifest = status.muxyExtension.manifest
            guard let token = tokens[status.id] else { continue }
            entries[status.id] = NotificationSocketServer.ExtensionSnapshotEntry(
                allowedEvents: Set(manifest.events),
                commandEvents: Set(manifest.commands.map(\.eventName)),
                permissions: Set(manifest.permissions),
                token: token
            )
        }
        return NotificationSocketServer.ExtensionSnapshot(entries: entries)
    }

    private func publishSnapshot() {
        NotificationSocketServer.shared.applyExtensionSnapshot(snapshotForSocketServer())
    }

    static func buildSnapshotForTesting(
        from entries: [(MuxyExtension, isEnabled: Bool)],
        token: String = "test-token"
    ) -> NotificationSocketServer.ExtensionSnapshot {
        var result: [String: NotificationSocketServer.ExtensionSnapshotEntry] = [:]
        for (ext, isEnabled) in entries where isEnabled {
            let manifest = ext.manifest
            result[ext.id] = NotificationSocketServer.ExtensionSnapshotEntry(
                allowedEvents: Set(manifest.events),
                commandEvents: Set(manifest.commands.map(\.eventName)),
                permissions: Set(manifest.permissions),
                token: token
            )
        }
        return NotificationSocketServer.ExtensionSnapshot(entries: result)
    }

    struct PaletteCommandBinding: Equatable {
        let muxyExtension: MuxyExtension
        let command: ExtensionPaletteCommand
    }

    struct TopbarItemBinding: Equatable, Identifiable {
        let muxyExtension: MuxyExtension
        let item: ExtensionTopbarItem

        var id: String { "\(muxyExtension.id):\(item.id)" }
    }

    struct StatusBarItemBinding: Equatable, Identifiable {
        let muxyExtension: MuxyExtension
        let item: ExtensionStatusBarItem
        let liveText: String?

        var id: String { "\(muxyExtension.id):\(item.id)" }
        var displayText: String? { liveText ?? item.text }
    }

    private var statusBarTextOverrides: [String: [String: String]] = [:]
    private(set) var topbarItems: [TopbarItemBinding] = []
    private(set) var leftStatusBarItems: [StatusBarItemBinding] = []
    private(set) var rightStatusBarItems: [StatusBarItemBinding] = []

    func paletteCommands() -> [PaletteCommandBinding] {
        statuses
            .filter(\.isEnabled)
            .flatMap { status in
                status.muxyExtension.manifest.commands.map { PaletteCommandBinding(muxyExtension: status.muxyExtension, command: $0) }
            }
    }

    func statusBarItems(side: ExtensionStatusBarItem.Side) -> [StatusBarItemBinding] {
        side == .left ? leftStatusBarItems : rightStatusBarItems
    }

    private func rebuildExtensionUICache() {
        var topbar: [TopbarItemBinding] = []
        var left: [StatusBarItemBinding] = []
        var right: [StatusBarItemBinding] = []
        for status in statuses where status.isEnabled {
            let ext = status.muxyExtension
            let overrides = statusBarTextOverrides[status.id]
            for item in ext.manifest.topbarItems {
                topbar.append(TopbarItemBinding(muxyExtension: ext, item: item))
            }
            for item in ext.manifest.statusBarItems {
                let binding = StatusBarItemBinding(
                    muxyExtension: ext,
                    item: item,
                    liveText: overrides?[item.id]
                )
                switch item.side {
                case .left: left.append(binding)
                case .right: right.append(binding)
                }
            }
        }
        topbarItems = topbar
        leftStatusBarItems = left
        rightStatusBarItems = right
    }

    func setStatusBarText(extensionID: String, itemID: String, text: String?) -> Bool {
        guard let muxyExtension = loadedExtension(id: extensionID),
              muxyExtension.manifest.statusBarItem(id: itemID) != nil
        else { return false }

        var overrides = statusBarTextOverrides[extensionID] ?? [:]
        if let text {
            overrides[itemID] = text
        } else {
            overrides.removeValue(forKey: itemID)
        }
        statusBarTextOverrides[extensionID] = overrides.isEmpty ? nil : overrides
        rebuildExtensionUICache()
        return true
    }

    struct CommandInvocation {
        let extensionID: String
        let commandID: String
        let appState: AppState
        let projectStore: ProjectStore?
        let worktreeStore: WorktreeStore?

        init(
            extensionID: String,
            commandID: String,
            appState: AppState,
            projectStore: ProjectStore? = nil,
            worktreeStore: WorktreeStore? = nil
        ) {
            self.extensionID = extensionID
            self.commandID = commandID
            self.appState = appState
            self.projectStore = projectStore
            self.worktreeStore = worktreeStore
        }
    }

    func triggerCommand(_ invocation: CommandInvocation) {
        guard let muxyExtension = statuses.first(where: { $0.id == invocation.extensionID })?.muxyExtension,
              let command = muxyExtension.manifest.commands.first(where: { $0.id == invocation.commandID })
        else { return }

        switch command.action {
        case .event:
            broadcastCommandEvent(
                extensionID: invocation.extensionID,
                commandID: invocation.commandID,
                name: command.eventName
            )
        case let .openTab(tabType, data):
            openExtensionTab(
                extensionID: invocation.extensionID,
                tabType: tabType,
                data: data,
                in: muxyExtension,
                appState: invocation.appState
            )
        case let .togglePanel(panelID):
            guard let panel = muxyExtension.manifest.panel(id: panelID) else { return }
            ExtensionPanelRegistry.shared.toggle(
                extensionID: invocation.extensionID,
                panel: panel,
                data: nil
            )
        case let .runScript(script):
            runExtensionScript(script: script, in: muxyExtension, invocation: invocation)
        }
    }

    private func runExtensionScript(
        script: String,
        in muxyExtension: MuxyExtension,
        invocation: CommandInvocation
    ) {
        guard extensionHasPermission(id: invocation.extensionID, permission: .commandsRunScript) else {
            ExtensionLogStore.shared.append(
                extensionID: invocation.extensionID,
                line: "[muxy] runScript blocked: missing commands:run-script permission"
            )
            return
        }
        guard let scriptURL = muxyExtension.resolveResource(script) else {
            ExtensionLogStore.shared.append(
                extensionID: invocation.extensionID,
                line: "[muxy] runScript blocked: script path escapes extension directory"
            )
            return
        }
        Task { @MainActor in
            do {
                try await ExtensionScriptRunner.shared.runScript(
                    extensionID: invocation.extensionID,
                    scriptURL: scriptURL,
                    appState: invocation.appState,
                    projectStore: invocation.projectStore,
                    worktreeStore: invocation.worktreeStore
                )
            } catch {
                ExtensionLogStore.shared.append(
                    extensionID: invocation.extensionID,
                    line: "[muxy] runScript failed: \(error.localizedDescription)"
                )
            }
        }
    }

    private func broadcastCommandEvent(extensionID: String, commandID: String, name: String) {
        NotificationSocketServer.shared.broadcast(
            event: ExtensionEvent(
                name: name,
                payload: ["extension": extensionID, "command": commandID]
            )
        )
    }

    private func openExtensionTab(
        extensionID: String,
        tabType tabTypeID: String,
        data: ExtensionJSON?,
        in muxyExtension: MuxyExtension,
        appState: AppState
    ) {
        guard let tabType = muxyExtension.manifest.tabType(id: tabTypeID),
              let projectID = appState.activeProjectID
        else { return }
        appState.dispatch(.createExtensionTab(
            projectID: projectID,
            areaID: nil,
            request: AppState.CreateExtensionTabRequest(
                extensionID: extensionID,
                tabTypeID: tabTypeID,
                title: tabType.title,
                data: data ?? tabType.defaultData
            )
        ))
    }

    func declaredAIProvider(for socketTypeKey: String) -> (extensionID: String, provider: ExtensionAIProvider)? {
        for status in statuses where status.isEnabled {
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
                ExtensionLogStore.shared.register(extensionID: ext.id, directory: ext.directory)
                statuses.append(ExtensionStatus(
                    id: ext.id,
                    muxyExtension: ext,
                    isEnabled: ExtensionEnabledStore.isEnabled(extensionID: ext.id),
                    isRunning: false,
                    lastError: nil
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

        guard let entrypointURL = ext.entrypointURL else { return }

        let process = Process()
        process.executableURL = entrypointURL
        process.currentDirectoryURL = ext.directory

        let token = Self.generateToken()
        tokens[ext.id] = token

        var environment = ProcessInfo.processInfo.environment
        environment["MUXY_SOCKET_PATH"] = NotificationSocketServer.socketPath
        environment["MUXY_EXTENSION_ID"] = ext.id
        environment["MUXY_EXTENSION_TOKEN"] = token
        let logURL = ExtensionLogStore.shared.logURL(extensionID: ext.id, directory: ext.directory)
        environment["MUXY_EXTENSION_LOG"] = logURL.path
        process.environment = environment

        if let logHandle = openProcessLogHandle(at: logURL) {
            process.standardOutput = logHandle
            process.standardError = logHandle
        }

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
            ExtensionLogStore.shared.append(
                extensionID: ext.id,
                line: "[muxy] started \(ext.id) v\(ext.manifest.version)"
            )
        } catch {
            statuses[index].lastError = error.localizedDescription
            ExtensionLogStore.shared.append(
                extensionID: ext.id,
                line: "[muxy] failed to start: \(error.localizedDescription)"
            )
            logger.error("Failed to start extension \(ext.id): \(error.localizedDescription)")
        }
    }

    private func openProcessLogHandle(at url: URL) -> FileHandle? {
        let directory = url.deletingLastPathComponent()
        if !FileManager.default.fileExists(atPath: directory.path) {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        if !FileManager.default.fileExists(atPath: url.path) {
            FileManager.default.createFile(atPath: url.path, contents: nil)
        }
        guard let handle = try? FileHandle(forWritingTo: url) else { return nil }
        try? handle.seekToEnd()
        return handle
    }

    private func stopProcess(extensionID: String) {
        ExtensionScriptRunner.shared.evict(extensionID: extensionID)
        tokens.removeValue(forKey: extensionID)
        guard let process = processes.removeValue(forKey: extensionID) else { return }
        if process.isRunning {
            intentionalStops.insert(extensionID)
            process.terminate()
        }
        if let index = statuses.firstIndex(where: { $0.id == extensionID }) {
            statuses[index].isRunning = false
        }
    }

    private static func generateToken() -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        let result = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard result == errSecSuccess else {
            return UUID().uuidString + UUID().uuidString
        }
        return Data(bytes).base64EncodedString()
    }

    private func handleTermination(extensionID: String, process: Process) {
        guard processes[extensionID] === process else { return }
        processes.removeValue(forKey: extensionID)
        let wasIntentional = intentionalStops.remove(extensionID) != nil
        guard let index = statuses.firstIndex(where: { $0.id == extensionID }) else { return }
        statuses[index].isRunning = false
        let outcome = Self.classifyTermination(
            wasIntentional: wasIntentional,
            terminationStatus: process.terminationStatus
        )
        switch outcome {
        case .stopped:
            ExtensionLogStore.shared.append(extensionID: extensionID, line: "[muxy] stopped")
        case .exitedCleanly:
            ExtensionLogStore.shared.append(extensionID: extensionID, line: "[muxy] exited cleanly")
        case let .exitedWithStatus(status):
            let message = "Process exited with status \(status)"
            statuses[index].lastError = message
            ExtensionLogStore.shared.append(extensionID: extensionID, line: "[muxy] \(message)")
        }
    }

    enum TerminationOutcome: Equatable {
        case stopped
        case exitedCleanly
        case exitedWithStatus(Int32)
    }

    nonisolated static func classifyTermination(wasIntentional: Bool, terminationStatus: Int32) -> TerminationOutcome {
        if wasIntentional { return .stopped }
        if terminationStatus == 0 { return .exitedCleanly }
        return .exitedWithStatus(terminationStatus)
    }
}
