import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionStore command permissions")
@MainActor
struct ExtensionStoreCommandPermissionTests {
    @Test("openTab commands require tabs write permission")
    func openTabRequiresTabsWritePermission() throws {
        let store = try makeStore(
            name: "open-tab-denied-\(UUID().uuidString)",
            permissions: [],
            action: #"{"kind":"openTab","tabType":"logs"}"#,
            extraManifest: #""tabTypes":[{"id":"logs","title":"Logs","entry":"tabs/logs.html"}],"#,
            files: ["tabs/logs.html": "<html></html>"]
        )
        let appState = makeAppState()
        let projectID = try #require(appState.activeProjectID)
        let before = try #require(appState.focusedArea(for: projectID)).tabs.count
        let extensionID = try #require(store.statuses.first?.id)

        store.triggerCommand(.init(extensionID: extensionID, commandID: "run", appState: appState))

        #expect(appState.focusedArea(for: projectID)?.tabs.count == before)
    }

    @Test("openTab commands run with tabs write permission")
    func openTabRunsWithTabsWritePermission() throws {
        let store = try makeStore(
            name: "open-tab-allowed-\(UUID().uuidString)",
            permissions: ["tabs:write"],
            action: #"{"kind":"openTab","tabType":"logs"}"#,
            extraManifest: #""tabTypes":[{"id":"logs","title":"Logs","entry":"tabs/logs.html"}],"#,
            files: ["tabs/logs.html": "<html></html>"]
        )
        let appState = makeAppState()
        let projectID = try #require(appState.activeProjectID)
        let before = try #require(appState.focusedArea(for: projectID)).tabs.count
        let extensionID = try #require(store.statuses.first?.id)

        store.triggerCommand(.init(extensionID: extensionID, commandID: "run", appState: appState))

        #expect(appState.focusedArea(for: projectID)?.tabs.count == before + 1)
    }

    @Test("popover commands require panels write permission")
    func popoverRequiresPanelsWritePermission() throws {
        let store = try makeStore(
            name: "popover-denied-\(UUID().uuidString)",
            permissions: [],
            action: #"{"kind":"openPopover","popover":"summary"}"#,
            extraManifest: #""popovers":[{"id":"summary","entry":"popovers/summary.html"}],"#,
            files: ["popovers/summary.html": "<html></html>"]
        )
        let muxyExtension = try #require(store.statuses.first?.muxyExtension)

        #expect(store.popover(for: muxyExtension, command: "run") == nil)
    }

    @Test("togglePanel commands require panels write permission")
    func togglePanelRequiresPanelsWritePermission() throws {
        let store = try makeStore(
            name: "panel-denied-\(UUID().uuidString)",
            permissions: [],
            action: #"{"kind":"togglePanel","panel":"summary"}"#,
            extraManifest: #""panels":[{"id":"summary","entry":"panels/summary.html"}],"#,
            files: ["panels/summary.html": "<html></html>"]
        )
        let appState = makeAppState()
        let extensionID = try #require(store.statuses.first?.id)

        store.triggerCommand(.init(extensionID: extensionID, commandID: "run", appState: appState))

        let hostPanelID = ExtensionPanelState.hostPanelID(extensionID: extensionID, panelID: "summary")
        #expect(ExtensionPanelRegistry.shared.state(forHostPanelID: hostPanelID) == nil)
    }

    private func makeStore(
        name: String,
        permissions: [String],
        action: String,
        extraManifest: String,
        files: [String: String]
    ) throws -> ExtensionStore {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("exts-\(UUID().uuidString)")
        let directory = root.appendingPathComponent(name)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let permissionJSON = permissions.map { "\"\($0)\"" }.joined(separator: ",")
        let manifest = """
        {
            "name": "\(name)",
            "version": "1.0.0",
            "permissions": [\(permissionJSON)],
            \(extraManifest)
            "commands": [{"id":"run","title":"Run","action":\(action)}]
        }
        """
        try ExtensionManifestFixture.write(flatManifest: manifest, to: directory)
        for (path, contents) in files {
            let url = directory.appendingPathComponent(path)
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try Data(contents.utf8).write(to: url)
        }
        ExtensionEnabledStore.setEnabled(true, extensionID: name)
        let store = ExtensionStore.makeForTesting(
            rootDirectory: root,
            snapshotSink: NoopExtensionSnapshotSink(),
            resolveHostURL: { nil }
        )
        store.startAll()
        return store
    }

    private func makeAppState() -> AppState {
        let appState = AppState(
            selectionStore: CommandPermissionSelectionStore(),
            terminalViews: CommandPermissionTerminalViews(),
            workspacePersistence: CommandPermissionWorkspacePersistence()
        )
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/test")
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return appState
    }
}

private final class NoopExtensionSnapshotSink: ExtensionSnapshotSink {
    nonisolated func applyExtensionSnapshot(_: NotificationSocketServer.ExtensionSnapshot) {}
}

private final class CommandPermissionWorkspacePersistence: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class CommandPermissionSelectionStore: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class CommandPermissionTerminalViews: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
