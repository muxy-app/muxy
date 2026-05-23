import Foundation
import Testing

@testable import Muxy

@Suite("BrowserDevServerAutoOpener")
@MainActor
struct BrowserDevServerAutoOpenerTests {
    @Test("posts a worktree-scoped tab when the originating pane is known and the preference is on")
    func opensTabInPaneOwningWorktreeWhenEnabled() {
        let harness = makeHarness()
        let previousPreference = swap(BrowserPreferences.autoOpenDevServerKey, with: true)
        defer { restore(BrowserPreferences.autoOpenDevServerKey, original: previousPreference) }

        let opener = BrowserDevServerAutoOpener(appState: harness.appState)
        _ = opener

        post(url: "https://localhost:3000", paneID: harness.backgroundPaneID)

        #expect(harness.activeArea.tabs.allSatisfy { $0.kind != .browser })
        #expect(harness.backgroundArea.tabs.contains { $0.kind == .browser })
    }

    @Test("does nothing when the auto-open preference is disabled")
    func noOpWhenPreferenceDisabled() {
        let harness = makeHarness()
        let previousPreference = swap(BrowserPreferences.autoOpenDevServerKey, with: false)
        defer { restore(BrowserPreferences.autoOpenDevServerKey, original: previousPreference) }

        let opener = BrowserDevServerAutoOpener(appState: harness.appState)
        _ = opener

        post(url: "https://localhost:3000", paneID: harness.backgroundPaneID)

        #expect(harness.activeArea.tabs.allSatisfy { $0.kind != .browser })
        #expect(harness.backgroundArea.tabs.allSatisfy { $0.kind != .browser })
    }

    private func swap(_ key: String, with value: Bool) -> Any? {
        let defaults = UserDefaults.standard
        let previous = defaults.object(forKey: key)
        defaults.set(value, forKey: key)
        return previous
    }

    private func restore(_ key: String, original: Any?) {
        if let original {
            UserDefaults.standard.set(original, forKey: key)
        } else {
            UserDefaults.standard.removeObject(forKey: key)
        }
    }

    private func post(url: String, paneID: UUID?) {
        var info: [String: Any] = [DevServerSnifferKeys.urlKey: url]
        if let paneID { info[DevServerSnifferKeys.paneIDKey] = paneID }
        NotificationCenter.default.post(
            name: .devServerDetected,
            object: nil,
            userInfo: info
        )
    }

    private func makeHarness() -> Harness {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let backgroundWorktreeID = UUID()
        let activeKey = WorktreeKey(projectID: projectID, worktreeID: activeWorktreeID)
        let backgroundKey = WorktreeKey(projectID: projectID, worktreeID: backgroundWorktreeID)
        let activeArea = TabArea(projectPath: "/tmp/active")
        let backgroundArea = TabArea(projectPath: "/tmp/background")
        guard let backgroundPane = backgroundArea.activeTab?.content.pane else {
            fatalError("Expected initial terminal pane in background area")
        }
        let appState = AppState(
            selectionStore: AutoOpenerSelectionStoreStub(),
            terminalViews: AutoOpenerTerminalViewRemovingStub(),
            workspacePersistence: AutoOpenerWorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = activeWorktreeID
        appState.workspaceRoots[activeKey] = .tabArea(activeArea)
        appState.workspaceRoots[backgroundKey] = .tabArea(backgroundArea)
        appState.focusedAreaID[activeKey] = activeArea.id
        appState.focusedAreaID[backgroundKey] = backgroundArea.id
        return Harness(
            appState: appState,
            activeArea: activeArea,
            backgroundArea: backgroundArea,
            backgroundPaneID: backgroundPane.id
        )
    }

    private struct Harness {
        let appState: AppState
        let activeArea: TabArea
        let backgroundArea: TabArea
        let backgroundPaneID: UUID
    }
}

private final class AutoOpenerWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class AutoOpenerSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class AutoOpenerTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
