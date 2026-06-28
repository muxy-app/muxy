import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI project deletion permissions")
struct MuxyAPIProjectDeletePermissionTests {
    @Test("projects.delete requires projects:delete")
    func deleteRequiresProjectsDelete() {
        #expect(MuxyAPI.Permissions.required(for: "projects.delete") == .projectsDelete)
    }

    @Test("projects.delete is a known verb")
    func deleteIsKnownVerb() {
        #expect(MuxyAPI.Permissions.verbNames.contains("projects.delete"))
    }

    @Test("delete consent defaults to remembering the project name")
    func deleteConsentRemembersProjectName() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .projectsDelete,
            payload: .project(name: "Repo", path: "/tmp/repo")
        )
        #expect(match == .projectNameEquals("Repo"))
    }
}

@Suite("MuxyAPI project deletion routing")
@MainActor
struct MuxyAPIProjectDeleteRoutingTests {
    @Test("home project cannot be deleted")
    func homeProjectRejected() async {
        let env = makeEnvironment(projects: [])
        let result = await MuxyAPI.Projects.delete(
            identifier: Project.homeID.uuidString,
            context: env.context
        )
        guard case .failure(.invalidArguments) = result else {
            Issue.record("expected invalidArguments for the home project")
            return
        }
    }

    @Test("unknown project is rejected")
    func unknownProjectRejected() async {
        let env = makeEnvironment(projects: [])
        let result = await MuxyAPI.Projects.delete(
            identifier: "does-not-exist",
            context: env.context
        )
        guard case .failure(.projectNotFound) = result else {
            Issue.record("expected projectNotFound for an unknown identifier")
            return
        }
    }

    @Test("ProjectRemovalService removes a local project from the stores")
    func removalServiceRemovesLocalProject() async throws {
        let project = Project(name: "Repo", path: "/tmp/muxy-delete-test-\(UUID().uuidString)")
        let env = makeEnvironment(projects: [project])
        #expect(env.projectStore.storedProjects.contains { $0.id == project.id })

        try await ProjectRemovalService.remove(
            project,
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        #expect(env.projectStore.storedProjects.contains { $0.id == project.id } == false)
    }

    private struct Environment {
        let appState: AppState
        let projectStore: ProjectStore
        let worktreeStore: WorktreeStore
        let projectGroupStore: ProjectGroupStore

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

    private func makeEnvironment(projects: [Project]) -> Environment {
        let projectStore = ProjectStore(persistence: ProjectDeletePersistenceStub(initial: projects))
        let worktreeStore = WorktreeStore(
            persistence: WorktreeDeletePersistenceStub(),
            projects: projects
        )
        let appState = AppState(
            selectionStore: ProjectDeleteSelectionStoreStub(),
            terminalViews: ProjectDeleteTerminalViewRemovingStub(),
            workspacePersistence: ProjectDeleteWorkspacePersistenceStub()
        )
        let projectGroupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        return Environment(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }
}

private final class ProjectDeletePersistenceStub: ProjectPersisting {
    var projects: [Project]
    init(initial: [Project]) { projects = initial }
    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
}

private final class WorktreeDeletePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]] = [:]
    func loadWorktrees(projectID: UUID) throws -> [Worktree] { storage[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { storage[projectID] = worktrees }
    func removeWorktrees(projectID: UUID) throws { storage.removeValue(forKey: projectID) }
}

private final class ProjectDeleteWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class ProjectDeleteSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class ProjectDeleteTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
