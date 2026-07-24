import Testing

@testable import Muxy

@Suite("WorktreeLocationSelection")
struct WorktreeLocationSelectionTests {
    @Test("editing a legacy folder preserves folder semantics")
    func editingLegacyFolderPreservesMode() {
        var selection = WorktreeLocationSelection(parentPath: "/tmp/legacy")

        selection.value = "/tmp/edited"

        #expect(selection.mode == .parentFolder)
        #expect(selection.selectedPathTemplate == nil)
        #expect(selection.selectedParentPath == "/tmp/edited")
    }

    @Test("switching modes retains each location value")
    func switchingModesRetainsValues() {
        var selection = WorktreeLocationSelection(parentPath: "/tmp/legacy")

        selection.select(.pathTemplate)
        #expect(selection.value.isEmpty)

        selection.value = "../{base-dir}.{branch}"
        selection.select(.parentFolder)

        #expect(selection.value == "/tmp/legacy")
        #expect(selection.selectedPathTemplate == nil)
        #expect(selection.selectedParentPath == "/tmp/legacy")

        selection.select(.pathTemplate)

        #expect(selection.value == "../{base-dir}.{branch}")
        #expect(selection.selectedPathTemplate == "../{base-dir}.{branch}")
        #expect(selection.selectedParentPath == nil)
    }

    @Test("default mode clears both persisted location values")
    func defaultModeClearsPersistedValues() {
        var selection = WorktreeLocationSelection(
            pathTemplate: "../{base-dir}.{branch}",
            parentPath: "/tmp/legacy"
        )

        selection.select(.defaultLocation)

        #expect(selection.selectedPathTemplate == nil)
        #expect(selection.selectedParentPath == nil)
    }
}
