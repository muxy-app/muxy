import Foundation
import Testing

@testable import Muxy

@Suite("BrowserTabReducer")
@MainActor
struct BrowserTabReducerTests {
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

    @Test("createBrowserTab inserts a browser tab with the url")
    func createBrowserTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let url = URL(string: "https://muxy.app")

        let action = AppState.Action.createBrowserTab(
            projectID: projectID,
            areaID: nil,
            url: url,
            profileID: BrowserProfile.defaultID
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let area = focusedArea(in: state, projectID: projectID)
        let tab = area?.activeTab
        #expect(tab?.kind == .browser)
        #expect(tab?.content.browserState?.url == url)
        #expect(area?.tabs.contains { $0.kind == .browser } == true)
    }

    @Test("createBrowserTab with nil url inserts a blank browser tab")
    func createBlankBrowserTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.createBrowserTab(
            projectID: projectID,
            areaID: nil,
            url: nil,
            profileID: BrowserProfile.defaultID
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let tab = focusedArea(in: state, projectID: projectID)?.activeTab
        #expect(tab?.kind == .browser)
        #expect(tab?.content.browserState?.url == nil)
        #expect(tab?.title == "New Tab")
    }

    @Test("browser tab survives snapshot round-trip")
    func snapshotRoundTrip() {
        let state = BrowserTabState(projectPath: testPath, url: URL(string: "https://muxy.app/docs"))
        let tab = TerminalTab(browserState: state)
        let snapshot = tab.snapshot()
        #expect(snapshot.kind == .browser)
        #expect(snapshot.browserURL == "https://muxy.app/docs")

        let restored = TerminalTab(restoring: snapshot)
        #expect(restored.kind == .browser)
        #expect(restored.content.browserState?.url?.absoluteString == "https://muxy.app/docs")
    }

    @Test("a newly created browser tab requests address field focus")
    func newBrowserTabFocusesAddress() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.createBrowserTab(
            projectID: projectID,
            areaID: nil,
            url: nil,
            profileID: BrowserProfile.defaultID
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let tab = focusedArea(in: state, projectID: projectID)?.activeTab
        #expect(tab?.content.browserState?.shouldFocusAddressOnOpen == true)
    }

    @Test("a restored browser tab does not request address field focus")
    func restoredBrowserTabDoesNotFocusAddress() {
        let state = BrowserTabState(projectPath: testPath, url: URL(string: "https://muxy.app/docs"))
        let tab = TerminalTab(browserState: state)
        let restored = TerminalTab(restoring: tab.snapshot())
        #expect(restored.content.browserState?.shouldFocusAddressOnOpen == false)
    }

    @Test("browser tab remount reuses the current url")
    func remountUsesCurrentURL() throws {
        let url = try #require(URL(string: "https://muxy.app/docs"))
        let state = BrowserTabState(projectPath: testPath, url: url)

        #expect(state.navigationURLForWebViewMount() == url)
        #expect(state.pendingURL == nil)
        #expect(state.navigationURLForWebViewMount() == url)
    }

    @Test("pending browser navigation wins over the current url")
    func pendingNavigationWinsOverCurrentURL() throws {
        let firstURL = try #require(URL(string: "https://muxy.app"))
        let secondURL = try #require(URL(string: "https://muxy.app/docs"))
        let state = BrowserTabState(projectPath: testPath, url: firstURL)
        _ = state.navigationURLForWebViewMount()

        state.pendingURL = secondURL

        #expect(state.navigationURLForWebViewMount() == secondURL)
        #expect(state.url == secondURL)
        #expect(state.pendingURL == nil)
    }
}
