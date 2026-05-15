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
}
