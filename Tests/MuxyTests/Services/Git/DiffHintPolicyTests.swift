import Testing

@testable import Muxy

@Suite("DiffHintPolicy")
struct DiffHintPolicyTests {
    @Test("unstaged side of staged added file uses new file preview path")
    func unstagedSideOfStagedAddedFileUsesNewFilePreviewPath() {
        let hints = DiffHintPolicy.hints(
            file: makeFile(xStatus: "A", yStatus: "M"),
            isStaged: false
        )

        #expect(!hints.hasStaged)
        #expect(!hints.hasUnstaged)
        #expect(hints.isUntrackedOrNew)
    }

    @Test("untracked files use new file preview path")
    func untrackedFilesUseNewFilePreviewPath() {
        let hints = DiffHintPolicy.hints(
            file: makeFile(xStatus: "?", yStatus: "?"),
            isStaged: false
        )

        #expect(!hints.hasStaged)
        #expect(!hints.hasUnstaged)
        #expect(hints.isUntrackedOrNew)
    }

    @Test("staged side only loads cached diff")
    func stagedSideOnlyLoadsCachedDiff() {
        let hints = DiffHintPolicy.hints(
            file: makeFile(xStatus: "M", yStatus: "M"),
            isStaged: true
        )

        #expect(hints.hasStaged)
        #expect(!hints.hasUnstaged)
        #expect(!hints.isUntrackedOrNew)
    }

    private func makeFile(xStatus: Character, yStatus: Character) -> GitStatusFile {
        GitStatusFile(
            path: "File.swift",
            oldPath: nil,
            xStatus: xStatus,
            yStatus: yStatus,
            additions: 1,
            deletions: 0,
            isBinary: false
        )
    }
}
