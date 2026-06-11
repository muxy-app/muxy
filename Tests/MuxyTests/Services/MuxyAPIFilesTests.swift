import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI.Files permissions, sandbox, and DTOs")
struct MuxyAPIFilesTests {
    @Test("read verbs require files:read")
    func readVerbsRequireFilesRead() {
        for verb in ["files.list", "files.read", "files.stat"] {
            #expect(MuxyAPI.Permissions.required(for: verb) == .filesRead, "\(verb) should need files:read")
        }
    }

    @Test("write verbs require files:write")
    func writeVerbsRequireFilesWrite() {
        for verb in ["files.write", "files.mkdir", "files.rename", "files.move", "files.delete"] {
            #expect(MuxyAPI.Permissions.required(for: verb) == .filesWrite, "\(verb) should need files:write")
        }
    }

    @Test("files verbs are recognized command names")
    func filesVerbsAreKnown() {
        for verb in MuxyAPI.Permissions.filesVerbs {
            #expect(MuxyAPI.Permissions.verbNames.contains(verb), "\(verb) should be a known verb")
        }
    }

    @Test("resolve keeps in-root paths and normalizes them")
    func resolveAcceptsInRootPaths() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let resolved = MuxyAPI.Files.resolve(root: root, relativePath: "src/main.swift")
        #expect(resolved == root + "/src/main.swift")
        #expect(MuxyAPI.Files.resolve(root: root, relativePath: "") == root)
    }

    @Test("resolve rejects parent-traversal escapes")
    func resolveRejectsParentTraversal() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        #expect(MuxyAPI.Files.resolve(root: root, relativePath: "../escape.txt") == nil)
        #expect(MuxyAPI.Files.resolve(root: root, relativePath: "a/../../escape.txt") == nil)
    }

    @Test("resolve rejects symlink escapes")
    func resolveRejectsSymlinkEscape() async throws {
        let root = try makeTempDir()
        let outside = try makeTempDir()
        defer {
            try? FileManager.default.removeItem(atPath: root)
            try? FileManager.default.removeItem(atPath: outside)
        }
        let link = root + "/link"
        try FileManager.default.createSymbolicLink(atPath: link, withDestinationPath: outside)
        #expect(MuxyAPI.Files.resolve(root: root, relativePath: "link/secret.txt") == nil)
    }

    @Test("resolve rejects dangling symlinks that point outside the root")
    func resolveRejectsDanglingSymlinkEscape() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let danglingTarget = FileManager.default.temporaryDirectory
            .appendingPathComponent("MuxyAPIFilesTests-missing-\(UUID().uuidString)").path
        try FileManager.default.createSymbolicLink(atPath: root + "/evil", withDestinationPath: danglingTarget)
        #expect(MuxyAPI.Files.resolve(root: root, relativePath: "evil/secret.txt") == nil)
    }

    @Test("resolve follows in-root symlinks transparently")
    func resolveFollowsInRootSymlink() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try FileManager.default.createDirectory(atPath: root + "/real", withIntermediateDirectories: false)
        try FileManager.default.createSymbolicLink(atPath: root + "/alias", withDestinationPath: root + "/real")
        #expect(MuxyAPI.Files.resolve(root: root, relativePath: "alias/note.txt") == root + "/real/note.txt")
    }

    @Test("contained returns in-root paths and throws on escape")
    func containedGuardsAtOpTime() async throws {
        let root = try makeTempDir()
        let outside = try makeTempDir()
        defer {
            try? FileManager.default.removeItem(atPath: root)
            try? FileManager.default.removeItem(atPath: outside)
        }
        #expect(try MuxyAPI.Files.contained(root: root, relativePath: "src/main.swift") == root + "/src/main.swift")
        try FileManager.default.createSymbolicLink(atPath: root + "/link", withDestinationPath: outside)
        #expect(throws: FileSystemOperationError.self) {
            _ = try MuxyAPI.Files.contained(root: root, relativePath: "link/secret.txt")
        }
    }

    @Test("writeFile overwrites and round-trips contents")
    func writeFileRoundTrips() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let target = root + "/note.txt"
        try await FileSystemOperations.writeFile(contents: "first", atAbsolutePath: target)
        try await FileSystemOperations.writeFile(contents: "second", atAbsolutePath: target)
        let read = try String(contentsOfFile: target, encoding: .utf8)
        #expect(read == "second")
    }

    @Test("list returns sorted entries with relative paths")
    func listReturnsEntries() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try FileManager.default.createDirectory(atPath: root + "/src", withIntermediateDirectories: false)
        try "x".write(toFile: root + "/a.txt", atomically: true, encoding: .utf8)
        let entries = await FileTreeService.loadChildren(of: root, repoRoot: root)
        #expect(entries.first?.isDirectory == true)
        #expect(entries.contains { $0.relativePath == "a.txt" })
    }

    @Test("filesystem errors surface their user message, not a generic description")
    func filesystemErrorsSurfaceUserMessage() {
        let error = FileSystemOperationError.underlying("path '../x' escapes the workspace root")
        #expect(error.userMessage == "path '../x' escapes the workspace root")
        #expect(error.localizedDescription != error.userMessage)
    }

    @Test("write consent defaults to remembering the operation")
    func writeConsentRemembersOperation() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .filesWrite,
            payload: .file(operation: "delete", path: "a.txt")
        )
        #expect(match == .fileOperationEquals("delete"))
    }

    @Test("DTOs encode entry, stat, and read shapes")
    func dtoEncoding() {
        let entry = FileTreeEntry(
            name: "a.txt",
            absolutePath: "/root/a.txt",
            relativePath: "a.txt",
            isDirectory: false,
            isIgnored: true
        )
        let entryDTO = FilesDTO.entry(entry)
        #expect(entryDTO["name"] as? String == "a.txt")
        #expect(entryDTO["path"] as? String == "a.txt")
        #expect(entryDTO["isDirectory"] as? Bool == false)
        #expect(entryDTO["isIgnored"] as? Bool == true)

        let statDTO = FilesDTO.stat(MuxyAPI.Files.StatResult(
            name: "a.txt",
            relativePath: "a.txt",
            isDirectory: false,
            size: 12
        ))
        #expect(statDTO["size"] as? Int == 12)

        let readDTO = FilesDTO.readResult(MuxyAPI.Files.ReadResult(
            relativePath: "a.txt",
            content: "hello",
            size: 5
        ))
        #expect(readDTO["content"] as? String == "hello")
        #expect(readDTO["size"] as? Int == 5)
    }

    private func makeTempDir() throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("MuxyAPIFilesTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.resolvingSymlinksInPath().path
    }
}

