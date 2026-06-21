import Foundation
import Testing
import WebKit

@testable import Muxy

@Suite("MuxyAPI.Browser", .serialized)
@MainActor
struct MuxyAPIBrowserTests {
    private let testPath = "/tmp/test"

    private func makeAppState(projectID: UUID = UUID(), worktreeID: UUID = UUID()) -> AppState {
        UserDefaults.standard.removeObject(forKey: BrowserPreferences.enabledKey)
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return appState
    }

    @Test("open returns a browser tab id")
    func openReturnsID() throws {
        let appState = makeAppState()
        let result = MuxyAPI.Browser.open(url: "https://example.com", appState: appState)
        let id = try result.get()
        let listed = MuxyAPI.Browser.list(appState: appState, profileStore: nil)
        #expect(listed.contains { $0.id == id })
        #expect(listed.first { $0.id == id }?.url == "https://example.com")
    }

    @Test("open without active project fails")
    func openNoProjectFails() {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        UserDefaults.standard.removeObject(forKey: BrowserPreferences.enabledKey)
        let result = MuxyAPI.Browser.open(url: nil, appState: appState)
        #expect(result == .failure(.noActiveProject))
    }

    @Test("open fails when the browser is disabled")
    func openDisabledFails() {
        let appState = makeAppState()
        BrowserPreferences.isEnabled = false
        defer { UserDefaults.standard.removeObject(forKey: BrowserPreferences.enabledKey) }
        let result = MuxyAPI.Browser.open(url: "https://example.com", appState: appState)
        #expect(result == .failure(.browserDisabled))
    }

    @Test("navigate updates the pending url")
    func navigateUpdatesURL() throws {
        let appState = makeAppState()
        let id = try MuxyAPI.Browser.open(url: "https://example.com", appState: appState).get()
        let result = MuxyAPI.Browser.navigate(tabIDString: id.uuidString, url: "muxy.app/docs", appState: appState)
        #expect(isSuccess(result))
    }

    @Test("navigate fails for an unknown tab id")
    func navigateUnknownFails() {
        let appState = makeAppState()
        let unknownID = UUID().uuidString
        let result = MuxyAPI.Browser.navigate(tabIDString: unknownID, url: "https://x.com", appState: appState)
        #expect(failureError(result) == .browserTabNotFound(unknownID))
    }

    private func isSuccess(_ result: Result<Void, APIError>) -> Bool {
        if case .success = result { return true }
        return false
    }

    private func failureError(_ result: Result<Void, APIError>) -> APIError? {
        if case let .failure(error) = result { return error }
        return nil
    }

    @Test("close removes the browser tab")
    func closeRemovesTab() throws {
        let appState = makeAppState()
        let id = try MuxyAPI.Browser.open(url: "https://example.com", appState: appState).get()
        _ = MuxyAPI.Browser.close(tabIDString: id.uuidString, appState: appState)
        let listed = MuxyAPI.Browser.list(appState: appState, profileStore: nil)
        #expect(!listed.contains { $0.id == id })
    }

    @Test("close fails when the browser is disabled")
    func closeDisabledFails() throws {
        let appState = makeAppState()
        let id = try MuxyAPI.Browser.open(url: "https://example.com", appState: appState).get()
        BrowserPreferences.isEnabled = false
        defer { UserDefaults.standard.removeObject(forKey: BrowserPreferences.enabledKey) }
        #expect(failureError(MuxyAPI.Browser.close(tabIDString: id.uuidString, appState: appState)) == .browserDisabled)
    }

    @Test("list resolves the profile name from the store")
    func listResolvesProfileName() throws {
        let appState = makeAppState()
        let store = BrowserProfileStore(persistence: BrowserProfilePersistenceStub())
        let profile = store.add(name: "Work")
        let id = try MuxyAPI.Browser.open(
            url: "https://example.com",
            profileID: profile.id,
            appState: appState
        ).get()
        let listed = MuxyAPI.Browser.list(appState: appState, profileStore: store)
        #expect(listed.first { $0.id == id }?.profile == "Work")
    }

    @Test("list returns empty when the browser is disabled")
    func listDisabledReturnsEmpty() throws {
        let appState = makeAppState()
        _ = try MuxyAPI.Browser.open(url: "https://example.com", appState: appState).get()
        BrowserPreferences.isEnabled = false
        defer { UserDefaults.standard.removeObject(forKey: BrowserPreferences.enabledKey) }
        #expect(MuxyAPI.Browser.list(appState: appState, profileStore: nil).isEmpty)
    }

    @Test("registry resolves a registered web view then clears on unregister")
    func registryRegistersAndUnregisters() {
        let registry = BrowserWebViewRegistry.shared
        let tabID = UUID()
        let webView = WKWebView(frame: .zero)
        registry.register(webView, for: tabID)
        #expect(registry.webView(for: tabID) === webView)
        registry.unregister(tabID)
        #expect(registry.webView(for: tabID) == nil)
    }
}

private final class BrowserProfilePersistenceStub: BrowserProfilePersisting {
    private var stored: [BrowserProfile] = []
    func loadProfiles() throws -> [BrowserProfile] { stored }
    func saveProfiles(_ profiles: [BrowserProfile]) throws { stored = profiles }
}

private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}
