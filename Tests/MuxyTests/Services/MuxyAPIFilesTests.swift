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
