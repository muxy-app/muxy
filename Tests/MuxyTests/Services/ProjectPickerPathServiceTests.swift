import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerPathService")
struct ProjectPickerPathServiceTests {
    @Test("path expansion standardization and display use the supplied home directory")
    func pathExpansionStandardizationAndDisplay() {
        let service = ProjectPickerPathService(homeDirectory: "/Users/alice")

        #expect(service.expandedPath("~/Projects") == "/Users/alice/Projects")
        #expect(service.expandedPath("~") == "/Users/alice")
        #expect(ProjectPickerPathService.standardizedPath("/Users/alice/Projects/../Code") == "/Users/alice/Code")
        #expect(service.abbreviatedDirectoryDisplayPath("/Users/alice/Projects") == "~/Projects/")
        #expect(service.abbreviatedDirectoryDisplayPath("/tmp/muxy") == "/tmp/muxy/")
    }

    @Test("input state separates directory leaf confirm and parent display paths")
    func inputStateInterpretation() {
        let service = ProjectPickerPathService(homeDirectory: "/Users/alice")
        let tildeState = service.state(for: "~/Projects/mu")
        let bareLeafState = service.state(for: "mu")
        let rootState = service.state(for: "")

        #expect(tildeState.directoryPath == "/Users/alice/Projects")
        #expect(tildeState.leafFilter == "mu")
        #expect(tildeState.confirmPath == "/Users/alice/Projects/mu")
        #expect(tildeState.standardizedConfirmPath == "/Users/alice/Projects/mu")
        #expect(tildeState.parentDisplayPath == "~/")
        #expect(tildeState.completionDisplayPrefix == "~/Projects/")

        #expect(bareLeafState.directoryPath == "/")
        #expect(bareLeafState.leafFilter == "mu")
        #expect(bareLeafState.confirmPath == "/mu")
        #expect(bareLeafState.completionDisplayPrefix == "/")

        #expect(rootState.directoryPath == "/")
        #expect(rootState.parentDisplayPath == "/")
        #expect(rootState.directoryRows(from: ["Users"]) == ["Users"])
    }

    @Test("directory rows include parent hide dotfiles filter and sort names")
    func directoryRows() {
        let service = ProjectPickerPathService(homeDirectory: "/Users/alice")
        let normal = service.state(for: "~/")
        let dotfileSearch = service.state(for: "~/.s")
        let filtered = service.state(for: "~/Projects/al")

        #expect(normal.directoryRows(from: ["Code", ".ssh", "Documents"]) == ["..", "Code", "Documents"])
        #expect(dotfileSearch.directoryRows(from: ["Code", ".ssh", "Documents"]) == ["..", ".ssh"])
        #expect(filtered.directoryRows(from: ["zeta", "Alpha", "alpha 2", ".alpha"]) == ["..", "Alpha", "alpha 2"])
    }

    @Test("typed path state and default location status use the filesystem adapter")
    func typedPathAndDefaultLocationStatus() {
        let service = ProjectPickerPathService(fileSystem: ProjectPickerPathServiceFileSystemStub(
            directoryStates: [
                "/tmp/ready": .directory,
                "/tmp/unreadable": .directory,
                "/tmp/file": .notDirectory,
            ],
            readablePaths: ["/tmp/ready"]
        ))

        #expect(service.typedPathState(path: "/tmp/missing") == .missing)
        #expect(service.typedPathState(path: "/tmp/ready") == .directory)
        #expect(service.typedPathState(path: "/tmp/file") == .notDirectory)

        #expect(service.defaultLocationStatus(path: "/tmp/missing") == .missing)
        #expect(service.defaultLocationStatus(path: "/tmp/file") == .notDirectory)
        #expect(service.defaultLocationStatus(path: "/tmp/unreadable") == .unreadable)
        #expect(service.defaultLocationStatus(path: "/tmp/ready") == .ready)
    }

    @Test("directory snapshot uses adapter contents and read failures")
    func directorySnapshotUsesAdapter() {
        let service = ProjectPickerPathService(homeDirectory: "/Users/alice", fileSystem: ProjectPickerPathServiceFileSystemStub(
            directoryContents: [
                "/Users/alice": .success([.directory("Code"), .file("notes.txt"), .directorySymlink("Linked")]),
                "/Users/alice/Missing": .failure(ProjectPickerPathServiceFileSystemStub.Error()),
            ]
        ))

        let readySnapshot = service.directorySnapshot(for: service.state(for: "~/"))
        let failedSnapshot = service.directorySnapshot(for: service.state(for: "~/Missing/"))

        #expect(readySnapshot == ProjectPickerDirectorySnapshot(
            rows: [.parent, .directory("Code"), .directorySymlink("Linked")],
            readFailed: false
        ))
        #expect(failedSnapshot == ProjectPickerDirectorySnapshot(rows: [".."], readFailed: true))
    }

    @Test("directory snapshot includes directory symlinks and excludes file symlinks")
    func directorySnapshotIncludesDirectorySymlinks() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-symlink-test-\(UUID().uuidString)")
        let targetDirectory = root.appendingPathComponent("target-directory")
        let targetFile = root.appendingPathComponent("target-file")
        let directoryLink = root.appendingPathComponent("directory-link")
        let fileLink = root.appendingPathComponent("file-link")
        try FileManager.default.createDirectory(at: targetDirectory, withIntermediateDirectories: true)
        try Data().write(to: targetFile)
        try FileManager.default.createSymbolicLink(at: directoryLink, withDestinationURL: targetDirectory)
        try FileManager.default.createSymbolicLink(at: fileLink, withDestinationURL: targetFile)
        defer { try? FileManager.default.removeItem(at: root) }

        let service = ProjectPickerPathService(homeDirectory: "/Users/alice")
        let snapshot = service.directorySnapshot(for: service.state(for: root.path + "/"))

        #expect(snapshot.rows.contains(.directory("target-directory")))
        #expect(snapshot.rows.contains(.directorySymlink("directory-link")))
        #expect(!snapshot.rows.map(\.name).contains("target-file"))
        #expect(!snapshot.rows.map(\.name).contains("file-link"))
        #expect(snapshot.rows.first { $0.name == "directory-link" }?.isDirectorySymlink == true)
    }
}

private struct ProjectPickerPathServiceFileSystemStub: ProjectPickerFileSystem {
    struct Error: Swift.Error {}

    var directoryStates: [String: ProjectPickerFileSystemDirectoryState] = [:]
    var readablePaths: Set<String> = []
    var directoryContents: [String: Result<[ProjectPickerFileSystemDirectoryEntry], Swift.Error>] = [:]

    func directoryState(atPath path: String) -> ProjectPickerFileSystemDirectoryState {
        directoryStates[path] ?? .missing
    }

    func isReadableFile(atPath path: String) -> Bool {
        readablePaths.contains(path)
    }

    func contentsOfDirectory(atPath path: String) throws -> [ProjectPickerFileSystemDirectoryEntry] {
        switch directoryContents[path] {
        case let .success(entries):
            return entries
        case let .failure(error):
            throw error
        case nil:
            throw Error()
        }
    }
}
