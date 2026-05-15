import Testing

@testable import Muxy

@Suite("ProjectPickerFooterShortcut")
struct ProjectPickerFooterShortcutTests {
    @Test("go back shortcut is shown before close in the footer")
    func goBackShortcutPrecedesClose() {
        let shortcuts = ProjectPickerFooterShortcut.ordered(actionTitle: "Add Project")

        #expect(shortcuts.map(\.label) == ["Navigate", "Open", "Add Project", "Go back", "Close"])
        #expect(shortcuts[3].keycap == .optionDelete)
        #expect(shortcuts[4].keycap == .escape)
    }
}
