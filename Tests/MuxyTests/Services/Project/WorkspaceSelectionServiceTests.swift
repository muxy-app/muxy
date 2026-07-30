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

    @Test("focus mode stays on when active project is in workspace")
    func focusModeStaysOnWhenActiveProjectIsInWorkspace() {
        let (appState, projectStore, worktreeStore, projectGroupStore, _) = makeStores()
        let alpha = Project(name: "alpha", path: "/tmp/alpha")
        let beta = Project(name: "beta", path: "/tmp/beta")
        projectStore.add(alpha)
        projectStore.add(beta)
        projectGroupStore.addGroup(name: "work")
        let group = projectGroupStore.groups.first { $0.name == "work" }
        let groupID = group?.id ?? UUID()
        projectGroupStore.addProject(projectID: alpha.id, toGroup: groupID)
        projectGroupStore.addProject(projectID: beta.id, toGroup: groupID)
        projectGroupStore.selectGroup(id: groupID)
        worktreeStore.add(Worktree(name: "main", path: alpha.path, isPrimary: true), to: alpha.id)
        appState.selectProject(alpha, worktree: worktreeStore.primary(for: alpha.id)!)
        let (expansionStore, layoutStore) = resetFocusModeAndLayout()
        defer { expansionStore.focusMode = false; layoutStore.set(.projectFocused) }
        expansionStore.focusMode = true
        layoutStore.set(.tabFocused)

        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(appState.activeProjectID == alpha.id)
        #expect(expansionStore.focusMode == true)
    }

    @Test("focus mode stays on for the visible local home project")
    func focusModeStaysOnForVisibleLocalHomeProject() {
        let previousVisibility = HomeProjectPreferences.isVisible
        HomeProjectPreferences.isVisible = true
        defer { HomeProjectPreferences.isVisible = previousVisibility }
        let (appState, projectStore, worktreeStore, projectGroupStore, _) = makeStores()
        projectStore.add(Project(name: "project", path: "/tmp/project"))
        appState.activeProjectID = Project.homeID
        let (expansionStore, layoutStore) = resetFocusModeAndLayout()
        defer { expansionStore.focusMode = false; layoutStore.set(.projectFocused) }
        expansionStore.focusMode = true
        layoutStore.set(.tabFocused)

        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(expansionStore.focusMode == true)
    }

    @Test("focus mode stays on for the visible remote home project")
    func focusModeStaysOnForVisibleRemoteHomeProject() {
        let previousVisibility = HomeProjectPreferences.isVisible
        HomeProjectPreferences.isVisible = true
        defer { HomeProjectPreferences.isVisible = previousVisibility }
        let (appState, projectStore, worktreeStore, projectGroupStore, deviceStore) = makeStores()
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~"))
        let group = projectGroupStore.addRemoteWorkspace(name: "prod", deviceID: device.id)
        projectGroupStore.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id)
        projectGroupStore.selectGroup(id: group.id)
        appState.activeProjectID = projectGroupStore.activeRemoteHomeProject?.id
        let (expansionStore, layoutStore) = resetFocusModeAndLayout()
        defer { expansionStore.focusMode = false; layoutStore.set(.projectFocused) }
        expansionStore.focusMode = true
        layoutStore.set(.tabFocused)

        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(expansionStore.focusMode == true)
    }

    @Test("focus mode turns off when active project is not in workspace")
    func focusModeTurnsOffWhenActiveProjectIsNotInWorkspace() {
        let previousVisibility = HomeProjectPreferences.isVisible
        HomeProjectPreferences.isVisible = false
        defer { HomeProjectPreferences.isVisible = previousVisibility }
        let (appState, projectStore, worktreeStore, projectGroupStore, _) = makeStores()
        let alpha = Project(name: "alpha", path: "/tmp/alpha")
        let beta = Project(name: "beta", path: "/tmp/beta")
        projectStore.add(alpha)
        projectStore.add(beta)
        projectGroupStore.addGroup(name: "work")
        let group = projectGroupStore.groups.first { $0.name == "work" }
        let groupID = group?.id ?? UUID()
        projectGroupStore.addProject(projectID: beta.id, toGroup: groupID)
        projectGroupStore.selectGroup(id: groupID)
        worktreeStore.add(Worktree(name: "main", path: alpha.path, isPrimary: true), to: alpha.id)
        appState.selectProject(alpha, worktree: worktreeStore.primary(for: alpha.id)!)
        let (expansionStore, layoutStore) = resetFocusModeAndLayout()
        defer { expansionStore.focusMode = false; layoutStore.set(.projectFocused) }
        expansionStore.focusMode = true
        layoutStore.set(.tabFocused)

        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(expansionStore.focusMode == false)
        #expect(appState.activeProjectID == beta.id)
    }

    @Test("selects first project when focus mode is off")
    func selectsFirstProjectWhenFocusModeIsOff() {
        let previousVisibility = HomeProjectPreferences.isVisible
        HomeProjectPreferences.isVisible = false
        defer { HomeProjectPreferences.isVisible = previousVisibility }
        let (appState, projectStore, worktreeStore, projectGroupStore, _) = makeStores()
        let alpha = Project(name: "alpha", path: "/tmp/alpha")
        let beta = Project(name: "beta", path: "/tmp/beta")
        projectStore.add(alpha)
        projectStore.add(beta)
        projectGroupStore.addGroup(name: "work")
        let group = projectGroupStore.groups.first { $0.name == "work" }
        let groupID = group?.id ?? UUID()
        projectGroupStore.addProject(projectID: beta.id, toGroup: groupID)
        projectGroupStore.selectGroup(id: groupID)
        worktreeStore.add(Worktree(name: "main", path: alpha.path, isPrimary: true), to: alpha.id)
        appState.selectProject(alpha, worktree: worktreeStore.primary(for: alpha.id)!)
        let (expansionStore, layoutStore) = resetFocusModeAndLayout()
        defer { expansionStore.focusMode = false; layoutStore.set(.projectFocused) }
        layoutStore.set(.tabFocused)

        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(appState.activeProjectID == beta.id)
    }

    @Test("project-focused layout ignores focus mode and selects first project")
    func projectFocusedLayoutIgnoresFocusMode() {
        let previousVisibility = HomeProjectPreferences.isVisible
        HomeProjectPreferences.isVisible = false
        defer { HomeProjectPreferences.isVisible = previousVisibility }
        let (appState, projectStore, worktreeStore, projectGroupStore, _) = makeStores()
        let alpha = Project(name: "alpha", path: "/tmp/alpha")
        let beta = Project(name: "beta", path: "/tmp/beta")
        projectStore.add(alpha)
        projectStore.add(beta)
        projectGroupStore.addGroup(name: "work")
        let group = projectGroupStore.groups.first { $0.name == "work" }
        let groupID = group?.id ?? UUID()
        projectGroupStore.addProject(projectID: beta.id, toGroup: groupID)
        projectGroupStore.selectGroup(id: groupID)
        worktreeStore.add(Worktree(name: "main", path: alpha.path, isPrimary: true), to: alpha.id)
        appState.selectProject(alpha, worktree: worktreeStore.primary(for: alpha.id)!)
        let (expansionStore, layoutStore) = resetFocusModeAndLayout()
        defer { expansionStore.focusMode = false; layoutStore.set(.projectFocused) }
        expansionStore.focusMode = true
        layoutStore.set(.projectFocused)

        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(appState.activeProjectID == beta.id)
        #expect(expansionStore.focusMode == true)
    }

    private func resetFocusModeAndLayout() -> (TabFocusedSidebarState, AppLayoutStore) {
        let expansionStore = TabFocusedSidebarState.shared
        let layoutStore = AppLayoutStore.shared
        expansionStore.focusMode = false
        layoutStore.set(.projectFocused)
        return (expansionStore, layoutStore)
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
