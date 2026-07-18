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
        session.setInput("~/")

        session.applyDirectorySnapshot(ProjectPickerDirectorySnapshot(rows: ["..", "Code", "Documents"], readFailed: false))

        #expect(session.directoryLoadState == .loaded)
        #expect(session.highlightedIndex == 1)
        #expect(session.highlightedRow == "Code")
    }

    @Test("navigation, completion, and parent commands update state")
    func commandStateTransitions() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/Projects/mu", homeDirectory: "/Users/alice", projectPaths: [])
        session.setInput("~/Projects/mu")
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
        session.setInput("~/Projects/")
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

        var existingSession = ProjectPickerSession(
            defaultDisplayPath: "/tmp/existing",
            projectPaths: [],
            pathService: pathService
        )
        existingSession.setInput("/tmp/existing")
        #expect(existingSession.typedPathState == .directory)
        #expect(existingSession.actionTitle == "Add")
        #expect(existingSession.topRightActionTitle == "Add Project")

        var missingSession = ProjectPickerSession(
            defaultDisplayPath: "/tmp/existing/missing",
            projectPaths: [],
            pathService: pathService
        )
        missingSession.setInput("/tmp/existing/missing")
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

        var session = ProjectPickerSession(defaultDisplayPath: root.path, projectPaths: [root.standardizedFileURL.path])
        session.setInput(root.path)

        #expect(session.actionTitle == "Open")
        #expect(session.topRightActionTitle == "Open Project")
    }

    @Test("local input defaults to folder search and explicit paths retain path mode")
    func inputModeSelection() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/Projects/", homeDirectory: "/Users/alice", projectPaths: [])

        #expect(session.input.isEmpty)
        #expect(session.searchRootPath == "/Users/alice/Projects")
        #expect(session.inputMode == .folderSearch)

        for input in ["muxy", "Projects/muxy", "Projects muxy", "muxy/"] {
            session.setInput(input)
            #expect(session.inputMode == .folderSearch)
        }

        for input in ["~/Projects", "/Users/alice/Projects", "./Projects", "../Projects", "~", ".", ".."] {
            session.setInput(input)
            #expect(session.inputMode == .path)
        }
    }

    @Test("remote sessions stay in path mode without recursive folder search")
    func remoteSessionUsesPathMode() {
        let session = ProjectPickerSession(
            projectPaths: [],
            context: .ssh(SSHDestination(host: "server", remoteRoot: "~/code"))
        )

        #expect(!session.allowsFolderSearch)
        #expect(session.input == "~/code/")
        #expect(session.inputMode == .path)
    }

    @Test("folder search results keep full path identity and can switch to path mode")
    func folderSearchResultState() {
        let first = ProjectPickerFolderSearchResult(
            name: "muxy",
            path: "/Users/alice/Code/muxy",
            displayPath: "~/Code/muxy/"
        )
        let existing = ProjectPickerFolderSearchResult(
            name: "muxy",
            path: "/Users/alice/Work/muxy",
            displayPath: "~/Work/muxy/"
        )
        var session = ProjectPickerSession(
            defaultDisplayPath: "~/",
            homeDirectory: "/Users/alice",
            projectPaths: [existing.path]
        )
        session.setInput("muxy")
        session.applyFolderSearchSnapshot(ProjectPickerFolderSearchSnapshot(
            results: [first, existing],
            readFailed: false
        ))

        #expect(session.highlightedSearchResult == first)
        #expect(session.actionTitle == "Add")

        session.handle(.moveHighlightDown)

        #expect(session.highlightedSearchResult == existing)
        #expect(session.actionTitle == "Open")

        session.handle(.completeHighlighted)

        #expect(session.input == "~/Work/muxy/")
        #expect(session.inputMode == .path)
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

    func contentsOfDirectory(atPath path: String) async throws -> [ProjectPickerFileSystemDirectoryEntry] {
        []
    }
}
