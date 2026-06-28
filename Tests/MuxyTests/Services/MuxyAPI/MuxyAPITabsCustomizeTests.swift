import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI.Tabs.setTitle and setIcon")
@MainActor
struct MuxyAPITabsCustomizeTests {
    private let testPath = "/tmp/test"
    private let owningExtension = "pr-tools"

    @Test("setTitle overrides the tab title and empty resets to default")
    func setTitleOverridesAndResets() {
        let (appState, state) = makeAppStateWithExtensionTab()

        let set = MuxyAPI.Tabs.setTitle(
            instanceID: state.id.uuidString,
            title: "PR #42",
            appState: appState,
            callingExtensionID: owningExtension
        )
        guard case .success = set else { Issue.record("expected success"); return }
        #expect(state.displayTitle == "PR #42")

        let reset = MuxyAPI.Tabs.setTitle(
            instanceID: state.id.uuidString,
            title: "   ",
            appState: appState,
            callingExtensionID: owningExtension
        )
        guard case .success = reset else { Issue.record("expected success"); return }
        #expect(state.displayTitle == "Viewer")
    }

    @Test("setIcon stores and clears the custom icon")
    func setIconStoresAndClears() {
        let (appState, state) = makeAppStateWithExtensionTab()

        _ = MuxyAPI.Tabs.setIcon(
            instanceID: state.id.uuidString,
            icon: .symbol("swift"),
            appState: appState,
            callingExtensionID: owningExtension
        )
        #expect(state.customIcon == .symbol("swift"))

        _ = MuxyAPI.Tabs.setIcon(
            instanceID: state.id.uuidString,
            icon: nil,
            appState: appState,
            callingExtensionID: owningExtension
        )
        #expect(state.customIcon == nil)
    }

    @Test("an extension cannot customize a tab it does not own")
    func rejectsForeignExtension() {
        let (appState, state) = makeAppStateWithExtensionTab()

        let result = MuxyAPI.Tabs.setTitle(
            instanceID: state.id.uuidString,
            title: "Hijacked",
            appState: appState,
            callingExtensionID: "other-ext"
        )
        guard case let .failure(error) = result else { Issue.record("expected failure"); return }
        #expect(error == .tabNotFound(state.id.uuidString))
        #expect(state.customTitle == nil)
    }

    @Test("unknown instance id fails")
    func rejectsUnknownInstance() {
        let (appState, _) = makeAppStateWithExtensionTab()
        let missing = UUID().uuidString

        let result = MuxyAPI.Tabs.setTitle(
            instanceID: missing,
            title: "x",
            appState: appState,
            callingExtensionID: owningExtension
        )
        guard case let .failure(error) = result else { Issue.record("expected failure"); return }
        #expect(error == .tabNotFound(missing))
    }

    private func makeAppStateWithExtensionTab() -> (AppState, ExtensionTabState) {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        area.createExtensionTab(
            extensionID: owningExtension,
            tabTypeID: "pr-viewer",
            title: "Viewer",
            data: nil
        )
        let state = area.tabs.compactMap { $0.content.extensionState }.first!
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return (appState, state)
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
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
