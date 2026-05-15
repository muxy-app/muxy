import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerSession")
struct ProjectPickerSessionTests {
    @Test("input changes request directory reload and reset loading state")
    func inputChangeRequestsDirectoryReload() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/", homeDirectory: "/Users/alice", projectPaths: [])

        let effect = session.setInput("~/Projects/mu")

        #expect(session.input == "~/Projects/mu")
        #expect(session.directoryLoadState == .loading(showsMessage: false))
        #expect(effect == .requestDirectoryReload(ProjectPickerNavigator(input: "~/Projects/mu", homeDirectory: "/Users/alice")))
    }

    @Test("snapshot application chooses first real row after parent row")
    func snapshotApplicationChoosesInitialHighlight() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/", homeDirectory: "/Users/alice", projectPaths: [])

        session.applyDirectorySnapshot(ProjectPickerDirectorySnapshot(rows: ["..", "Code", "Documents"], readFailed: false))

        #expect(session.directoryLoadState == .loaded)
        #expect(session.highlightedIndex == 1)
        #expect(session.highlightedRow == "Code")
    }

    @Test("navigation, completion, and parent commands update state through effects")
    func commandStateTransitions() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/Projects/mu", homeDirectory: "/Users/alice", projectPaths: [])
        session.applyDirectorySnapshot(ProjectPickerDirectorySnapshot(rows: ["muxy", "sample"], readFailed: false))

        _ = session.handle(.moveHighlightDown)
        #expect(session.highlightedIndex == 1)

        let completionEffects = session.handle(.completeHighlighted)
        #expect(session.input == "~/Projects/sample/")
        #expect(completionEffects == [
            .requestDirectoryReload(ProjectPickerNavigator(input: "~/Projects/sample/", homeDirectory: "/Users/alice")),
        ])

        let parentEffects = session.handle(.goBack)
        #expect(session.input == "~/Projects/")
        #expect(parentEffects == [
            .requestDirectoryReload(ProjectPickerNavigator(input: "~/Projects/", homeDirectory: "/Users/alice")),
        ])
    }

    @Test("return descends into selected folder and parent row goes up")
    func returnDescendsAndParentGoesUp() {
        var session = ProjectPickerSession(defaultDisplayPath: "~/Projects/", homeDirectory: "/Users/alice", projectPaths: [])
        session.applyDirectorySnapshot(ProjectPickerDirectorySnapshot(rows: ["..", "muxy"], readFailed: false))

        let descendEffects = session.handle(.openHighlighted)

        #expect(session.input == "~/Projects/muxy/")
        #expect(descendEffects == [
            .requestDirectoryReload(ProjectPickerNavigator(input: "~/Projects/muxy/", homeDirectory: "/Users/alice")),
        ])

        session.applyDirectorySnapshot(ProjectPickerDirectorySnapshot(rows: [".."], readFailed: false))
        session.selectRow(at: 0)
        let parentEffects = session.handle(.openHighlighted)

        #expect(session.input == "~/Projects/")
        #expect(parentEffects == [
            .requestDirectoryReload(ProjectPickerNavigator(input: "~/Projects/", homeDirectory: "/Users/alice")),
        ])
    }

    @Test("typed path confirmation emits create or confirm effects")
    func typedPathConfirmationEffects() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-session-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        var existingSession = ProjectPickerSession(defaultDisplayPath: root.path, projectPaths: [])
        #expect(existingSession.handle(.confirmTypedPath) == [
            .confirmProjectPath(path: root.standardizedFileURL.path, createIfMissing: false),
        ])

        let missing = root.appendingPathComponent("missing", isDirectory: true)
        var missingSession = ProjectPickerSession(defaultDisplayPath: missing.path, projectPaths: [])
        #expect(missingSession.handle(.confirmTypedPath) == [
            .confirmCreateDirectory(path: missing.standardizedFileURL.path),
        ])
        #expect(missingSession.confirmCreateDirectoryAccepted() == [
            .confirmProjectPath(path: missing.standardizedFileURL.path, createIfMissing: true),
        ])
    }

    @Test("existing project updates action titles and failure presentation stays view independent")
    func actionTitlesAndFailurePresentation() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-session-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let session = ProjectPickerSession(defaultDisplayPath: root.path, projectPaths: [root.standardizedFileURL.path])
        let presentation = session.confirmationFailurePresentation(for: .notDirectory)

        #expect(session.actionTitle == "Open")
        #expect(session.topRightActionTitle == "Open Project")
        #expect(presentation.title == "Path Is Not a Folder")
        #expect(presentation.message == "Muxy can only add folders as projects. Choose a folder or type a new folder path.")
    }
}
