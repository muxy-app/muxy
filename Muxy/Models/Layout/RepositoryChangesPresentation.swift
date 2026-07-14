import Foundation

struct RepositoryChangesLineStats: Equatable {
    let additions: Int
    let deletions: Int
    let hasKnownValues: Bool
}

struct RepositoryChangesDiscardRequest: Equatable {
    let paths: [String]
    let untrackedPaths: [String]
}

enum RepositoryChangesPresentation {
    static func chipLabel(_ summary: GitRepositorySummary) -> String {
        guard summary.changedCount > 0 else { return "Clean" }
        return summary.changedCount == 1 ? "1 change" : "\(summary.changedCount) changes"
    }

    static func stagedFiles(_ files: [GitStatusFile]) -> [GitStatusFile] {
        files.filter { $0.isStaged && !$0.isConflicted }
    }

    static func unstagedFiles(_ files: [GitStatusFile]) -> [GitStatusFile] {
        files.filter { $0.isUnstaged && !$0.isConflicted }
    }

    static func conflictedFiles(_ files: [GitStatusFile]) -> [GitStatusFile] {
        files.filter(\.isConflicted)
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
            let fileAdditions = staged.map { file.additions(isStaged: $0) } ?? file.additions
            let fileDeletions = staged.map { file.deletions(isStaged: $0) } ?? file.deletions
            if let fileAdditions {
                additions += fileAdditions
                hasKnownValues = true
            }
            if let fileDeletions {
                deletions += fileDeletions
                hasKnownValues = true
            }
        }
        return RepositoryChangesLineStats(
            additions: additions,
            deletions: deletions,
            hasKnownValues: hasKnownValues
        )
    }
}
