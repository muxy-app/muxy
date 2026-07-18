import Foundation

struct RepositoryChangesLineStats: Equatable {
    let additions: Int
    let deletions: Int
    let hasKnownValues: Bool

    func merging(_ other: RepositoryChangesLineStats) -> RepositoryChangesLineStats {
        RepositoryChangesLineStats(
            additions: additions + other.additions,
            deletions: deletions + other.deletions,
            hasKnownValues: hasKnownValues || other.hasKnownValues
        )
    }
}

struct RepositoryChangesDiscardRequest: Equatable {
    let paths: [String]
    let untrackedPaths: [String]
}

struct RepositoryChangesSnapshot: Equatable {
    let fileCount: Int
    let conflictedFiles: [GitStatusFile]
    let stagedFiles: [GitStatusFile]
    let unstagedFiles: [GitStatusFile]
    let totalLineStats: RepositoryChangesLineStats
    let conflictedLineStats: RepositoryChangesLineStats
    let stagedLineStats: RepositoryChangesLineStats
    let unstagedLineStats: RepositoryChangesLineStats

    var isEmpty: Bool { fileCount == 0 }

    static let empty = RepositoryChangesSnapshot(
        fileCount: 0,
        conflictedFiles: [],
        stagedFiles: [],
        unstagedFiles: [],
        totalLineStats: RepositoryChangesLineStats(additions: 0, deletions: 0, hasKnownValues: false),
        conflictedLineStats: RepositoryChangesLineStats(additions: 0, deletions: 0, hasKnownValues: false),
        stagedLineStats: RepositoryChangesLineStats(additions: 0, deletions: 0, hasKnownValues: false),
        unstagedLineStats: RepositoryChangesLineStats(additions: 0, deletions: 0, hasKnownValues: false)
    )
}

enum RepositoryChangesPresentation {
    static func loadSnapshot(_ files: [GitStatusFile]) async -> RepositoryChangesSnapshot {
        makeSnapshot(files)
    }

    static func makeSnapshot(_ files: [GitStatusFile]) -> RepositoryChangesSnapshot {
        var conflictedFiles: [GitStatusFile] = []
        var stagedFiles: [GitStatusFile] = []
        var unstagedFiles: [GitStatusFile] = []
        var totalLineStats = RepositoryChangesSnapshot.empty.totalLineStats
        var conflictedLineStats = RepositoryChangesSnapshot.empty.conflictedLineStats
        var stagedLineStats = RepositoryChangesSnapshot.empty.stagedLineStats
        var unstagedLineStats = RepositoryChangesSnapshot.empty.unstagedLineStats

        for file in files {
            totalLineStats = totalLineStats.merging(lineStats(file, staged: nil))
            if file.isConflicted {
                conflictedFiles.append(file)
                conflictedLineStats = conflictedLineStats.merging(lineStats(file, staged: nil))
                continue
            }
            if file.isStaged {
                stagedFiles.append(file)
                stagedLineStats = stagedLineStats.merging(lineStats(file, staged: true))
            }
            if file.isUnstaged {
                unstagedFiles.append(file)
                unstagedLineStats = unstagedLineStats.merging(lineStats(file, staged: false))
            }
        }

        return RepositoryChangesSnapshot(
            fileCount: files.count,
            conflictedFiles: conflictedFiles,
            stagedFiles: stagedFiles,
            unstagedFiles: unstagedFiles,
            totalLineStats: totalLineStats,
            conflictedLineStats: conflictedLineStats,
            stagedLineStats: stagedLineStats,
            unstagedLineStats: unstagedLineStats
        )
    }

    static func chipLabel(_ summary: GitRepositorySummary) -> String {
        guard summary.changedCount > 0 else { return "Clean" }
        return summary.changedCount == 1 ? "1 change" : "\(summary.changedCount) changes"
    }

    static func discardRequest(_ file: GitStatusFile) -> RepositoryChangesDiscardRequest? {
        guard !file.isConflicted else { return nil }
        if file.isUntracked {
            return RepositoryChangesDiscardRequest(paths: [], untrackedPaths: [file.path])
        }
        if file.xStatus == " ", file.yStatus == "R", let oldPath = file.oldPath {
            return RepositoryChangesDiscardRequest(paths: [oldPath], untrackedPaths: [file.path])
        }
        if file.xStatus == " ", file.yStatus == "C" {
            return RepositoryChangesDiscardRequest(paths: [], untrackedPaths: [file.path])
        }
        return RepositoryChangesDiscardRequest(paths: [file.path], untrackedPaths: [])
    }

    static func lineStats(_ files: [GitStatusFile], staged: Bool? = nil) -> RepositoryChangesLineStats {
        var additions = 0
        var deletions = 0
        var hasKnownValues = false
        for file in files {
            let stats = lineStats(file, staged: staged)
            if stats.hasKnownValues {
                additions += stats.additions
                deletions += stats.deletions
                hasKnownValues = true
            }
        }
        return RepositoryChangesLineStats(
            additions: additions,
            deletions: deletions,
            hasKnownValues: hasKnownValues
        )
    }

    static func lineStats(_ file: GitStatusFile, staged: Bool?) -> RepositoryChangesLineStats {
        let additions = staged.map { file.additions(isStaged: $0) } ?? file.additions
        let deletions = staged.map { file.deletions(isStaged: $0) } ?? file.deletions
        return RepositoryChangesLineStats(
            additions: additions ?? 0,
            deletions: deletions ?? 0,
            hasKnownValues: additions != nil || deletions != nil
        )
    }
}
