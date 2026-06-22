import Darwin
import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("NotificationSocketServer notifications", .serialized)
struct NotificationSocketServerNotificationTests {
    @Test("socket notification with pane id is stored")
    func socketNotificationWithPaneIDIsStored() async throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let paneID = try fixture.activePaneID()
        try await sendAndWaitForNotification("custom|\(paneID.uuidString)|Manual test|Hello")

        let notification = try #require(NotificationStore.shared.notifications.first)
        #expect(notification.paneID == paneID)
        #expect(notification.source == .socket)
        #expect(notification.title == "Manual test")
        #expect(notification.body == "Hello")
    }

    @Test("socket notification with empty pane id uses active pane")
    func socketNotificationWithEmptyPaneIDUsesActivePane() async throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let paneID = try fixture.activePaneID()
        try await sendAndWaitForNotification("custom||Manual test|Hello")

        let notification = try #require(NotificationStore.shared.notifications.first)
        #expect(notification.paneID == paneID)
        #expect(notification.source == .socket)
        #expect(notification.title == "Manual test")
        #expect(notification.body == "Hello")
    }

    private func sendAndWaitForNotification(_ payload: String) async throws {
        let initialCount = NotificationStore.shared.notifications.count
        NotificationSocketServer.shared.start()
        await NotificationSocketServer.shared.awaitReady()
        try Self.send(payload + "\n")
        try await Self.waitUntil {
            NotificationStore.shared.notifications.count > initialCount
        }
    }

    private static func send(_ payload: String) throws {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw POSIXError(.EMFILE) }
        defer { close(descriptor) }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let path = NotificationSocketServer.socketPath
        let capacity = MemoryLayout.size(ofValue: addr.sun_path)
        guard path.utf8.count < capacity else { throw POSIXError(.ENAMETOOLONG) }
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            let bound = ptr.withMemoryRebound(to: CChar.self, capacity: capacity) { $0 }
            _ = path.withCString { strncpy(bound, $0, capacity - 1) }
        }

        let result = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .ECONNREFUSED) }

        let data = Data(payload.utf8)
        try data.withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return }
            let written = Darwin.write(descriptor, baseAddress, buffer.count)
            guard written == buffer.count else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        }
    }

    private static func waitUntil(_ predicate: @escaping @MainActor () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(2)
        while Date() < deadline {
            if predicate() { return }
            try await Task.sleep(for: .milliseconds(20))
        }
        Issue.record("Timed out waiting for notification")
    }

    @MainActor
    private struct Fixture {
        let rootURL: URL
        let appState: AppState
        let worktreeStore: WorktreeStore

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("NotificationSocketServerTests-\(UUID().uuidString)", isDirectory: true)
            try FileManager.default.createDirectory(at: rootURL, withIntermediateDirectories: true)

            let project = Project(name: "Test", path: rootURL.path)
            let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
            appState = AppState(
                selectionStore: SelectionStoreStub(),
                terminalViews: TerminalViewRemovingStub(),
                workspacePersistence: WorkspacePersistenceStub()
            )
            worktreeStore = WorktreeStore(
                persistence: WorktreePersistenceStub(worktrees: [project.id: [worktree]]),
                projects: [project]
            )

            let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
            let area = TabArea(projectPath: project.path)
            appState.activeProjectID = project.id
            appState.activeWorktreeID[project.id] = worktree.id
            appState.workspaceRoots[key] = .tabArea(area)
            appState.focusedAreaID[key] = area.id

            NotificationStore.shared.appState = appState
            NotificationStore.shared.worktreeStore = worktreeStore
            NotificationStore.shared.clear()
            ToastState.shared.dismiss()
        }

        func activePaneID() throws -> UUID {
            try #require(NotificationNavigator.activePaneID(appState: appState))
        }

        func cleanUp() {
            NotificationStore.shared.clear()
            ToastState.shared.dismiss()
            try? FileManager.default.removeItem(at: rootURL)
        }
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

private final class WorktreePersistenceStub: WorktreePersisting {
    private var worktrees: [UUID: [Worktree]]

    init(worktrees: [UUID: [Worktree]]) {
        self.worktrees = worktrees
    }

    func loadWorktrees(projectID: UUID) throws -> [Worktree] { worktrees[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { self.worktrees[projectID] = worktrees }
    func removeWorktrees(projectID: UUID) throws { worktrees[projectID] = nil }
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
