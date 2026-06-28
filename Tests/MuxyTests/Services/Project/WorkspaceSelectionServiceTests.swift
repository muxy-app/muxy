import Foundation
import Testing

@testable import Muxy

@Suite("WorkspaceSelectionService.selectFirstProject")
@MainActor
struct WorkspaceSelectionServiceTests {
    @Test("remote workspace selects its home when home is visible")
    func remoteSelectsHome() {
        let previousVisibility = HomeProjectPreferences.isVisible
        HomeProjectPreferences.isVisible = true
        defer { HomeProjectPreferences.isVisible = previousVisibility }
        let (appState, projectStore, worktreeStore, projectGroupStore, deviceStore) = makeStores()
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~"))
        let group = projectGroupStore.addRemoteWorkspace(name: "prod", deviceID: device.id)
        projectGroupStore.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id)
        projectGroupStore.selectGroup(id: group.id)

        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(appState.activeProjectID == projectGroupStore.activeRemoteHomeProject?.id)
    }

    @Test("remote workspace selects its first project when home is hidden")
    func remoteSelectsFirstProjectWhenHomeHidden() {
        let previousVisibility = HomeProjectPreferences.isVisible
        HomeProjectPreferences.isVisible = false
        defer { HomeProjectPreferences.isVisible = previousVisibility }
        let (appState, projectStore, worktreeStore, projectGroupStore, deviceStore) = makeStores()
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~"))
        let group = projectGroupStore.addRemoteWorkspace(name: "prod", deviceID: device.id)
        let remote = projectGroupStore.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id)
        projectGroupStore.selectGroup(id: group.id)

        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(appState.activeProjectID == remote?.id)
        #expect(appState.activeProjectID != projectGroupStore.activeRemoteHomeProject?.id)
    }

    @Test("local workspace selects the home project when home is visible")
    func localSelectsHome() {
        let previousVisibility = HomeProjectPreferences.isVisible
        HomeProjectPreferences.isVisible = true
        defer { HomeProjectPreferences.isVisible = previousVisibility }
        let (appState, projectStore, worktreeStore, projectGroupStore, _) = makeStores()
        projectStore.add(Project(name: "local", path: "/tmp/local"))

        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(appState.activeProjectID == Project.homeID)
    }

    private func makeStores() -> (AppState, ProjectStore, WorktreeStore, ProjectGroupStore, RemoteDeviceStore) {
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub())
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
        return (appState, projectStore, worktreeStore, projectGroupStore, deviceStore)
    }
}

private final class ProjectPersistenceStub: ProjectPersisting {
    private var projects: [Project] = []
    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
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
