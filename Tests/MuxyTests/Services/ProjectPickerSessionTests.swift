import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerSession")
struct ProjectPickerSessionTests {
    @Test("input changes reset loading state")
    func inputChangeResetsLoadingState() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/", homeDirectory: "/Users/alice", projectPaths: [])

        session.setInput("~/Projects/mu")

        #expect(session.input == "~/Projects/mu")
        #expect(session.directoryLoadState == .loading(showsMessage: false))
    }

    @Test("snapshot application chooses first real row after parent row")
    func snapshotApplicationChoosesInitialHighlight() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/", homeDirectory: "/Users/alice", projectPaths: [])

        session.applyDirectorySnapshot(ProjectPickerDirectorySnapshot(rows: ["..", "Code", "Documents"], readFailed: false))

        #expect(session.directoryLoadState == .loaded)
        #expect(session.highlightedIndex == 1)
        #expect(session.highlightedRow == "Code")
    }

    @Test("navigation, completion, and parent commands update state")
    func commandStateTransitions() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/Projects/mu", homeDirectory: "/Users/alice", projectPaths: [])
        session.applyDirectorySnapshot(ProjectPickerDirectorySnapshot(rows: ["muxy", "sample"], readFailed: false))

        session.handle(.moveHighlightDown)
        #expect(session.highlightedIndex == 1)

        session.handle(.completeHighlighted)
        #expect(session.input == "~/Projects/sample/")

        session.handle(.goBack)
        #expect(session.input == "~/Projects/")
    }

    @Test("return descends into selected folder and parent row goes up")
    func returnDescendsAndParentGoesUp() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/Projects/", homeDirectory: "/Users/alice", projectPaths: [])
        session.applyDirectorySnapshot(ProjectPickerDirectorySnapshot(rows: ["..", "muxy"], readFailed: false))

        session.handle(.openHighlighted)

        #expect(session.input == "~/Projects/muxy/")

        session.applyDirectorySnapshot(ProjectPickerDirectorySnapshot(rows: [".."], readFailed: false))
        session.selectRow(at: 0)
        session.handle(.openHighlighted)

        #expect(session.input == "~/Projects/")
    }

    @Test("typed path state drives action titles")
    func typedPathStateDrivesActionTitles() {
        let pathService = ProjectPickerPathService(
            fileSystem: ProjectPickerFileSystemStub(directoryStates: [
                "/tmp/existing": .directory,
                "/tmp/existing/missing": .missing,
            ])
        )

        let existingSession = ProjectPickerSession(
            defaultDisplayPath: "/tmp/existing",
            projectPaths: [],
            pathService: pathService
        )
        #expect(existingSession.typedPathState == .directory)
        #expect(existingSession.actionTitle == "Add")
        #expect(existingSession.topRightActionTitle == "Add Project")

        let missingSession = ProjectPickerSession(
            defaultDisplayPath: "/tmp/existing/missing",
            projectPaths: [],
            pathService: pathService
        )
        #expect(missingSession.typedPathState == .missing)
        #expect(missingSession.actionTitle == "Create & Add")
        #expect(missingSession.topRightActionTitle == "Create & Add Project")
    }

    @Test("existing project updates action titles")
    func existingProjectUpdatesActionTitles() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-session-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let session = ProjectPickerSession(defaultDisplayPath: root.path, projectPaths: [root.standardizedFileURL.path])

        #expect(session.actionTitle == "Open")
        #expect(session.topRightActionTitle == "Open Project")
    }
}

private struct ProjectPickerFileSystemStub: ProjectPickerFileSystem {
    let directoryStates: [String: ProjectPickerFileSystemDirectoryState]

    func directoryState(atPath path: String) -> ProjectPickerFileSystemDirectoryState {
        directoryStates[path] ?? .missing
    }

    func isReadableFile(atPath path: String) -> Bool {
        directoryStates[path] == .directory
    }

    func contentsOfDirectory(atPath path: String) throws -> [ProjectPickerFileSystemDirectoryEntry] {
        []
    }
}
