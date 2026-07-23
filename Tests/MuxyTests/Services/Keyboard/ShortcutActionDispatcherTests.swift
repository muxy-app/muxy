import Foundation
import Testing

@testable import Muxy

@Suite("ShortcutActionDispatcher")
@MainActor
struct ShortcutActionDispatcherTests {
    @Test("create worktree posts request notification")
    func createWorktreePostsRequestNotification() {
        let project = Project(name: "App", path: "/tmp/app")
        let center = NotificationCenter()
        let capture = NotificationCapture()
        let token = center.addObserver(forName: .createWorktreeRequested, object: nil, queue: nil) { notification in
            capture.record(notification.name)
        }
        defer { center.removeObserver(token) }

        let performed = makeDispatcher(notificationCenter: center).perform(.createWorktree, activeProject: project)

        #expect(performed)
        #expect(capture.contains(.createWorktreeRequested))
    }

    @Test("remove current worktree posts request notification")
    func removeCurrentWorktreePostsRequestNotification() {
        let project = Project(name: "App", path: "/tmp/app")
        let center = NotificationCenter()
        let capture = NotificationCapture()
        let token = center.addObserver(forName: .removeCurrentWorktreeRequested, object: nil, queue: nil) { notification in
            capture.record(notification.name)
        }
        defer { center.removeObserver(token) }

        let performed = makeDispatcher(notificationCenter: center).perform(.removeCurrentWorktree, activeProject: project)

        #expect(performed)
        #expect(capture.contains(.removeCurrentWorktreeRequested))
    }

    @Test("recently removed projects opens its omnibox scope")
    func recentlyRemovedProjectsOpensOmniboxScope() {
        let center = NotificationCenter()
        let capture = NotificationCapture()
        let token = center.addObserver(forName: .terminalOmnibox, object: nil, queue: nil) { notification in
            capture.record(notification)
        }
        defer { center.removeObserver(token) }

        let performed = makeDispatcher(notificationCenter: center).perform(
            .recentlyRemovedProjects,
            activeProject: nil
        )

        #expect(performed)
        #expect(capture.contains(.terminalOmnibox))
        #expect(capture.containsLaunchScope(.recentlyRemovedProjects))
    }

    @Test("worktree requests return false without an active project")
    func worktreeRequestsReturnFalseWithoutActiveProject() {
        let dispatcher = makeDispatcher(notificationCenter: NotificationCenter())

        #expect(!dispatcher.perform(.createWorktree, activeProject: nil))
        #expect(!dispatcher.perform(.removeCurrentWorktree, activeProject: nil))
    }

    @Test("move pane actions require an active project")
    func movePaneActionsRequireActiveProject() {
        let dispatcher = makeDispatcher(notificationCenter: NotificationCenter())
        let actions: [ShortcutAction] = [.movePaneLeft, .movePaneRight, .movePaneUp, .movePaneDown]

        for action in actions {
            #expect(!dispatcher.perform(action, activeProject: nil))
        }
    }

    private func makeDispatcher(notificationCenter: NotificationCenter) -> ShortcutActionDispatcher {
        let projectStore = ProjectStore(persistence: DispatcherProjectPersistenceStub())
        let worktreeStore = WorktreeStore(persistence: DispatcherWorktreePersistenceStub(), projects: [])
        let appState = AppState(
            selectionStore: DispatcherSelectionStoreStub(),
            terminalViews: DispatcherTerminalViewRemovingStub(),
            workspacePersistence: DispatcherWorkspacePersistenceStub()
        )
        let projectGroupStore = ProjectGroupStore(
            persistence: DispatcherProjectGroupPersistenceStub(),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        return ShortcutActionDispatcher(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            ghostty: GhosttyService.shared,
            notificationCenter: notificationCenter
        )
    }
}

private final class NotificationCapture: @unchecked Sendable {
    private let lock = NSLock()
    private var names: [Notification.Name] = []
    private var launchScopes: [String] = []

    func record(_ name: Notification.Name) {
        lock.lock()
        names.append(name)
        lock.unlock()
    }

    func record(_ notification: Notification) {
        lock.lock()
        names.append(notification.name)
        if let launchScope = notification.userInfo?["launchScope"] as? String {
            launchScopes.append(launchScope)
        }
        lock.unlock()
    }

    func contains(_ name: Notification.Name) -> Bool {
        lock.lock()
        let result = names.contains(name)
        lock.unlock()
        return result
    }

    func containsLaunchScope(_ scope: TerminalOmniboxLaunchScope) -> Bool {
        lock.lock()
        let result = launchScopes.contains(scope.rawValue)
        lock.unlock()
        return result
    }
}

private final class DispatcherProjectPersistenceStub: ProjectPersisting {
    func loadProjects() throws -> [Project] { [] }
    func saveProjects(_: [Project]) throws {}
}

private final class DispatcherWorktreePersistenceStub: WorktreePersisting {
    func loadWorktrees(projectID _: UUID) throws -> [Worktree] { [] }
    func saveWorktrees(_: [Worktree], projectID _: UUID) throws {}
    func removeWorktrees(projectID _: UUID) throws {}
}

private final class DispatcherProjectGroupPersistenceStub: ProjectGroupPersisting {
    func loadProjectGroups() throws -> [ProjectGroup] { [] }
    func saveProjectGroups(_: [ProjectGroup]) throws {}
    func loadActiveGroupID() -> UUID? { nil }
    func saveActiveGroupID(_: UUID?) {}
}

private final class DispatcherWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class DispatcherSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class DispatcherTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
