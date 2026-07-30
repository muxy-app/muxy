import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("WorkspaceFileService sandbox")
struct WorkspaceFileServiceSandboxTests {
    @Test("resolve keeps in-root paths and normalizes them")
    func resolveAcceptsInRootPaths() throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let child = WorkspaceFileService.resolve(root: root, relativePath: "src/main.swift")
        let resolvedRoot = WorkspaceFileService.resolve(root: root, relativePath: "")
        #expect(child.map { WorkspaceFileService.relative($0, root: root) } == "src/main.swift")
        #expect(resolvedRoot.map { WorkspaceFileService.relative($0, root: root) } == "")
    }

    @Test("resolve rejects parent-traversal escapes")
    func resolveRejectsParentTraversal() throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        #expect(WorkspaceFileService.resolve(root: root, relativePath: "../escape.txt") == nil)
        #expect(WorkspaceFileService.resolve(root: root, relativePath: "a/../../escape.txt") == nil)
    }

    @Test("resolve rejects symlink escapes")
    func resolveRejectsSymlinkEscape() throws {
        let root = try makeTempDir()
        let outside = try makeTempDir()
        defer {
            try? FileManager.default.removeItem(atPath: root)
            try? FileManager.default.removeItem(atPath: outside)
        }
        try FileManager.default.createSymbolicLink(atPath: root + "/link", withDestinationPath: outside)
        #expect(WorkspaceFileService.resolve(root: root, relativePath: "link/secret.txt") == nil)
    }

    @Test("resolve rejects multi-hop symlink escapes")
    func resolveRejectsMultiHopSymlinkEscape() throws {
        let root = try makeTempDir()
        let outside = try makeTempDir()
        defer {
            try? FileManager.default.removeItem(atPath: root)
            try? FileManager.default.removeItem(atPath: outside)
        }
        try FileManager.default.createSymbolicLink(atPath: root + "/a", withDestinationPath: root + "/b")
        try FileManager.default.createSymbolicLink(atPath: root + "/b", withDestinationPath: outside)
        #expect(WorkspaceFileService.resolve(root: root, relativePath: "a/secret.txt") == nil)
    }

    @Test("resolve rejects symlink cycles")
    func resolveRejectsSymlinkCycle() throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try FileManager.default.createSymbolicLink(atPath: root + "/a", withDestinationPath: root + "/b")
        try FileManager.default.createSymbolicLink(atPath: root + "/b", withDestinationPath: root + "/a")
        #expect(WorkspaceFileService.resolve(root: root, relativePath: "a/file.txt") == nil)
    }

    @Test("resolve rejects dangling symlinks that point outside the root")
    func resolveRejectsDanglingSymlinkEscape() throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let danglingTarget = FileManager.default.temporaryDirectory
            .appendingPathComponent("WorkspaceFileServiceTests-missing-\(UUID().uuidString)").path
        try FileManager.default.createSymbolicLink(atPath: root + "/evil", withDestinationPath: danglingTarget)
        #expect(WorkspaceFileService.resolve(root: root, relativePath: "evil/secret.txt") == nil)
    }

    @Test("resolve follows in-root symlinks transparently")
    func resolveFollowsInRootSymlink() throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try FileManager.default.createDirectory(atPath: root + "/real", withIntermediateDirectories: false)
        try FileManager.default.createSymbolicLink(atPath: root + "/alias", withDestinationPath: root + "/real")
        let resolved = WorkspaceFileService.resolve(root: root, relativePath: "alias/note.txt")
        #expect(resolved.map { WorkspaceFileService.relative($0, root: root) } == "real/note.txt")
    }

    @Test("resolve permits revisiting a symlink without a cycle")
    func resolvePermitsRevisitedSymlink() throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try FileManager.default.createDirectory(atPath: root + "/real", withIntermediateDirectories: false)
        try FileManager.default.createSymbolicLink(atPath: root + "/alias", withDestinationPath: root + "/real")

        let resolved = WorkspaceFileService.resolve(root: root, relativePath: "alias/../alias/note.txt")

        #expect(resolved.map { WorkspaceFileService.relative($0, root: root) } == "real/note.txt")
    }

    @Test("contained returns in-root paths and throws outsideRoot on escape")
    func containedGuardsAtOpTime() throws {
        let root = try makeTempDir()
        let outside = try makeTempDir()
        defer {
            try? FileManager.default.removeItem(atPath: root)
            try? FileManager.default.removeItem(atPath: outside)
        }
        let contained = try WorkspaceFileService.contained(root: root, relativePath: "src/main.swift")
        #expect(WorkspaceFileService.relative(contained, root: root) == "src/main.swift")
        try FileManager.default.createSymbolicLink(atPath: root + "/link", withDestinationPath: outside)
        #expect(throws: FileSystemOperationError.outsideRoot("link/secret.txt")) {
            _ = try WorkspaceFileService.contained(root: root, relativePath: "link/secret.txt")
        }
    }

    @Test("outsideRoot keeps the message the extension API documents")
    func outsideRootMessageIsStable() {
        #expect(FileSystemOperationError.outsideRoot("../x").userMessage == "path '../x' escapes the workspace root")
        #expect(FileSystemOperationError.outsideRoot("").userMessage == "path escapes the workspace root")
    }

    @Test("relative strips the root prefix and falls back to the last component")
    func relativeMapsBackToRoot() throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        #expect(WorkspaceFileService.relative(root, root: root) == "")
        #expect(WorkspaceFileService.relative(root + "/src/main.swift", root: root) == "src/main.swift")
        #expect(WorkspaceFileService.relative("/elsewhere/note.txt", root: root) == "note.txt")
    }

    @Test("relative paths beneath the filesystem root retain every component")
    func relativeMapsFromFilesystemRoot() {
        let expected = "MuxyRoot-\(UUID().uuidString)/nested/file.txt"

        #expect(WorkspaceFileService.relative("/\(expected)", root: "/") == expected)
    }

    private func makeTempDir() throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("WorkspaceFileServiceTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.resolvingSymlinksInPath().path
    }
}

