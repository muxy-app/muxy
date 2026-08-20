import Testing

@testable import Muxy

@Suite("RepositoryChangesFileSelection")
struct RepositoryChangesFileSelectionTests {
    private let ids = ["a.swift", "b.swift", "c.swift", "d.swift", "e.swift"]

    @Test("plain click selects only the clicked file")
    func exclusiveClickReplacesSelection() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "b.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "d.swift", ids: ids, kind: .exclusive)

        #expect(selection.selectedIDs == ["d.swift"])
        #expect(selection.anchorID == "d.swift")
    }

    @Test("plain click on the only selected file clears the selection")
    func exclusiveClickOnSoleSelectionClears() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "b.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "b.swift", ids: ids, kind: .exclusive)

        #expect(selection.isEmpty)
        #expect(selection.anchorID == nil)
    }

    @Test("command click toggles files without clearing the rest")
    func commandClickToggles() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "a.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "c.swift", ids: ids, kind: .toggle)
        selection.handleClick(id: "e.swift", ids: ids, kind: .toggle)
        selection.handleClick(id: "c.swift", ids: ids, kind: .toggle)

        #expect(selection.selectedIDs == ["a.swift", "e.swift"])
        #expect(selection.anchorID == "c.swift")
    }

    @Test("shift click selects a contiguous range from the anchor")
    func shiftClickSelectsRange() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "b.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "d.swift", ids: ids, kind: .range)

        #expect(selection.selectedIDs == ["b.swift", "c.swift", "d.swift"])
        #expect(selection.anchorID == "b.swift")
    }

    @Test("later shift clicks keep the original anchor")
    func shiftClickKeepsAnchor() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "c.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "e.swift", ids: ids, kind: .range)
        selection.handleClick(id: "a.swift", ids: ids, kind: .range)

        #expect(selection.selectedIDs == ["a.swift", "b.swift", "c.swift"])
        #expect(selection.anchorID == "c.swift")
    }

    @Test("command-shift click adds a range to the existing selection")
    func commandShiftAddsRange() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "a.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "e.swift", ids: ids, kind: .toggle)
        selection.handleClick(id: "c.swift", ids: ids, kind: .addingRange)

        #expect(selection.selectedIDs == ["a.swift", "c.swift", "d.swift", "e.swift"])
        #expect(selection.anchorID == "e.swift")
    }

    @Test("shift click without an anchor selects the clicked file")
    func shiftClickWithoutAnchor() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "c.swift", ids: ids, kind: .range)

        #expect(selection.selectedIDs == ["c.swift"])
        #expect(selection.anchorID == "c.swift")
    }

    @Test("clicks on unknown ids are ignored")
    func ignoresUnknownIDs() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "b.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "missing.swift", ids: ids, kind: .toggle)

        #expect(selection.selectedIDs == ["b.swift"])
        #expect(selection.anchorID == "b.swift")
    }

    @Test("maps modifier flags onto click kinds")
    func clickKindFromModifiers() {
        #expect(RepositoryChangesFileSelection.Click.from(command: false, shift: false) == .exclusive)
        #expect(RepositoryChangesFileSelection.Click.from(command: true, shift: false) == .toggle)
        #expect(RepositoryChangesFileSelection.Click.from(command: false, shift: true) == .range)
        #expect(RepositoryChangesFileSelection.Click.from(command: true, shift: true) == .addingRange)
    }

    @Test("row actions apply to the selection when the clicked file is selected")
    func actionTargetsUseSelection() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "a.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "c.swift", ids: ids, kind: .toggle)
        let files = ids.map { file(path: $0) }

        #expect(selection.actionTargets(files[0], in: files).map(\.path) == ["a.swift", "c.swift"])
        #expect(selection.actionTargets(files[3], in: files).map(\.path) == ["d.swift"])
    }

    @Test("retain drops files that left the list and preserves list order")
    func retainDropsMissingFiles() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "a.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "c.swift", ids: ids, kind: .toggle)
        selection.handleClick(id: "e.swift", ids: ids, kind: .toggle)
        selection.retain(ids: ["c.swift", "d.swift", "e.swift"])

        let remaining = [
            file(path: "c.swift"),
            file(path: "d.swift"),
            file(path: "e.swift"),
        ]
        #expect(selection.files(in: remaining).map(\.path) == ["c.swift", "e.swift"])
        #expect(selection.anchorID == "e.swift")

        selection.retain(ids: ["c.swift"])
        #expect(selection.selectedIDs == ["c.swift"])
        #expect(selection.anchorID == nil)
    }

    @Test("remove drops staged files without selecting them in another list")
    func removeDropsIDs() {
        var selection = RepositoryChangesFileSelection()
        selection.handleClick(id: "a.swift", ids: ids, kind: .exclusive)
        selection.handleClick(id: "c.swift", ids: ids, kind: .toggle)
        selection.remove(ids: ["a.swift", "c.swift"])

        #expect(selection.isEmpty)
        #expect(selection.anchorID == nil)
    }

    private func file(path: String) -> GitStatusFile {
        GitStatusFile(
            path: path,
            oldPath: nil,
            xStatus: " ",
            yStatus: "M",
            additions: nil,
            deletions: nil,
            isBinary: false
        )
    }
}
