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
        #expect(WorkspaceFileService.resolve(root: root, relativePath: "src/main.swift") == root + "/src/main.swift")
        #expect(WorkspaceFileService.resolve(root: root, relativePath: "") == root)
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
        #expect(WorkspaceFileService.resolve(root: root, relativePath: "alias/note.txt") == root + "/real/note.txt")
    }

    @Test("contained returns in-root paths and throws outsideRoot on escape")
    func containedGuardsAtOpTime() throws {
        let root = try makeTempDir()
        let outside = try makeTempDir()
        defer {
            try? FileManager.default.removeItem(atPath: root)
            try? FileManager.default.removeItem(atPath: outside)
        }
        #expect(try WorkspaceFileService.contained(root: root, relativePath: "src/main.swift") == root + "/src/main.swift")
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
        #expect(WorkspaceFileService.relative(root + "/src/main.swift", root: root) == "src/main.swift")
        #expect(WorkspaceFileService.relative("/elsewhere/note.txt", root: root) == "note.txt")
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
        let oversized = Data(repeating: 0x41, count: WorkspaceFileService.maxReadBytes + 1)
        try oversized.write(to: URL(fileURLWithPath: root.path + "/big.txt"))

        await #expect(throws: FileSystemOperationError.self) {
            _ = try await WorkspaceFileService.read(root: root, path: "big.txt", encoding: .base64)
        }
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

    private func makeRoot() throws -> WorkspaceFileService.Root {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("WorkspaceFileServiceLocalTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return WorkspaceFileService.Root(path: dir.resolvingSymlinksInPath().path, workspaceContext: .local)
    }
}
