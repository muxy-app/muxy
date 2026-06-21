import Foundation
import Testing

@testable import Muxy

@Suite("BrowserProfileTab")
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
}
