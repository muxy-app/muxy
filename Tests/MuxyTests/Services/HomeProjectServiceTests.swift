import Foundation
import Testing

@testable import Muxy

@Suite("HomeProjectService.openHomeTab")
@MainActor
struct HomeProjectServiceTests {
    @Test("remote workspace opens the remote home project, not the local home")
    func remoteWorkspaceOpensRemoteHome() {
        let (appState, worktreeStore, projectGroupStore, deviceStore) = makeStores()
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~/code"))
        let group = projectGroupStore.addRemoteWorkspace(name: "prod", deviceID: device.id)
        projectGroupStore.selectGroup(id: group.id)

        let opened = HomeProjectService.openHomeTab(
            appState: appState,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        let remoteHome = projectGroupStore.activeRemoteHomeProject
        #expect(opened)
        #expect(appState.activeProjectID == remoteHome?.id)
        #expect(appState.activeProjectID != Project.homeID)
        #expect(remoteHome?.remoteWorkspaceID == group.id)
        #expect(remoteHome?.path == "~/code")
    }

    @Test("local workspace opens the local home project")
    func localWorkspaceOpensLocalHome() {
        let previousVisibility = HomeProjectPreferences.isVisible
        HomeProjectPreferences.isVisible = true
        defer { HomeProjectPreferences.isVisible = previousVisibility }
        let (appState, worktreeStore, projectGroupStore, _) = makeStores()

        let opened = HomeProjectService.openHomeTab(
            appState: appState,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(opened)
        #expect(appState.activeProjectID == Project.homeID)
    }

    private func makeStores() -> (AppState, WorktreeStore, ProjectGroupStore, RemoteDeviceStore) {
        let worktreeStore = WorktreeStore(persistence: WorktreePersistenceStub(), projects: [])
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let deviceStore = RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence())
        let projectGroupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: deviceStore,
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        return (appState, worktreeStore, projectGroupStore, deviceStore)
    }
}

private final class WorktreePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]] = [:]
    func loadWorktrees(projectID: UUID) throws -> [Worktree] { storage[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws {
        storage[projectID] = worktrees
    }

    func removeWorktrees(projectID: UUID) throws { storage.removeValue(forKey: projectID) }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws { snapshots = workspaces }
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
    func removeView(for paneID: UUID) {}
    func needsConfirmQuit(for paneID: UUID) -> Bool { false }
}
