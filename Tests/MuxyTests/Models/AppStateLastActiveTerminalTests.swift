import Foundation
import Testing

@testable import Muxy

@Suite("AppState.lastActiveTerminalPane")
@MainActor
struct AppStateLastActiveTerminalTests {
    @Test("focusing a terminal area records its pane as the last active terminal")
    func focusingTerminalRecordsLastActivePane() {
        let harness = makeHarness()
        let area = harness.area
        guard let pane = area.activeTab?.content.pane else {
            Issue.record("Expected initial terminal pane")
            return
        }

        harness.appState.dispatch(.focusArea(projectID: harness.projectID, areaID: area.id))

        #expect(harness.appState.lastActiveTerminalPaneID[harness.key] == pane.id)
    }

    @Test("switching to a browser tab keeps the previously active terminal pane")
    func switchingToBrowserKeepsTerminalRecord() {
        let harness = makeHarness()
        let area = harness.area
        guard let terminalPane = area.activeTab?.content.pane else {
            Issue.record("Expected initial terminal pane")
            return
        }

        harness.appState.dispatch(.createBrowserTab(
            projectID: harness.projectID,
            areaID: area.id,
            initialURL: "https://example.com"
        ))

        let browserTab = area.activeTab
        #expect(browserTab?.kind == .browser)
        #expect(harness.appState.lastActiveTerminalPaneID[harness.key] == terminalPane.id)

        guard let resolved = harness.appState.lastActiveTerminalPane(for: harness.key) else {
            Issue.record("Expected resolved last active terminal pane")
            return
        }
        #expect(resolved.paneID == terminalPane.id)
        #expect(resolved.areaID == area.id)
    }

    @Test("closing the tracked terminal clears the recorded pane")
    func closingTrackedTerminalClearsRecord() {
        let harness = makeHarness()
        let area = harness.area
        guard let terminalPane = area.activeTab?.content.pane,
              let terminalTabID = area.activeTabID
        else {
            Issue.record("Expected initial terminal pane")
            return
        }

        harness.appState.dispatch(.focusArea(projectID: harness.projectID, areaID: area.id))
        #expect(harness.appState.lastActiveTerminalPaneID[harness.key] == terminalPane.id)

        harness.appState.dispatch(.createBrowserTab(
            projectID: harness.projectID,
            areaID: area.id,
            initialURL: "https://example.com"
        ))
        harness.appState.dispatch(.closeTab(
            projectID: harness.projectID,
            areaID: area.id,
            tabID: terminalTabID
        ))

        #expect(harness.appState.lastActiveTerminalPaneID[harness.key] == nil)
        #expect(harness.appState.lastActiveTerminalPane(for: harness.key) == nil)
    }

    @Test("moving focus across areas records each terminal pane as last active")
    func movingFocusAcrossAreasUpdatesRecord() {
        let harness = makeHarness()
        let firstArea = harness.area
        guard let firstPane = firstArea.activeTab?.content.pane else {
            Issue.record("Expected initial terminal pane")
            return
        }

        harness.appState.dispatch(.splitArea(.init(
            projectID: harness.projectID,
            areaID: firstArea.id,
            direction: .horizontal,
            position: .second
        )))
        guard let secondAreaID = harness.appState.focusedAreaID[harness.key],
              secondAreaID != firstArea.id,
              let root = harness.appState.workspaceRoots[harness.key],
              let secondArea = root.findArea(id: secondAreaID),
              let secondPane = secondArea.activeTab?.content.pane
        else {
            Issue.record("Expected second area with a terminal pane")
            return
        }

        #expect(harness.appState.lastActiveTerminalPaneID[harness.key] == secondPane.id)

        harness.appState.dispatch(.focusArea(projectID: harness.projectID, areaID: firstArea.id))
        #expect(harness.appState.lastActiveTerminalPaneID[harness.key] == firstPane.id)
    }

    private func makeHarness() -> Harness {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/test")
        let appState = AppState(
            selectionStore: LastActiveSelectionStoreStub(),
            terminalViews: LastActiveTerminalViewRemovingStub(),
            workspacePersistence: LastActiveWorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return Harness(appState: appState, projectID: projectID, key: key, area: area)
    }

    private struct Harness {
        let appState: AppState
        let projectID: UUID
        let key: WorktreeKey
        let area: TabArea
    }
}

private final class LastActiveWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class LastActiveSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class LastActiveTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
