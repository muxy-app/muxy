import Foundation
import Testing

@testable import Muxy

@Suite("GitModels")
struct GitModelsTests {
    private func makeStatusFile(
        xStatus: Character = " ",
        yStatus: Character = " "
    ) -> GitStatusFile {
        GitStatusFile(
            path: "test.swift",
            oldPath: nil,
            xStatus: xStatus,
            yStatus: yStatus,
            additions: nil,
            deletions: nil,
            isBinary: false
        )
    }

    @Test("isStaged returns true for staged statuses", arguments: ["A", "M", "D", "R", "C"] as [Character])
    func isStagedTrue(status: Character) {
        let file = makeStatusFile(xStatus: status)
        #expect(file.isStaged)
    }

    @Test("isStaged returns false for non-staged statuses", arguments: [" ", "?", "U"] as [Character])
    func isStagedFalse(status: Character) {
        let file = makeStatusFile(xStatus: status)
        #expect(!file.isStaged)
    }

    @Test("isUnstaged returns true for unstaged yStatus", arguments: ["M", "D", "?"] as [Character])
    func isUnstagedTrue(status: Character) {
        let file = makeStatusFile(yStatus: status)
        #expect(file.isUnstaged)
    }

    @Test("isUnstaged returns true for untracked file")
    func isUnstagedUntracked() {
        let file = makeStatusFile(xStatus: "?", yStatus: "?")
        #expect(file.isUnstaged)
    }

    @Test("isUnstaged returns false for staged-only file")
    func isUnstagedFalse() {
        let file = makeStatusFile(xStatus: "A", yStatus: " ")
        #expect(!file.isUnstaged)
    }

    @Test("statusText returns correct priority")
    func statusText() {
        #expect(makeStatusFile(xStatus: "A").statusText == "A")
        #expect(makeStatusFile(yStatus: "A").statusText == "A")
        #expect(makeStatusFile(xStatus: "D").statusText == "D")
        #expect(makeStatusFile(xStatus: "R").statusText == "R")
        #expect(makeStatusFile(xStatus: "C").statusText == "C")
        #expect(makeStatusFile(xStatus: "M").statusText == "M")
        #expect(makeStatusFile(xStatus: "U").statusText == "U")
        #expect(makeStatusFile(xStatus: " ", yStatus: " ").statusText == "?")
    }

    @Test("stagedStatusText returns xStatus as string")
    func stagedStatusText() {
        #expect(makeStatusFile(xStatus: "M").stagedStatusText == "M")
        #expect(makeStatusFile(xStatus: "A").stagedStatusText == "A")
    }

    @Test("unstagedStatusText returns yStatus or U for untracked")
    func unstagedStatusText() {
        #expect(makeStatusFile(yStatus: "M").unstagedStatusText == "M")
        #expect(makeStatusFile(yStatus: "D").unstagedStatusText == "D")
        #expect(makeStatusFile(xStatus: "?", yStatus: "?").unstagedStatusText == "U")
    }

    @Test("GitCommit.isMerge with single parent is false")
    func commitNotMerge() {
        let commit = GitCommit(
            hash: "abc123",
            shortHash: "abc",
            subject: "test",
            authorName: "Test",
            authorDate: Date(),
            refs: [],
            parentHashes: ["parent1"]
        )
        #expect(!commit.isMerge)
    }

    @Test("GitCommit.isMerge with multiple parents is true")
    func commitIsMerge() {
        let commit = GitCommit(
            hash: "abc123",
            shortHash: "abc",
            subject: "Merge branch",
            authorName: "Test",
            authorDate: Date(),
            refs: [],
            parentHashes: ["parent1", "parent2"]
        )
        #expect(commit.isMerge)
    }
}
