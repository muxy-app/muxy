import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("RemoteServerDelegate files")
@MainActor
struct RemoteServerDelegateFilesTests {
    @Test("write, list, read, rename, move, and delete work end to end on a real worktree")
    func fileLifecycle() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let project = Project(id: UUID(), name: "Demo", path: root, sortOrder: 0)
        let delegate = makeDelegate(projects: [project])

        _ = try await delegate.filesMkdir(projectID: project.id, path: "notes")
        let written = try await delegate.filesWrite(
            projectID: project.id,
            path: "notes/todo.md",
            contents: "# Todo\n",
            encoding: .utf8
        )
        #expect(written == "notes/todo.md")

        let entries = try await delegate.filesList(projectID: project.id, path: "notes")
        #expect(entries.map(\.name) == ["todo.md"])
        #expect(entries.first?.path == "notes/todo.md")
        #expect(entries.first?.isDirectory == false)

        let content = try await delegate.filesRead(projectID: project.id, path: "notes/todo.md", encoding: .utf8)
        #expect(content.content == "# Todo\n")
        #expect(content.encoding == .utf8)

        let stat = try await delegate.filesStat(projectID: project.id, path: "notes/todo.md")
        #expect(stat.name == "todo.md")
        #expect(stat.size == 7)

        let renamed = try await delegate.filesRename(projectID: project.id, path: "notes/todo.md", newName: "done.md")
        #expect(renamed == "notes/done.md")

        _ = try await delegate.filesMkdir(projectID: project.id, path: "archive")
        let moved = try await delegate.filesMove(projectID: project.id, paths: ["notes/done.md"], into: "archive")
        #expect(moved == ["archive/done.md"])

