import Foundation
import Testing

@testable import Muxy

@Suite("AppState background sessions", .serialized)
@MainActor
struct AppStateBackgroundSessionTests {
    @Test("sending the final tab to the background releases its session and keeps the workspace")
    func backgroundFinalTab() throws {
        let context = makeContext()
        let pane = try #require(context.area.activeTab?.content.pane)
        context.terminalViews.persistentSessions[pane.id] = pane.sessionID

        #expect(context.appState.canSendTabToBackground(paneID: pane.id))
        #expect(context.appState.sendTabToBackground(paneID: pane.id))
        #expect(context.terminalViews.releasedPaneIDs == [pane.id])
        #expect(context.terminalViews.removedPaneIDs.isEmpty)
        #expect(context.appState.workspaceRoots[context.key] != nil)
        #expect(context.appState.workspaceRoots[context.key]?.allTabs().isEmpty == true)
        #expect(context.appState.activeProjectID == context.key.projectID)
    }

    @Test("sending a split tab to the background releases every terminal session")
    func backgroundSplitTab() throws {
        let context = makeContext()
        let rootTab = try #require(context.area.activeTab)
        let rootPane = try #require(rootTab.content.pane)
        let childPane = TerminalPaneState(projectPath: context.area.projectPath)
        let childTab = TerminalTab(pane: childPane, parentTabID: rootTab.id)
        let childArea = TabArea(projectPath: context.area.projectPath, existingTab: childTab)
        context.appState.workspaceRoots[context.key] = .split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(context.area),
            second: .tabArea(childArea)
        ))
        context.terminalViews.persistentSessions = [
            rootPane.id: rootPane.sessionID,
            childPane.id: childPane.sessionID,
        ]

        #expect(context.appState.canSendTabToBackground(paneID: childPane.id))
        #expect(context.appState.sendTabToBackground(paneID: childPane.id))
        #expect(Set(context.terminalViews.releasedPaneIDs) == Set([rootPane.id, childPane.id]))
        #expect(context.appState.workspaceRoots[context.key]?.allTabs().isEmpty == true)
    }

    @Test("ordinary, pinned, and mixed-content tabs cannot be sent to the background")
    func rejectsIneligibleTabs() throws {
        let context = makeContext()
        let rootTab = try #require(context.area.activeTab)
        let rootPane = try #require(rootTab.content.pane)

        #expect(!context.appState.canSendTabToBackground(paneID: rootPane.id))

        context.terminalViews.persistentSessions[rootPane.id] = rootPane.sessionID
        context.area.togglePin(rootTab.id)
        #expect(!context.appState.canSendTabToBackground(paneID: rootPane.id))

        context.area.togglePin(rootTab.id)
        let extensionState = ExtensionTabState(
            extensionID: "editor",
            tabTypeID: "document",
            projectPath: context.area.projectPath,
            defaultTitle: "Document"
        )
        let extensionTab = TerminalTab(extensionState: extensionState, parentTabID: rootTab.id)
        let childArea = TabArea(projectPath: context.area.projectPath, existingTab: extensionTab)
        context.appState.workspaceRoots[context.key] = .split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(context.area),
            second: .tabArea(childArea)
        ))

        #expect(!context.appState.canSendTabToBackground(paneID: rootPane.id))
        #expect(!context.appState.sendTabToBackground(paneID: rootPane.id))
        #expect(context.terminalViews.releasedPaneIDs.isEmpty)
    }

    private func makeContext() -> BackgroundSessionTestContext {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let terminalViews = BackgroundSessionTerminalViews()
        let appState = AppState(
            selectionStore: BackgroundSessionSelectionStore(),
            terminalViews: terminalViews,
            workspacePersistence: BackgroundSessionWorkspacePersistence()
        )
        let area = TabArea(projectPath: "/tmp/test")
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return BackgroundSessionTestContext(
            appState: appState,
            terminalViews: terminalViews,
            key: key,
            area: area
        )
    }
}

@MainActor
private struct BackgroundSessionTestContext {
    let appState: AppState
    let terminalViews: BackgroundSessionTerminalViews
    let key: WorktreeKey
    let area: TabArea
}

@MainActor
private final class BackgroundSessionTerminalViews: TerminalViewRemoving {
    var persistentSessions: [UUID: UUID] = [:]
    private(set) var removedPaneIDs: [UUID] = []
    private(set) var releasedPaneIDs: [UUID] = []

    func removeView(for paneID: UUID) {
        removedPaneIDs.append(paneID)
    }

    func releaseViewPreservingSession(for paneID: UUID) {
        releasedPaneIDs.append(paneID)
    }

    func hasPersistentSession(for paneID: UUID, sessionID: UUID) -> Bool {
        persistentSessions[paneID] == sessionID
    }

    func needsConfirmQuit(for _: UUID) -> Bool {
        false
    }
}

@MainActor
private final class BackgroundSessionSelectionStore: ActiveProjectSelectionStoring {
    private var projectID: UUID?
    private var worktreeIDs: [UUID: UUID] = [:]

    func loadActiveProjectID() -> UUID? { projectID }
    func saveActiveProjectID(_ id: UUID?) { projectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { worktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { worktreeIDs = ids }
}

private final class BackgroundSessionWorkspacePersistence: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []

    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws { snapshots = workspaces }
}
