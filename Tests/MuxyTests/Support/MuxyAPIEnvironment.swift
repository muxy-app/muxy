import Foundation

@testable import Muxy

final class ProjectGroupPersistenceStub: ProjectGroupPersisting {
    var groups: [ProjectGroup]
    var savedGroups: [ProjectGroup]?
    var storedActiveGroupID: UUID?

    init(initial: [ProjectGroup] = [], storedActiveGroupID: UUID? = nil) {
        groups = initial
        self.storedActiveGroupID = storedActiveGroupID
    }

    func loadProjectGroups() throws -> [ProjectGroup] {
        groups
    }

    func saveProjectGroups(_ groups: [ProjectGroup]) throws {
        savedGroups = groups
        self.groups = groups
    }

    func loadActiveGroupID() -> UUID? {
        storedActiveGroupID
    }

    func saveActiveGroupID(_ id: UUID?) {
        storedActiveGroupID = id
    }
}

final class ProjectManagementPersistenceStub: ProjectPersisting {
    var projects: [Project]

    init(initial: [Project]) {
        projects = initial
    }

    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
}

final class ProjectManagementWorktreePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]] = [:]

    func loadWorktrees(projectID: UUID) throws -> [Worktree] { storage[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { storage[projectID] = worktrees }
    func removeWorktrees(projectID: UUID) throws { storage.removeValue(forKey: projectID) }
}

final class ProjectManagementWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
final class ProjectManagementSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
final class ProjectManagementTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}

@MainActor
struct ProjectManagementEnvironment {
    let appState: AppState
    let projectStore: ProjectStore
    let worktreeStore: WorktreeStore
    let projectGroupStore: ProjectGroupStore

    init(projects: [Project] = []) {
        projectStore = ProjectStore(persistence: ProjectManagementPersistenceStub(initial: projects))
        worktreeStore = WorktreeStore(
            persistence: ProjectManagementWorktreePersistenceStub(),
            projects: projects
        )
        appState = AppState(
            selectionStore: ProjectManagementSelectionStoreStub(),
            terminalViews: ProjectManagementTerminalViewRemovingStub(),
            workspacePersistence: ProjectManagementWorkspacePersistenceStub()
        )
        projectGroupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
    }

    var context: MuxyAPI.Projects.Context {
        MuxyAPI.Projects.Context(
            extensionID: "test",
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }
}
