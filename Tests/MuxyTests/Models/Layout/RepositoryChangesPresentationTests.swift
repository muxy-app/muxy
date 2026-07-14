import Testing

@testable import Muxy

@Suite("RepositoryChangesPresentation")
struct RepositoryChangesPresentationTests {
    @Test("chip label distinguishes clean, singular, and plural changes")
    func chipLabels() {
        #expect(RepositoryChangesPresentation.chipLabel(summary(changedCount: 0)) == "Clean")
        #expect(RepositoryChangesPresentation.chipLabel(summary(changedCount: 1)) == "1 change")
        #expect(RepositoryChangesPresentation.chipLabel(summary(changedCount: 3)) == "3 changes")
    }

    @Test("groups staged, unstaged, both-sided, and conflicted files")
    func groupsFiles() {
        let staged = file(path: "staged.swift", xStatus: "M")
        let unstaged = file(path: "unstaged.swift", yStatus: "M")
        let both = file(path: "both.swift", xStatus: "M", yStatus: "M")
        let conflict = file(path: "conflict.swift", xStatus: "U", yStatus: "U")
        let files = [staged, unstaged, both, conflict]
        let snapshot = RepositoryChangesPresentation.makeSnapshot(files)

        #expect(snapshot.fileCount == files.count)
        #expect(snapshot.stagedFiles.map(\.path) == ["staged.swift", "both.swift"])
        #expect(snapshot.unstagedFiles.map(\.path) == ["unstaged.swift", "both.swift"])
        #expect(snapshot.conflictedFiles.map(\.path) == ["conflict.swift"])
    }

    @Test("groups staged and unstaged file-type changes")
    func groupsTypeChanges() {
        let staged = file(path: "staged-link", xStatus: "T")
        let unstaged = file(path: "unstaged-link", yStatus: "T")
        let snapshot = RepositoryChangesPresentation.makeSnapshot([staged, unstaged])

        #expect(snapshot.stagedFiles.map(\.path) == ["staged-link"])
        #expect(snapshot.unstagedFiles.map(\.path) == ["unstaged-link"])
    }

    @Test("line stats use the selected side of a both-sided file")
    func sideSpecificLineStats() {
        let file = GitStatusFile(
            path: "both.swift",
            oldPath: nil,
            xStatus: "M",
            yStatus: "M",
            additions: 12,
            deletions: 7,
            stagedAdditions: 3,
            stagedDeletions: 2,
            unstagedAdditions: 9,
            unstagedDeletions: 5,
            isBinary: false
        )

        #expect(RepositoryChangesPresentation.lineStats([file]) == RepositoryChangesLineStats(
            additions: 12,
            deletions: 7,
            hasKnownValues: true
        ))
        #expect(RepositoryChangesPresentation.lineStats([file], staged: true) == RepositoryChangesLineStats(
            additions: 3,
            deletions: 2,
            hasKnownValues: true
        ))
        #expect(RepositoryChangesPresentation.lineStats([file], staged: false) == RepositoryChangesLineStats(
            additions: 9,
            deletions: 5,
            hasKnownValues: true
        ))
        let snapshot = RepositoryChangesPresentation.makeSnapshot([file])
        #expect(snapshot.totalLineStats == RepositoryChangesLineStats(
            additions: 12,
            deletions: 7,
            hasKnownValues: true
        ))
        #expect(snapshot.stagedLineStats == RepositoryChangesLineStats(
            additions: 3,
            deletions: 2,
            hasKnownValues: true
        ))
        #expect(snapshot.unstagedLineStats == RepositoryChangesLineStats(
            additions: 9,
            deletions: 5,
            hasKnownValues: true
        ))
    }

    @Test("discard request separates tracked and untracked paths")
    func discardRequests() {
        #expect(RepositoryChangesPresentation.discardRequest(file(
            path: "tracked.swift",
            yStatus: "M"
        )) == RepositoryChangesDiscardRequest(paths: ["tracked.swift"], untrackedPaths: []))
        #expect(RepositoryChangesPresentation.discardRequest(file(
            path: "new.swift",
            xStatus: "?",
            yStatus: "?"
        )) == RepositoryChangesDiscardRequest(paths: [], untrackedPaths: ["new.swift"]))
    }

    @Test("discard request restores an unstaged rename without losing the original")
    func discardUnstagedRename() {
        let rename = file(path: "new.swift", oldPath: "old.swift", yStatus: "R")

        #expect(RepositoryChangesPresentation.discardRequest(rename) == RepositoryChangesDiscardRequest(
            paths: ["old.swift"],
            untrackedPaths: ["new.swift"]
        ))
        #expect(RepositoryChangesPresentation.discardRequest(file(
            path: "conflict.swift",
            xStatus: "U",
            yStatus: "U"
        )) == nil)
    }

    private func summary(changedCount: Int) -> GitRepositorySummary {
        GitRepositorySummary(
            branch: "main",
            headOID: "abc",
            isDetached: false,
            aheadBehind: GitRepositoryService.AheadBehind(ahead: 0, behind: 0, hasUpstream: true),
            changedCount: changedCount,
            stagedCount: 0,
            unstagedCount: changedCount,
            untrackedCount: 0
        )
    }

    private func file(
        path: String,
        oldPath: String? = nil,
        xStatus: Character = " ",
        yStatus: Character = " "
    ) -> GitStatusFile {
        GitStatusFile(
            path: path,
            oldPath: oldPath,
            xStatus: xStatus,
            yStatus: yStatus,
            additions: nil,
            deletions: nil,
            isBinary: false
        )
    }
}
