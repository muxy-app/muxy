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
}
