import Testing

@testable import Muxy

@Suite("ProjectPickerFooterShortcut")
struct ProjectPickerFooterShortcutTests {
    @Test("go back shortcut is shown before close in the footer")
    func goBackShortcutPrecedesClose() {
        let shortcuts = ProjectPickerFooterShortcut.ordered(actionTitle: "Add Project")

        #expect(shortcuts.map(\.label) == ["Navigate", "Open", "Add Project", "Go back", "Close"])
        #expect(shortcuts.map(\.command) == ProjectPickerCommand.footerCommands)
        #expect(shortcuts.flatMap(\.command.sessionCommands).allSatisfy { $0.isSessionHandled })
        #expect(shortcuts[3].keycap == .optionDelete)
        #expect(shortcuts[4].keycap == .escape)
    }

    @Test("typed path action title changes label without changing command identity")
    func typedPathActionTitleOnlyChangesLabel() {
        let addShortcuts = ProjectPickerFooterShortcut.ordered(actionTitle: "Add Project")
        let createShortcuts = ProjectPickerFooterShortcut.ordered(actionTitle: "Create & Add Project")

        #expect(addShortcuts.map(\.command) == createShortcuts.map(\.command))
        #expect(addShortcuts[2].label == "Add Project")
        #expect(createShortcuts[2].label == "Create & Add Project")
        #expect(addShortcuts[2].command == .confirmTypedPath)
    }
}
