import Foundation
import Testing
import WebKit

@testable import Muxy

@Suite("BrowserWebView window.close", .serialized)
@MainActor
struct BrowserWebViewCloseTests {
    private let testPath = "/tmp/test"

    private struct Fixture {
        let appState: AppState
        let key: WorktreeKey
        let tabID: UUID
        let state: BrowserTabState
        let coordinator: BrowserWebView.Coordinator
    }

    private func makeFixture() throws -> Fixture {
        UserDefaults.standard.removeObject(forKey: BrowserPreferences.enabledKey)
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = AppState(
            selectionStore: CloseSelectionStoreStub(),
            terminalViews: CloseTerminalViewRemovingStub(),
            workspacePersistence: CloseWorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id

        let tabID = try MuxyAPI.Browser.open(url: "https://example.com", appState: appState).get()
        let tab = try #require(appState.workspaceRoots[key]?.locateTab(id: tabID)?.tab)
        let state = try #require(tab.content.browserState)
        let coordinator = BrowserWebView.Coordinator(
            state: state,
            appState: appState,
            historyStore: BrowserHistoryStore(persistence: InMemoryBrowserHistoryPersistence())
        )
        return Fixture(appState: appState, key: key, tabID: tabID, state: state, coordinator: coordinator)
    }

    private func tabIDs(_ fixture: Fixture) -> [UUID] {
        fixture.appState.workspaceRoots[fixture.key]?.allAreas().flatMap { $0.tabs.map(\.id) } ?? []
    }

    @Test("webViewDidClose closes the browser tab that hosts the page")
    func closesOwningTab() throws {
        let fixture = try makeFixture()
        #expect(tabIDs(fixture).contains(fixture.tabID))

        fixture.coordinator.webViewDidClose(WKWebView(frame: .zero))

        #expect(!tabIDs(fixture).contains(fixture.tabID))
    }

    @Test("webViewDidClose leaves a pinned tab open")
    func keepsPinnedTabOpen() throws {
        let fixture = try makeFixture()
        let tab = try #require(fixture.appState.workspaceRoots[fixture.key]?.locateTab(id: fixture.tabID)?.tab)
        tab.isPinned = true

        fixture.coordinator.webViewDidClose(WKWebView(frame: .zero))

        #expect(tabIDs(fixture).contains(fixture.tabID))
    }

    @Test("webViewDidClose closes only the tab that owns the state")
    func leavesSiblingTabsOpen() throws {
        let fixture = try makeFixture()
        let siblingID = try MuxyAPI.Browser.open(url: "https://sibling.test", appState: fixture.appState).get()

        fixture.coordinator.webViewDidClose(WKWebView(frame: .zero))

        #expect(!tabIDs(fixture).contains(fixture.tabID))
        #expect(tabIDs(fixture).contains(siblingID))
    }

    @Test("locateTab finds the tab hosting a browser state")
    func locatesBrowserStateTab() throws {
        let fixture = try makeFixture()

        let located = try #require(fixture.appState.locateTab(forBrowserState: fixture.state.id))

        #expect(located.tab.id == fixture.tabID)
        #expect(located.worktreeKey == fixture.key)
    }

    @Test("locateTab returns nil for an unknown browser state")
    func locateUnknownBrowserStateReturnsNil() throws {
        let fixture = try makeFixture()

        #expect(fixture.appState.locateTab(forBrowserState: UUID()) == nil)
    }
}

@MainActor
private final class CloseSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class CloseTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}

private final class CloseWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}
