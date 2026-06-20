import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("RemoteServerDelegate workspaces")
@MainActor
struct RemoteServerDelegateWorkspacesTests {
    @Test("listWorkspaces always exposes the default Local workspace")
    func defaultLocalWorkspaceExists() {
        let delegate = makeDelegate(projects: [], groups: [])

        let workspaces = delegate.listWorkspaces()

        #expect(workspaces.count == 1)
        let local = workspaces.first
        #expect(local?.id == WorkspaceInfoDTO.defaultLocalID)
        #expect(local?.isDefault == true)
        #expect(local?.kind == .local)
    }

    @Test("ungrouped local projects and Home land in the default Local workspace")
    func ungroupedProjectsInDefaultWorkspace() {
        let project = localProject(name: "Alpha")
        let delegate = makeDelegate(projects: [project], groups: [])

        let local = delegate.listProjectsByWorkspace(workspaceID: WorkspaceInfoDTO.defaultLocalID)

        #expect(local.contains { $0.id == project.id })
        #expect(local.contains { $0.id == Project.homeID })
        #expect(local.allSatisfy { $0.workspaceID == WorkspaceInfoDTO.defaultLocalID })
    }

    @Test("a local group exposes only its members and excludes them from default")
    func localGroupScopesProjects() {
        let grouped = localProject(name: "Grouped")
        let loose = localProject(name: "Loose")
        let group = ProjectGroup(name: "Frontend", projectIDs: [grouped.id])
        let delegate = makeDelegate(projects: [grouped, loose], groups: [group])

        let workspaces = delegate.listWorkspaces()
        let groupInfo = workspaces.first { $0.id == group.id }
        #expect(groupInfo?.name == "Frontend")
        #expect(groupInfo?.kind == .local)
        #expect(groupInfo?.projectCount == 1)

        let groupProjects = delegate.listProjectsByWorkspace(workspaceID: group.id)
        #expect(groupProjects.map(\.id) == [grouped.id])
        #expect(groupProjects.first?.workspaceID == group.id)
        #expect(groupProjects.first?.workspaceName == "Frontend")

        let defaultProjects = delegate.listProjectsByWorkspace(workspaceID: WorkspaceInfoDTO.defaultLocalID)
        #expect(defaultProjects.contains { $0.id == loose.id })
        #expect(!defaultProjects.contains { $0.id == grouped.id })
    }

    @Test("listProjects equals the union of every workspace's projects")
    func listProjectsMatchesWorkspaceUnion() {
        let grouped = localProject(name: "Grouped")
        let loose = localProject(name: "Loose")
        let group = ProjectGroup(name: "Frontend", projectIDs: [grouped.id])
        let delegate = makeDelegate(projects: [grouped, loose], groups: [group])

        let flat = Set(delegate.listProjects().map(\.id))
        let perWorkspace = Set(delegate.listWorkspaces().flatMap {
            delegate.listProjectsByWorkspace(workspaceID: $0.id).map(\.id)
        })

        #expect(flat == perWorkspace)
        #expect(flat.contains(grouped.id))
        #expect(flat.contains(loose.id))
        #expect(flat.contains(Project.homeID))
    }

    @Test("listProjects keeps the stored local project order regardless of grouping")
    func listProjectsPreservesLocalOrder() {
        let first = localProject(name: "First")
        let second = localProject(name: "Second")
        let third = localProject(name: "Third")
        let group = ProjectGroup(name: "Frontend", projectIDs: [second.id])
        let delegate = makeDelegate(projects: [first, second, third], groups: [group])

        let localIDs = delegate.listProjects()
            .filter { $0.workspaceKind == .local }
            .map(\.id)

        #expect(localIDs == [Project.homeID, first.id, second.id, third.id])
    }

    @Test("unknown workspace id returns no projects")
    func unknownWorkspaceIsEmpty() {
        let delegate = makeDelegate(projects: [localProject(name: "Alpha")], groups: [])

        #expect(delegate.listProjectsByWorkspace(workspaceID: UUID()).isEmpty)
    }

    private func localProject(name: String) -> Project {
        Project(id: UUID(), name: name, path: "/tmp/\(name)", sortOrder: 0)
    }

    private func makeDelegate(projects: [Project], groups: [ProjectGroup]) -> RemoteServerDelegate {
        let projectStore = ProjectStore(persistence: InMemoryProjectPersistence(initial: projects))
        let worktreeStore = WorktreeStore(
            persistence: InMemoryWorktreePersistence(),
            projects: projectStore.projects
        )
        let appState = AppState(
            selectionStore: InMemorySelectionStore(),
            terminalViews: NoopTerminalViewRemoving(),
            workspacePersistence: InMemoryWorkspacePersistence()
        )
        let projectGroupStore = ProjectGroupStore(
            persistence: InMemoryGroupPersistence(initial: groups),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        return RemoteServerDelegate(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }
}

private final class InMemoryProjectPersistence: ProjectPersisting {
    private var projects: [Project]
    init(initial: [Project]) { projects = initial }
    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
}

private final class InMemoryWorktreePersistence: WorktreePersisting {
    private var storage: [UUID: [Worktree]] = [:]
    func loadWorktrees(projectID: UUID) throws -> [Worktree] { storage[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { storage[projectID] = worktrees }
    func removeWorktrees(projectID: UUID) throws { storage.removeValue(forKey: projectID) }
}

private final class InMemoryGroupPersistence: ProjectGroupPersisting {
    private var groups: [ProjectGroup]
    private var activeGroupID: UUID?
    init(initial: [ProjectGroup]) { groups = initial }
    func loadProjectGroups() throws -> [ProjectGroup] { groups }
    func saveProjectGroups(_ groups: [ProjectGroup]) throws { self.groups = groups }
    func loadActiveGroupID() -> UUID? { activeGroupID }
    func saveActiveGroupID(_ id: UUID?) { activeGroupID = id }
}

private final class InMemoryWorkspacePersistence: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws { snapshots = workspaces }
}

@MainActor
private final class InMemorySelectionStore: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class NoopTerminalViewRemoving: TerminalViewRemoving {
    func removeView(for paneID: UUID) {}
    func needsConfirmQuit(for paneID: UUID) -> Bool { false }
}
