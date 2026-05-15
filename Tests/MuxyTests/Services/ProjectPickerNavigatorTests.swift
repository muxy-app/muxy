import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerNavigator")
struct ProjectPickerNavigatorTests {
    @Test("tilde path expands home directory and uses the leaf segment as the filter")
    func tildePathFilter() {
        let navigator = ProjectPickerNavigator(input: "~/Projects/mu", homeDirectory: "/Users/alice")

        #expect(navigator.directoryPath == "/Users/alice/Projects")
        #expect(navigator.leafFilter == "mu")
    }

    @Test("directory rows include parent and hide dotfiles until the leaf starts with a dot")
    func directoryRowsHideDotfiles() {
        let normal = ProjectPickerNavigator(input: "~/", homeDirectory: "/Users/alice")
        let dotfileSearch = ProjectPickerNavigator(input: "~/.s", homeDirectory: "/Users/alice")

        #expect(normal.directoryRows(from: ["Code", ".ssh", "Documents"]) == ["..", "Code", "Documents"])
        #expect(dotfileSearch.directoryRows(from: ["Code", ".ssh", "Documents"]) == ["..", ".ssh"])
    }

    @Test("tab completion replaces the typed leaf with the highlighted directory")
    func tabCompletion() {
        let navigator = ProjectPickerNavigator(input: "~/Projects/mu", homeDirectory: "/Users/alice")

        #expect(navigator.completedPath(highlightedRow: "muxy") == "~/Projects/muxy/")
    }

    @Test("bare leaf input filters from filesystem root")
    func bareLeafInputFiltersRoot() {
        let navigator = ProjectPickerNavigator(input: "mu", homeDirectory: "/Users/alice")

        #expect(navigator.directoryPath == "/")
        #expect(navigator.leafFilter == "mu")
        #expect(navigator.confirmPath == "/mu")
        #expect(navigator.completedPath(highlightedRow: "muxy") == "/muxy/")
    }

    @Test("completion from empty and bare tilde inputs stays absolute")
    func completionFromEmptyAndBareTildeInputs() {
        #expect(ProjectPickerNavigator(input: "", homeDirectory: "/Users/alice").completedPath(highlightedRow: "Users") == "/Users/")
        #expect(ProjectPickerNavigator(input: "~", homeDirectory: "/Users/alice").directoryPath == "/Users/alice")
        #expect(ProjectPickerNavigator(input: "~", homeDirectory: "/Users/alice").completedPath(highlightedRow: "Projects") == "~/Projects/")
    }

    @Test("path expansion uses the supplied home directory")
    func pathExpansionUsesSuppliedHomeDirectory() {
        #expect(ProjectPickerPathSemantics.expandedPath("~/Projects", homeDirectory: "/Users/alice") == "/Users/alice/Projects")
        #expect(ProjectPickerPathSemantics.expandedPath("~", homeDirectory: "/Users/alice") == "/Users/alice")
    }

    @Test("parent path walks above home to filesystem root and stops")
    func parentPathWalksToRoot() {
        #expect(ProjectPickerNavigator(input: "~/Projects/", homeDirectory: "/Users/alice").parentDisplayPath == "~/")
        #expect(ProjectPickerNavigator(input: "~/", homeDirectory: "/Users/alice").parentDisplayPath == "/Users/")
        #expect(ProjectPickerNavigator(input: "/Users/", homeDirectory: "/Users/alice").parentDisplayPath == "/")
        #expect(ProjectPickerNavigator(input: "/", homeDirectory: "/Users/alice").parentDisplayPath == "/")
    }

    @Test("empty input browses from filesystem root instead of process working directory")
    func emptyInputBrowsesRoot() {
        let navigator = ProjectPickerNavigator(input: "", homeDirectory: "/Users/alice")

        #expect(navigator.directoryPath == "/")
        #expect(navigator.parentDisplayPath == "/")
    }

    @Test("directory snapshot includes symlinked folders")
    func directorySnapshotIncludesSymlinkedFolders() throws {
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

        let navigator = ProjectPickerNavigator(input: root.path + "/", homeDirectory: "/Users/alice")
        let snapshot = ProjectPickerDirectorySnapshot.load(navigator: navigator)

        #expect(snapshot.rows.contains("target-directory"))
        #expect(snapshot.rows.contains("directory-link"))
        #expect(!snapshot.rows.contains("target-file"))
        #expect(!snapshot.rows.contains("file-link"))
    }

}
