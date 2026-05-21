import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerNavigator")
struct ProjectPickerNavigatorTests {
    @Test("tab completion replaces the typed leaf with the highlighted directory")
    func tabCompletion() {
        let navigator = ProjectPickerNavigator(input: "~/Projects/mu", homeDirectory: "/Users/alice")

        #expect(navigator.completedPath(highlightedRow: "muxy") == "~/Projects/muxy/")
    }

    @Test("completion from empty and bare tilde inputs stays absolute")
    func completionFromEmptyAndBareTildeInputs() {
        #expect(ProjectPickerNavigator(input: "", homeDirectory: "/Users/alice").completedPath(highlightedRow: "Users") == "/Users/")
        #expect(ProjectPickerNavigator(input: "~", homeDirectory: "/Users/alice").completedPath(highlightedRow: "Projects") == "~/Projects/")
    }

    @Test("parent path walks above home to filesystem root and stops")
    func parentPathWalksToRoot() {
        #expect(ProjectPickerNavigator(input: "~/Projects/", homeDirectory: "/Users/alice").parentDisplayPath == "~/")
        #expect(ProjectPickerNavigator(input: "~/", homeDirectory: "/Users/alice").parentDisplayPath == "/Users/")
        #expect(ProjectPickerNavigator(input: "/Users/", homeDirectory: "/Users/alice").parentDisplayPath == "/")
        #expect(ProjectPickerNavigator(input: "/", homeDirectory: "/Users/alice").parentDisplayPath == "/")
    }

    @Test("ghost text completes matching display paths and ignores parent rows")
    func ghostTextUsesHighlightedRow() {
        #expect(ProjectPickerNavigator(input: "~/Projects/mu", homeDirectory: "/Users/alice").ghostText(highlightedRow: "muxy") == "xy/")
        #expect(ProjectPickerNavigator(input: "mu", homeDirectory: "/Users/alice").ghostText(highlightedRow: "muxy") == "xy/")
        #expect(ProjectPickerNavigator(input: "muxy", homeDirectory: "/Users/alice").ghostText(highlightedRow: "muxy") == "/")
        #expect(ProjectPickerNavigator(input: "~/Projects/", homeDirectory: "/Users/alice").ghostText(highlightedRow: "..") == "")
    }
}
