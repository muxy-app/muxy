import Testing

@testable import Muxy

@Suite("DiffCollapsePolicy")
struct DiffCollapsePolicyTests {
    @Test("collapses binary and deleted files")
    func collapsesBinaryAndDeletedFiles() {
        #expect(DiffCollapsePolicy.shouldCollapseByDefault(makeFile(path: "image.png", yStatus: "M", isBinary: true)))
        #expect(DiffCollapsePolicy.shouldCollapseByDefault(makeFile(path: "Old.swift", yStatus: "D")))
    }

    @Test("collapses large regular diffs")
    func collapsesLargeRegularDiffs() {
        #expect(DiffCollapsePolicy.shouldCollapseByDefault(makeFile(path: "Feature.swift", yStatus: "M", additions: 900, deletions: 100)))
    }

    @Test("collapses noisy files at lower threshold")
    func collapsesNoisyFilesAtLowerThreshold() {
        #expect(DiffCollapsePolicy.shouldCollapseByDefault(makeFile(path: "package-lock.json", yStatus: "M", additions: 200)))
        #expect(DiffCollapsePolicy.shouldCollapseByDefault(makeFile(path: "Sources/Generated/API.swift", yStatus: "M", additions: 200)))
        #expect(DiffCollapsePolicy.shouldCollapseByDefault(makeFile(path: "Tests/__snapshots__/View.snap", yStatus: "M", additions: 200)))
    }

    @Test("keeps small meaningful diffs expanded")
    func keepsSmallMeaningfulDiffsExpanded() {
        #expect(!DiffCollapsePolicy.shouldCollapseByDefault(makeFile(path: "Feature.swift", yStatus: "M", additions: 50)))
        #expect(!DiffCollapsePolicy.shouldCollapseByDefault(makeFile(path: "package-lock.json", yStatus: "M", additions: 199)))
    }

    @Test("uses staged and unstaged bucket stats independently")
    func usesBucketStatsIndependently() {
        let file = GitStatusFile(
            path: "Feature.swift",
            oldPath: nil,
            xStatus: "M",
            yStatus: "M",
            additions: 1001,
            deletions: 0,
            stagedAdditions: 1,
            stagedDeletions: 2,
            unstagedAdditions: 900,
            unstagedDeletions: 100,
            isBinary: false
        )

        #expect(file.additions(isStaged: true) == 1)
        #expect(file.deletions(isStaged: true) == 2)
        #expect(file.additions(isStaged: false) == 900)
        #expect(file.deletions(isStaged: false) == 100)
        #expect(!DiffCollapsePolicy.shouldCollapseByDefault(file, isStaged: true))
        #expect(DiffCollapsePolicy.shouldCollapseByDefault(file, isStaged: false))
    }

    private func makeFile(
        path: String,
        xStatus: Character = " ",
        yStatus: Character,
        additions: Int = 1,
        deletions: Int = 0,
        isBinary: Bool = false
    ) -> GitStatusFile {
        GitStatusFile(
            path: path,
            oldPath: nil,
            xStatus: xStatus,
            yStatus: yStatus,
            additions: additions,
            deletions: deletions,
            isBinary: isBinary
        )
    }
}
