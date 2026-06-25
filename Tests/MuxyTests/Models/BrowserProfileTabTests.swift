import Foundation
import Testing
import WebKit

@testable import Muxy

@Suite("BrowserProfileTab", .serialized)
@MainActor
struct BrowserProfileTabTests {
    private let testPath = "/tmp/test"

    private func makeState(projectID: UUID, worktreeID: UUID) -> WorkspaceState {
        var state = WorkspaceState(
            activeProjectID: projectID,
            activeWorktreeID: [projectID: worktreeID],
            workspaceRoots: [:],
            focusedAreaID: [:],
            focusHistory: [:]
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        state.workspaceRoots[key] = .tabArea(area)
        state.focusedAreaID[key] = area.id
        return state
    }

    private func focusedArea(in state: WorkspaceState, projectID: UUID) -> TabArea? {
        guard let worktreeID = state.activeWorktreeID[projectID] else { return nil }
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        guard let focusedID = state.focusedAreaID[key],
              let root = state.workspaceRoots[key]
        else { return nil }
        return root.findArea(id: focusedID)
    }

    @Test("createBrowserTab honors the profile id")
    func createBrowserTabWithProfile() {
        let projectID = UUID()
        let worktreeID = UUID()
        let profileID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.createBrowserTab(
            projectID: projectID,
            areaID: nil,
            url: URL(string: "https://muxy.app"),
            profileID: profileID
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let tab = focusedArea(in: state, projectID: projectID)?.activeTab
        #expect(tab?.content.browserState?.profileID == profileID)
    }

    @Test("createBrowserTab is a no-op when the browser is disabled")
    func disabledBlocksCreation() {
        let original = UserDefaults.standard.object(forKey: BrowserPreferences.enabledKey)
        defer {
            if let original {
                UserDefaults.standard.set(original, forKey: BrowserPreferences.enabledKey)
            } else {
                UserDefaults.standard.removeObject(forKey: BrowserPreferences.enabledKey)
            }
        }
        BrowserPreferences.isEnabled = false

        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.createBrowserTab(
            projectID: projectID,
            areaID: nil,
            url: URL(string: "https://muxy.app"),
            profileID: BrowserProfile.defaultID
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let area = focusedArea(in: state, projectID: projectID)
        #expect(area?.tabs.contains { $0.kind == .browser } == false)
    }

    @Test("browser profile survives snapshot round-trip")
    func snapshotRoundTrip() {
        let profileID = UUID()
        let state = BrowserTabState(
            projectPath: testPath,
            url: URL(string: "https://muxy.app"),
            profileID: profileID
        )
        let snapshot = TerminalTab(browserState: state).snapshot()
        #expect(snapshot.browserProfileID == profileID.uuidString)

        let restored = TerminalTab(restoring: snapshot)
        #expect(restored.content.browserState?.profileID == profileID)
    }

    @Test("restoring a snapshot without a profile id falls back to default")
    func restoreFallsBackToDefault() {
        let snapshot = TerminalTabSnapshot(
            kind: .browser,
            customTitle: nil,
            colorID: nil,
            isPinned: false,
            projectPath: testPath,
            paneTitle: "Browser",
            browserURL: "https://muxy.app",
            browserProfileID: nil
        )
        let restored = TerminalTab(restoring: snapshot)
        #expect(restored.content.browserState?.profileID == BrowserProfile.defaultID)
    }

    @Test("switching browser profile reloads the current page")
    func switchProfileReloadsCurrentPage() throws {
        let url = try #require(URL(string: "https://muxy.app"))
        let profileID = UUID()
        let state = BrowserTabState(projectPath: testPath, url: url)
        _ = state.navigationURLForWebViewMount()

        state.switchProfile(to: profileID)

        #expect(state.profileID == profileID)
        #expect(state.pendingURL == url)
    }

    @Test("switching to the same browser profile is a no-op")
    func switchProfileSameProfileDoesNothing() throws {
        let url = try #require(URL(string: "https://muxy.app"))
        let profileID = UUID()
        let state = BrowserTabState(projectPath: testPath, url: url, profileID: profileID)
        _ = state.navigationURLForWebViewMount()

        state.switchProfile(to: profileID)

        #expect(state.profileID == profileID)
        #expect(state.pendingURL == nil)
    }

    @Test("active browser inspect request uses registered web view")
    func inspectActiveBrowserElementUsesRegisteredWebView() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID

        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let browserState = BrowserTabState(projectPath: testPath, url: URL(string: "https://muxy.app"))
        let area = TabArea(projectPath: testPath, existingTab: TerminalTab(browserState: browserState))
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id

        let webView = InspectingWebViewStub(frame: .zero)
        BrowserWebViewRegistry.shared.register(webView, for: browserState.id)
        defer { BrowserWebViewRegistry.shared.unregister(browserState.id) }

        #expect(appState.inspectActiveBrowserElement())
        #expect(webView.inspectCount == 1)
        #expect(browserState.pendingCommand == nil)
    }

    @Test("active browser inspect request without a live web view becomes pending command")
    func inspectActiveBrowserElementSetsPendingCommand() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID

        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let browserState = BrowserTabState(projectPath: testPath, url: URL(string: "https://muxy.app"))
        let area = TabArea(projectPath: testPath, existingTab: TerminalTab(browserState: browserState))
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id

        #expect(appState.inspectActiveBrowserElement())
        #expect(browserState.pendingCommand == .inspectElement)
    }

    @Test("active non-browser inspect request is ignored")
    func inspectActiveBrowserElementIgnoresTerminalTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID

        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id

        #expect(!appState.inspectActiveBrowserElement())
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class InspectingWebViewStub: WKWebView, BrowserElementInspecting {
    var inspectCount = 0
    var closeCount = 0

    func inspectElement() -> Bool {
        inspectCount += 1
        return true
    }

    func closeInspector() -> Bool {
        closeCount += 1
        return true
    }
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
