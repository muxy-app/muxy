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

    @Test("directory read errors distinguish permissions from missing folders and other failures")
    func directoryReadErrorCategorization() {
        #expect(ProjectPickerDirectoryReadFailure(error: posixError(EACCES)).kind == .permissionDenied)
        #expect(ProjectPickerDirectoryReadFailure(error: posixError(EPERM)).kind == .permissionDenied)
        #expect(ProjectPickerDirectoryReadFailure(error: posixError(ENOENT)).kind == .notFound)
        #expect(ProjectPickerDirectoryReadFailure(error: posixError(EIO)).kind == .ioFailure)
        #expect(ProjectPickerDirectoryReadFailure(error: cocoaError(underlying: posixError(EACCES))).kind == .permissionDenied)
    }

    private func posixError(_ code: Int32) -> NSError {
        NSError(domain: NSPOSIXErrorDomain, code: Int(code))
    }

    private func cocoaError(underlying: NSError) -> NSError {
        NSError(domain: NSCocoaErrorDomain, code: NSFileReadNoPermissionError, userInfo: [NSUnderlyingErrorKey: underlying])
    }
}