@Suite("WorkspaceFileService encoding")
struct WorkspaceFileServiceEncodingTests {
    @Test("encode returns text for utf8 and base64 for binary")
    func encodeSwitchesOnEncoding() throws {
        let data = Data([0x00, 0x01, 0xFF])
        #expect(try WorkspaceFileService.encode(data, as: .base64) == data.base64EncodedString())
        #expect(try WorkspaceFileService.encode(Data("hi".utf8), as: .utf8) == "hi")
    }

    @Test("encode rejects non-UTF-8 bytes when utf8 is requested")
    func encodeRejectsInvalidUTF8() {
        #expect(throws: FileSystemOperationError.self) {
            _ = try WorkspaceFileService.encode(Data([0xFF, 0xFE]), as: .utf8)
        }
    }

    @Test("decodeBase64 tolerates wrapped output and rejects garbage")
    func decodeBase64Behaviour() throws {
        let data = Data([0x0A, 0x0B, 0x0C])
        let wrapped = data.base64EncodedString() + "\n"
        #expect(try WorkspaceFileService.decodeBase64(wrapped) == data)
        #expect(throws: FileSystemOperationError.self) {
            _ = try WorkspaceFileService.decodeBase64("not base64!!")
        }
    }
}

@Suite("WorkspaceFileService local operations")
@MainActor
struct WorkspaceFileServiceLocalTests {
    @Test("write then read round-trips UTF-8 text")
    func textRoundTrip() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }

        let written = try await WorkspaceFileService.write(
            root: root,
            path: "notes.md",
            contents: "# Todo\n",
            encoding: .utf8
        )
        #expect(written == "notes.md")

        let read = try await WorkspaceFileService.read(root: root, path: "notes.md", encoding: .utf8)
        #expect(read.content == "# Todo\n")
        #expect(read.encoding == .utf8)
        #expect(read.size == 7)
    }

    @Test("write then read round-trips binary through base64")
    func binaryRoundTrip() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }
        let bytes = Data([0x89, 0x50, 0x4E, 0x47, 0x00, 0xFF, 0x1A])

        _ = try await WorkspaceFileService.write(
            root: root,
            path: "logo.png",
            contents: bytes.base64EncodedString(),
            encoding: .base64
        )

        let read = try await WorkspaceFileService.read(root: root, path: "logo.png", encoding: .base64)
        #expect(Data(base64Encoded: read.content) == bytes)
        #expect(read.encoding == .base64)
        #expect(read.size == bytes.count)
    }

    @Test("reading binary bytes as utf8 fails instead of returning mojibake")
    func binaryAsTextFails() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }
        try Data([0xFF, 0xFE, 0x00]).write(to: URL(fileURLWithPath: root.path + "/blob.bin"))

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.read(root: root, path: "blob.bin", encoding: .utf8)
        }
    }

    @Test("reading a missing file reports sourceMissing")
    func missingReadUsesFileSystemError() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }

        await #expect(throws: FileSystemOperationError.sourceMissing(root.path + "/missing.txt")) {
            _ = try await WorkspaceFileService.read(root: root, path: "missing.txt", encoding: .utf8)
        }
    }

    @Test("writing invalid base64 is rejected before touching disk")
    func invalidBase64Rejected() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.write(
                root: root,
                path: "bad.bin",
                contents: "not base64!!",
                encoding: .base64
            )
        }
        #expect(!FileManager.default.fileExists(atPath: root.path + "/bad.bin"))
    }

    @Test("read refuses files past the size limit")
    func readSizeLimit() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }
        let exact = Data(repeating: 0x41, count: WorkspaceFileService.maxReadBytes)
        try exact.write(to: URL(fileURLWithPath: root.path + "/exact.txt"))
        let oversized = Data(repeating: 0x41, count: WorkspaceFileService.maxReadBytes + 1)
        try oversized.write(to: URL(fileURLWithPath: root.path + "/big.txt"))

        let read = try await WorkspaceFileService.read(root: root, path: "exact.txt", encoding: .base64)
        #expect(read.size == exact.count)
        #expect(Data(base64Encoded: read.content) == exact)

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.read(root: root, path: "big.txt", encoding: .base64)
        }
    }

    @Test("write refuses UTF-8 and Base64 payloads past the raw byte limit")
    func writeSizeLimit() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }
        let oversized = Data(repeating: 0x41, count: WorkspaceFileService.maxWriteBytes + 1)

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.write(
                root: root,
                path: "large.txt",
                contents: String(decoding: oversized, as: UTF8.self),
                encoding: .utf8
            )
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.write(
                root: root,
                path: "large.bin",
                contents: oversized.base64EncodedString(),
                encoding: .base64
            )
        }
        #expect(!FileManager.default.fileExists(atPath: root.path + "/large.txt"))
        #expect(!FileManager.default.fileExists(atPath: root.path + "/large.bin"))
    }

    @Test("list, stat, mkdir, rename, and move operate on relative paths")
    func directoryOperations() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }

        let created = try await WorkspaceFileService.mkdir(root: root, path: "notes")
        #expect(created == "notes")

        _ = try await WorkspaceFileService.write(root: root, path: "todo.md", contents: "x", encoding: .utf8)
        let renamed = try await WorkspaceFileService.rename(root: root, path: "todo.md", newName: "done.md")
        #expect(renamed == "done.md")

        let moved = try await WorkspaceFileService.move(root: root, paths: ["done.md"], into: "notes")
        #expect(moved == ["notes/done.md"])

        let entries = try await WorkspaceFileService.list(root: root, path: "notes")
        #expect(entries.map(\.name) == ["done.md"])

        let stat = try await WorkspaceFileService.stat(root: root, path: "notes/done.md")
        #expect(stat.isDirectory == false)
        #expect(stat.relativePath == "notes/done.md")
    }

    @Test("root listing and stat are allowed and stat stays root-relative")
    func rootReadsAreAllowed() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }
        try "x".write(toFile: root.path + "/file.txt", atomically: true, encoding: .utf8)

        let entries = try await WorkspaceFileService.list(root: root, path: "")
        let stat = try await WorkspaceFileService.stat(root: root, path: ".")

        #expect(entries.map(\.name) == ["file.txt"])
        #expect(stat.relativePath == "")
        #expect(stat.isDirectory)
    }

    @Test("root mutations are rejected while moving into the root remains allowed")
    func rootMutationsAreRejected() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }
        try FileManager.default.createDirectory(
            atPath: root.path + "/nested",
            withIntermediateDirectories: false
        )
        try "x".write(toFile: root.path + "/nested/file.txt", atomically: true, encoding: .utf8)

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.write(
                root: root,
                path: "",
                contents: "x",
                encoding: .utf8
            )
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.mkdir(root: root, path: ".")
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.rename(root: root, path: "", newName: "renamed")
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.move(root: root, paths: ["."], into: "nested")
        }
        await #expect(throws: FileSystemOperationError.self) {
            try await WorkspaceFileService.delete(root: root, paths: [""])
        }

        let moved = try await WorkspaceFileService.move(
            root: root,
            paths: ["nested/file.txt"],
            into: ""
        )
        #expect(moved == ["file.txt"])
        #expect(FileManager.default.fileExists(atPath: root.path + "/file.txt"))
    }

    @Test("multi-hop symlink escapes reject reads and writes")
    func multiHopSymlinkOperationsAreRejected() async throws {
        let root = try makeRoot()
        let outside = try makeTempDir()
        defer {
            try? FileManager.default.removeItem(atPath: root.path)
            try? FileManager.default.removeItem(atPath: outside)
        }
        try "secret".write(toFile: outside + "/secret.txt", atomically: true, encoding: .utf8)
        try FileManager.default.createSymbolicLink(
            atPath: root.path + "/a",
            withDestinationPath: root.path + "/b"
        )
        try FileManager.default.createSymbolicLink(atPath: root.path + "/b", withDestinationPath: outside)

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.read(root: root, path: "a/secret.txt", encoding: .utf8)
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.write(
                root: root,
                path: "a/created.txt",
                contents: "nope",
                encoding: .utf8
            )
        }
        #expect(!FileManager.default.fileExists(atPath: outside + "/created.txt"))
    }

    @Test("listing a missing path or a file fails")
    func invalidListTargetsFail() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }
        try "x".write(toFile: root.path + "/file.txt", atomically: true, encoding: .utf8)

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.list(root: root, path: "missing")
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.list(root: root, path: "file.txt")
        }
    }

    @Test("operations outside the root are rejected")
    func escapesRejected() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }

        await #expect(throws: FileSystemOperationError.outsideRoot("../escape.txt")) {
            _ = try await WorkspaceFileService.write(
                root: root,
                path: "../escape.txt",
                contents: "nope",
                encoding: .utf8
            )
        }
    }

    @Test("same-name rename rejects a missing source")
    func sameNameRenameRequiresSource() async throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(atPath: root.path) }

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.rename(
                root: root,
                path: "missing.txt",
                newName: "missing.txt"
            )
        }
    }

    private func makeRoot() throws -> WorkspaceFileService.Root {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("WorkspaceFileServiceLocalTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return WorkspaceFileService.Root(path: dir.resolvingSymlinksInPath().path, workspaceContext: .local)
    }

    private func makeTempDir() throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("WorkspaceFileServiceLocalTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.resolvingSymlinksInPath().path
    }
}

