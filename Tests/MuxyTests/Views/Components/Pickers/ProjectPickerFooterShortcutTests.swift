import Testing

@testable import Muxy

@Suite("ProjectPickerFooterShortcut")
struct ProjectPickerFooterShortcutTests {
    @Test("tab autocomplete shortcut is shown in the footer")
    func tabAutocompleteShortcutIsShown() {
        let shortcuts = ProjectPickerFooterShortcut.ordered(actionTitle: "Add Project")

        #expect(shortcuts.map(\.label) == ["Navigate", "Autocomplete", "Open", "Add Project", "Go back", "Close"])
        #expect(shortcuts.map(\.intents) == [
            [.moveHighlightUp, .moveHighlightDown],
            [.completeHighlighted],
            [.openHighlighted],
            [.confirmTypedPath],
            [.goBack],
            [.dismiss],
        ])
        #expect(shortcuts.flatMap(\.intents).allSatisfy(ProjectPickerCommand.handledIntents.contains))
        #expect(shortcuts[1].keycap == .tab)
        #expect(shortcuts[4].keycap == .optionDelete)
        #expect(shortcuts[5].keycap == .escape)
    }

    @Test("typed path action title changes label without changing command identity")
    func typedPathActionTitleOnlyChangesLabel() {
        let addShortcuts = ProjectPickerFooterShortcut.ordered(actionTitle: "Add Project")
        let createShortcuts = ProjectPickerFooterShortcut.ordered(actionTitle: "Create & Add Project")

        #expect(addShortcuts.map(\.intents) == createShortcuts.map(\.intents))
        #expect(addShortcuts[3].label == "Add Project")
        #expect(createShortcuts[3].label == "Create & Add Project")
        #expect(addShortcuts[3].intents == [.confirmTypedPath])
    }
}