        try await delegate.filesDelete(projectID: project.id, paths: ["archive/done.md"])
        #expect(!FileManager.default.fileExists(atPath: root + "/archive/done.md"))
    }

    @Test("binary content round-trips through base64")
    func binaryRoundTrip() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let project = Project(id: UUID(), name: "Demo", path: root, sortOrder: 0)
        let delegate = makeDelegate(projects: [project])
        let bytes = Data([0x89, 0x50, 0x4E, 0x47, 0x00, 0xFF])

        _ = try await delegate.filesWrite(
            projectID: project.id,
            path: "logo.png",
            contents: bytes.base64EncodedString(),
            encoding: .base64
        )

        let content = try await delegate.filesRead(projectID: project.id, path: "logo.png", encoding: .base64)
        #expect(Data(base64Encoded: content.content) == bytes)
        #expect(content.size == bytes.count)
    }

    @Test("an unknown project is a 404")
    func unknownProjectIsNotFound() async throws {
        let delegate = makeDelegate(projects: [])

        do {
            _ = try await delegate.filesList(projectID: UUID(), path: ".")
            Issue.record("expected an unknown project to throw")
        } catch let error as MuxyError {
            #expect(error.code == 404)
        }
    }

    @Test("a path escaping the worktree root is a 403")
    func escapeIsForbidden() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let project = Project(id: UUID(), name: "Demo", path: root, sortOrder: 0)
        let delegate = makeDelegate(projects: [project])

        do {
            _ = try await delegate.filesRead(projectID: project.id, path: "../escape.txt", encoding: .utf8)
            Issue.record("expected an escaping path to throw")
        } catch let error as MuxyError {
            #expect(error.code == 403)
            #expect(error.message == "path '../escape.txt' escapes the workspace root")
        }
    }

    @Test("a filesystem failure surfaces as 500 with the underlying message")
    func filesystemFailureIsInternal() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let project = Project(id: UUID(), name: "Demo", path: root, sortOrder: 0)
        let delegate = makeDelegate(projects: [project])

        do {
            _ = try await delegate.filesStat(projectID: project.id, path: "missing.txt")
            Issue.record("expected stat of a missing file to throw")
        } catch let error as MuxyError {
            #expect(error.code == 500)
        }
    }

    @Test("listing a missing path is a 500 instead of an empty directory")
    func missingListTargetIsInternal() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let project = Project(id: UUID(), name: "Demo", path: root, sortOrder: 0)
        let delegate = makeDelegate(projects: [project])

        do {
            _ = try await delegate.filesList(projectID: project.id, path: "missing")
            Issue.record("expected list of a missing directory to throw")
        } catch let error as MuxyError {
            #expect(error.code == 500)
        }
    }

    private func makeTempDir() throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("RemoteServerDelegateFilesTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.resolvingSymlinksInPath().path
    }

    private func makeDelegate(projects: [Project]) -> RemoteServerDelegate {
        let projectStore = ProjectStore(persistence: FilesProjectPersistenceStub(initial: projects))
        let worktreeStore = WorktreeStore(
            persistence: FilesWorktreePersistenceStub(),
            projects: projectStore.projects
        )
        let appState = AppState(
            selectionStore: FilesSelectionStoreStub(),
            terminalViews: FilesTerminalViewRemovingStub(),
            workspacePersistence: FilesWorkspacePersistenceStub()
        )
        let projectGroupStore = ProjectGroupStore(
            persistence: FilesGroupPersistenceStub(),
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

@Suite("RemoteServerDelegate file change paths")
struct RemoteServerDelegateFileChangeTests {
    @Test("absolute paths become root-relative, git internals are dropped, and order is stable")
    func relativePathsFilterGitInternals() {
        let paths = RemoteServerDelegate.workspaceRelativePaths(
            [
                "/repo/src/b.swift",
                "/repo/.git/HEAD",
                "/repo/src/a.swift",
                "/repo/.gitignore",
            ],
            root: "/repo"
        )

        #expect(paths == [".gitignore", "src/a.swift", "src/b.swift"])
    }

    @Test("paths outside the root are dropped, not degraded to a bare filename")
    func relativePathsDropOutsiders() {
        let paths = RemoteServerDelegate.workspaceRelativePaths(
            ["/elsewhere/secret.txt", "/repo", "/repo/kept.txt", "/repository/sibling.txt"],
            root: "/repo"
        )

        #expect(paths == ["kept.txt"])
    }

    @Test("a trailing slash on the root does not leak into relative paths")
    func relativePathsTolerateTrailingSlash() {
        #expect(RemoteServerDelegate.workspaceRelativePaths(["/repo/a.txt"], root: "/repo/") == ["a.txt"])
    }

    @Test("a batch beyond the cap is truncated and flagged")
    func batchIsCapped() {
        let paths = (0 ..< (FileChangedEventDTO.pathLimit + 10)).map { "file-\($0).txt" }

        let dto = FileChangedEventDTO.capped(projectID: UUID(), worktreeID: UUID(), paths: paths)

        #expect(dto.paths.count == FileChangedEventDTO.pathLimit)
        #expect(dto.truncated)
    }

    @Test("a batch within the cap is passed through untruncated")
    func smallBatchIsIntact() {
        let worktreeID = UUID()

        let dto = FileChangedEventDTO.capped(projectID: UUID(), worktreeID: worktreeID, paths: ["a.txt", "b.txt"])

        #expect(dto.paths == ["a.txt", "b.txt"])
        #expect(!dto.truncated)
        #expect(dto.worktreeID == worktreeID)
    }

    @Test("the change payload survives a round trip through the notification")
    func changeRoundTripsThroughNotification() {
        let projectID = UUID()
        let worktreeID = UUID()
        let sent = WorkspaceFilesChange(projectID: projectID, worktreeID: worktreeID, root: "/repo", paths: ["a.txt"])

        let received = WorkspaceFilesChange(Notification(
            name: .workspaceFilesDidChange,
            object: nil,
            userInfo: [
                "projectID": sent.projectID,
                "worktreeID": worktreeID,
                "root": sent.root,
                "paths": sent.paths,
            ]
        ))

        #expect(received?.projectID == projectID)
        #expect(received?.worktreeID == worktreeID)
        #expect(received?.paths == ["a.txt"])
    }

    @Test("a change without a worktree still decodes")
    func changeDecodesWithoutWorktree() {
        let received = WorkspaceFilesChange(Notification(
            name: .workspaceFilesDidChange,
            object: nil,
            userInfo: ["projectID": UUID(), "root": "/repo", "paths": ["a.txt"]]
        ))

        #expect(received != nil)
        #expect(received?.worktreeID == nil)
    }
}

private final class FilesProjectPersistenceStub: ProjectPersisting {
    private var projects: [Project]
    init(initial: [Project]) { projects = initial }
    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
}

private final class FilesWorktreePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]] = [:]
    func loadWorktrees(projectID: UUID) throws -> [Worktree] { storage[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { storage[projectID] = worktrees }
    func removeWorktrees(projectID: UUID) throws { storage.removeValue(forKey: projectID) }
}

private final class FilesGroupPersistenceStub: ProjectGroupPersisting {
    private var groups: [ProjectGroup] = []
    private var activeGroupID: UUID?
    func loadProjectGroups() throws -> [ProjectGroup] { groups }
    func saveProjectGroups(_ groups: [ProjectGroup]) throws { self.groups = groups }
    func loadActiveGroupID() -> UUID? { activeGroupID }
    func saveActiveGroupID(_ id: UUID?) { activeGroupID = id }
}

private final class FilesWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class FilesSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class FilesTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