@Suite("RemoteFileService operations")
struct RemoteFileServiceTests {
    @Test("multi-hop and dangling external symlinks reject reads and writes")
    func symlinkEscapesAreRejected() async throws {
        let root = try makeTempDir()
        let outside = try makeTempDir()
        let newlineOutside = root + "\n"
        try FileManager.default.createDirectory(atPath: newlineOutside, withIntermediateDirectories: false)
        defer {
            try? FileManager.default.removeItem(atPath: root)
            try? FileManager.default.removeItem(atPath: outside)
            try? FileManager.default.removeItem(atPath: newlineOutside)
        }
        try "secret".write(toFile: outside + "/secret.txt", atomically: true, encoding: .utf8)
        try "secret".write(toFile: newlineOutside + "/secret.txt", atomically: true, encoding: .utf8)
        try FileManager.default.createSymbolicLink(atPath: root + "/a", withDestinationPath: root + "/b")
        try FileManager.default.createSymbolicLink(atPath: root + "/b", withDestinationPath: outside)
        try FileManager.default.createSymbolicLink(
            atPath: root + "/dangling",
            withDestinationPath: outside + "/created.txt"
        )
        try FileManager.default.createSymbolicLink(
            atPath: root + "/newline",
            withDestinationPath: newlineOutside
        )
        try FileManager.default.createSymbolicLink(
            atPath: root + "/same-name",
            withDestinationPath: outside + "/secret.txt"
        )
        let service = makeService()

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await service.read(
                root: root,
                relativePath: "a/secret.txt",
                maxBytes: WorkspaceFileService.maxReadBytes,
                encoding: .utf8
            )
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await service.read(
                root: root,
                relativePath: "newline/secret.txt",
                maxBytes: WorkspaceFileService.maxReadBytes,
                encoding: .utf8
            )
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await service.write(
                root: root,
                relativePath: "dangling",
                data: Data("nope".utf8),
                maxBytes: WorkspaceFileService.maxWriteBytes
            )
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await service.rename(
                root: root,
                relativePath: "same-name",
                newName: "same-name"
            )
        }
        #expect(!FileManager.default.fileExists(atPath: outside + "/created.txt"))
    }

    @Test("invalid UTF-8 remote reads fail")
    func invalidUTF8Fails() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try Data([0xFF, 0xFE]).write(to: URL(fileURLWithPath: root + "/invalid.bin"))

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await makeService().read(
                root: root,
                relativePath: "invalid.bin",
                maxBytes: WorkspaceFileService.maxReadBytes,
                encoding: .utf8
            )
        }
    }

    @Test("remote reads enforce the byte limit on the returned payload")
    func readSizeLimit() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try Data(repeating: 0x41, count: 5).write(to: URL(fileURLWithPath: root + "/large.bin"))

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await makeService().read(
                root: root,
                relativePath: "large.bin",
                maxBytes: 4,
                encoding: .base64
            )
        }
    }

    @Test("remote writes send binary bytes directly and replace atomically")
    func binaryWriteRoundTrip() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let bytes = Data([0x00, 0xFF, 0x0A, 0x7F])

        let path = try await makeService().write(
            root: root,
            relativePath: "binary.dat",
            data: bytes,
            maxBytes: WorkspaceFileService.maxWriteBytes
        )

        #expect(path == "binary.dat")
        #expect(try Data(contentsOf: URL(fileURLWithPath: root + "/binary.dat")) == bytes)
        #expect(try FileManager.default.contentsOfDirectory(atPath: root) == ["binary.dat"])
    }

    @Test("remote writes reject directory targets without creating hidden files")
    func directoryWriteFails() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let directory = root + "/assets"
        try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: false)

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await makeService().write(
                root: root,
                relativePath: "assets",
                data: Data("content".utf8),
                maxBytes: WorkspaceFileService.maxWriteBytes
            )
        }

        #expect(try FileManager.default.contentsOfDirectory(atPath: directory).isEmpty)
    }

    @Test("remote writes preserve existing modes and honor the process umask for new files")
    func writePreservesModes() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let executable = root + "/tool"
        let remoteNew = root + "/remote.txt"
        let localNew = root + "/local.txt"
        try "old".write(toFile: executable, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: executable)
        let service = makeService()

        _ = try await service.write(
            root: root,
            relativePath: "tool",
            data: Data("new".utf8),
            maxBytes: WorkspaceFileService.maxWriteBytes
        )
        _ = try await service.write(
            root: root,
            relativePath: "remote.txt",
            data: Data("new".utf8),
            maxBytes: WorkspaceFileService.maxWriteBytes
        )
        try Data("new".utf8).write(to: URL(fileURLWithPath: localNew), options: .atomic)

        let executableMode = try posixMode(executable)
        let remoteMode = try posixMode(remoteNew)
        let localMode = try posixMode(localNew)
        #expect(executableMode == 0o755)
        #expect(remoteMode == localMode)
    }

    @Test("remote writes validate mode probe output before falling back")
    func writeModeFallback() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }

        for status in [0, 1] {
            let name = "tool-\(status)"
            let executable = root + "/\(name)"
            try "old".write(toFile: executable, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: executable)
            let statShim = "stat() { if [ \"$1\" = '-f' ]; then printf 'filesystem output\\n'; "
                + "return \(status); fi; /usr/bin/stat -f '%Lp' \"$3\"; }; "

            _ = try await makeService(commandPrefix: statShim).write(
                root: root,
                relativePath: name,
                data: Data("new".utf8),
                maxBytes: WorkspaceFileService.maxWriteBytes
            )

            #expect(try posixMode(executable) == 0o755)
        }
    }

    @Test("an incomplete remote write leaves existing content unchanged")
    func incompleteWritePreservesTarget() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try "original".write(toFile: root + "/note.txt", atomically: true, encoding: .utf8)
        let replacement = Data("replacement".utf8)
        let service = makeService { input in
            input.map { Data($0.dropLast()) }
        }

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await service.write(
                root: root,
                relativePath: "note.txt",
                data: replacement,
                maxBytes: WorkspaceFileService.maxWriteBytes
            )
        }

        #expect(try String(contentsOfFile: root + "/note.txt", encoding: .utf8) == "original")
        #expect(try FileManager.default.contentsOfDirectory(atPath: root) == ["note.txt"])
    }

    @Test("rename rejects existing files and directories without overwriting")
    func renameCollisionsFail() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try "source".write(toFile: root + "/source.txt", atomically: true, encoding: .utf8)
        try "existing".write(toFile: root + "/existing.txt", atomically: true, encoding: .utf8)
        try FileManager.default.createDirectory(atPath: root + "/existing", withIntermediateDirectories: false)
        let service = makeService()

        await #expect(throws: FileSystemOperationError.destinationExists(root + "/existing.txt")) {
            _ = try await service.rename(root: root, relativePath: "source.txt", newName: "existing.txt")
        }
        await #expect(throws: FileSystemOperationError.destinationExists(root + "/existing")) {
            _ = try await service.rename(root: root, relativePath: "source.txt", newName: "existing")
        }
        await #expect(throws: FileSystemOperationError.sourceMissing(root + "/missing.txt")) {
            _ = try await service.rename(root: root, relativePath: "missing.txt", newName: "missing.txt")
        }
        #expect(try String(contentsOfFile: root + "/source.txt", encoding: .utf8) == "source")
        #expect(try String(contentsOfFile: root + "/existing.txt", encoding: .utf8) == "existing")
    }

    @Test("move uniquifies existing file and directory destinations")
    func moveCollisionsAreUniquified() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try FileManager.default.createDirectory(atPath: root + "/first", withIntermediateDirectories: false)
        try FileManager.default.createDirectory(atPath: root + "/second", withIntermediateDirectories: false)
        try FileManager.default.createDirectory(atPath: root + "/archive", withIntermediateDirectories: false)
        try "first".write(toFile: root + "/first/item.txt", atomically: true, encoding: .utf8)
        try "second".write(toFile: root + "/second/item", atomically: true, encoding: .utf8)
        try "existing".write(toFile: root + "/archive/item.txt", atomically: true, encoding: .utf8)
        try FileManager.default.createDirectory(atPath: root + "/archive/item", withIntermediateDirectories: false)
        let service = makeService()

        let fileMove = try await service.move(root: root, paths: ["first/item.txt"], into: "archive")
        let directoryMove = try await service.move(root: root, paths: ["second/item"], into: "archive")

        #expect(fileMove == ["archive/item 2.txt"])
        #expect(directoryMove == ["archive/item 2"])
        #expect(try String(contentsOfFile: root + "/archive/item.txt", encoding: .utf8) == "existing")
        #expect(FileManager.default.fileExists(atPath: root + "/archive/item"))
        #expect(try String(contentsOfFile: root + "/archive/item 2", encoding: .utf8) == "second")
    }

    @Test("root reads and move destinations are allowed while root mutations fail")
    func rootOperationPolicy() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        try FileManager.default.createDirectory(atPath: root + "/nested", withIntermediateDirectories: false)
        try "x".write(toFile: root + "/nested/file.txt", atomically: true, encoding: .utf8)
        let service = makeService()

        let stat = try await service.stat(root: root, relativePath: "")
        #expect(stat.relativePath == "")

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await service.write(
                root: root,
                relativePath: "",
                data: Data("x".utf8),
                maxBytes: WorkspaceFileService.maxWriteBytes
            )
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await service.mkdir(root: root, relativePath: ".")
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await service.rename(root: root, relativePath: "", newName: "renamed")
        }
        await #expect(throws: FileSystemOperationError.self) {
            _ = try await service.move(root: root, paths: ["."], into: "nested")
        }
        await #expect(throws: FileSystemOperationError.self) {
            try await service.delete(root: root, paths: [""])
        }

        let moved = try await service.move(root: root, paths: ["nested/file.txt"], into: "")
        #expect(moved == ["file.txt"])
    }

    @Test("remote writes enforce the raw byte limit before running a command")
    func writeSizeLimit() async throws {
        let root = try makeTempDir()
        defer { try? FileManager.default.removeItem(atPath: root) }
        let oversized = Data(repeating: 0x41, count: WorkspaceFileService.maxWriteBytes + 1)

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await makeService().write(
                root: root,
                relativePath: "large.bin",
                data: oversized,
                maxBytes: WorkspaceFileService.maxWriteBytes
            )
        }
        #expect(!FileManager.default.fileExists(atPath: root + "/large.bin"))
    }

    private func makeService(
        commandPrefix: String = "",
        transformInput: @escaping @Sendable (Data?) -> Data? = { $0 }
    ) -> RemoteFileService {
        RemoteFileService(destination: SSHDestination(host: "unused")) { _, command, input in
            try await GitProcessRunner.runResolved(
                ResolvedLaunch(
                    executable: "/bin/sh",
                    arguments: ["-c", commandPrefix + command],
                    workingDirectory: nil
                ),
                stdinData: transformInput(input)
            )
        }
    }

    private func makeTempDir() throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("RemoteFileServiceTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.resolvingSymlinksInPath().path
    }

    private func posixMode(_ path: String) throws -> Int {
        let attributes = try FileManager.default.attributesOfItem(atPath: path)
        return (attributes[.posixPermissions] as? NSNumber)?.intValue ?? 0
    }
}