@Suite("MuxyAPI.Files local routing through a wired context")
@MainActor
struct MuxyAPIFilesRoutingTests {
    @Test("read and list route to a local project resolved by the project group store")
    func readListRoutesToResolvedProject() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let notes = (root as NSString).appendingPathComponent("notes")
        try FileManager.default.createDirectory(atPath: notes, withIntermediateDirectories: true)
        try "hello".write(toFile: (notes as NSString).appendingPathComponent("todo.txt"), atomically: true, encoding: .utf8)

        let context = makeContext(project: Project(name: "demo", path: root))

        let read = try await unwrap(MuxyAPI.Files.read(
            projectIdentifier: "demo",
            path: "notes/todo.txt",
            context: context
        ))
        #expect(read.content == "hello")
        #expect(read.relativePath == "notes/todo.txt")

        let entries = try await unwrap(MuxyAPI.Files.list(
            projectIdentifier: "demo",
            path: "notes",
            context: context
        ))
        #expect(entries.contains { $0.name == "todo.txt" })
    }

    @Test("unknown project is rejected, not silently routed")
    func unknownProjectRejected() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let context = makeContext(project: Project(name: "demo", path: root))
        let result = await MuxyAPI.Files.read(
            projectIdentifier: "does-not-exist",
            path: "x.txt",
            context: context
        )
        guard case .failure = result else {
            Issue.record("expected failure for unknown project")
            return
        }
    }

    private func unwrap<T>(_ result: Result<T, APIError>) throws -> T {
        switch result {
        case let .success(value): value
        case let .failure(error): throw error
        }
    }

    private func makeTempDir() throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("MuxyAPIFilesRoutingTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.resolvingSymlinksInPath().path
    }

    private func makeContext(project: Project) -> MuxyAPI.Files.Context {
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub())
        projectStore.add(project)
        let worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceStub(),
            projects: [project]
        )
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let projectGroupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        return MuxyAPI.Files.Context(
            extensionID: "test",
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
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
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { storage[projectID] = worktrees }
    func removeWorktrees(projectID: UUID) throws { storage.removeValue(forKey: projectID) }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
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
