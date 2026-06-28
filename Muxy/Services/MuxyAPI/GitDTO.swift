import Foundation

enum GitDTO {
    private static let isoFormat = Date.ISO8601FormatStyle()

    static func status(_ snapshot: GitStatusSnapshot) -> [String: Any] {
        [
            "branch": snapshot.branch,
            "aheadBehind": aheadBehind(snapshot.aheadBehind),
            "defaultBranch": snapshot.defaultBranch ?? NSNull(),
            "branches": snapshot.branches,
            "stagedFiles": snapshot.stagedFiles.map { file($0, staged: true) },
            "unstagedFiles": snapshot.unstagedFiles.map { file($0, staged: false) },
            "pullRequest": snapshot.pullRequest.map(prInfo) ?? NSNull(),
        ]
    }

    static func aheadBehind(_ value: GitRepositoryService.AheadBehind) -> [String: Any] {
        [
            "ahead": value.ahead,
            "behind": value.behind,
            "hasUpstream": value.hasUpstream,
        ]
    }

    static func file(_ file: GitStatusFile, staged: Bool) -> [String: Any] {
        [
            "path": file.path,
            "oldPath": file.oldPath ?? NSNull(),
            "status": String(staged ? file.xStatus : file.yStatus),
            "isStaged": file.isStaged,
            "isUnstaged": file.isUnstaged,
            "isBinary": file.isBinary,
            "additions": file.additions(isStaged: staged) ?? NSNull(),
            "deletions": file.deletions(isStaged: staged) ?? NSNull(),
        ]
    }

    static func repoInfo(_ info: GitRepositoryService.RepoInfo) -> [String: Any] {
        [
            "root": info.root,
            "gitDir": info.gitDir,
            "isWorktree": info.isWorktree,
            "currentBranch": info.currentBranch,
        ]
    }

    static func rawDiff(_ result: GitRepositoryService.RawDiffResult) -> [String: Any] {
        [
            "diff": result.diff,
            "truncated": result.truncated,
        ]
    }

    static func diff(_ result: GitRepositoryService.PatchAndCompareResult) -> [String: Any] {
        [
            "additions": result.additions,
            "deletions": result.deletions,
            "truncated": result.truncated,
            "rows": result.rows.map(diffRow),
        ]
    }

    static func diffRow(_ row: DiffDisplayRow) -> [String: Any] {
        [
            "kind": diffRowKind(row.kind),
            "oldLineNumber": row.oldLineNumber ?? NSNull(),
            "newLineNumber": row.newLineNumber ?? NSNull(),
            "oldText": row.oldText ?? NSNull(),
            "newText": row.newText ?? NSNull(),
            "text": row.text,
        ]
    }

    static func commit(_ commit: GitCommit) -> [String: Any] {
        [
            "hash": commit.hash,
            "shortHash": commit.shortHash,
            "subject": commit.subject,
            "authorName": commit.authorName,
            "authorDate": iso(commit.authorDate),
            "isMerge": commit.isMerge,
            "parentHashes": commit.parentHashes,
            "refs": commit.refs.map { ["name": $0.name, "kind": refKind($0.kind)] },
        ]
    }

    static func prInfo(_ info: GitRepositoryService.PRInfo) -> [String: Any] {
        [
            "url": info.url,
            "number": info.number,
            "state": info.state.rawValue,
            "isDraft": info.isDraft,
            "baseBranch": info.baseBranch,
            "mergeable": info.mergeable ?? NSNull(),
            "mergeStateStatus": info.mergeStateStatus.rawValue,
            "isCrossRepository": info.isCrossRepository,
            "checks": checks(info.checks),
        ]
    }

    static func prListItem(_ item: GitRepositoryService.PRListItem) -> [String: Any] {
        [
            "number": item.number,
            "title": item.title,
            "author": item.author,
            "headBranch": item.headBranch,
            "baseBranch": item.baseBranch,
            "state": item.state.rawValue,
            "isDraft": item.isDraft,
            "url": item.url,
            "updatedAt": item.updatedAt.map(iso) ?? NSNull(),
            "mergeable": item.mergeable ?? NSNull(),
            "mergeStateStatus": item.mergeStateStatus.rawValue,
            "checks": checks(item.checks),
        ]
    }

    static func worktree(_ record: GitWorktreeRecord) -> [String: Any] {
        [
            "path": record.path,
            "branch": record.branch ?? NSNull(),
            "head": record.head ?? NSNull(),
            "isBare": record.isBare,
            "isDetached": record.isDetached,
            "isPrunable": record.isPrunable,
        ]
    }

    private static func checks(_ checks: GitRepositoryService.PRChecks) -> [String: Any] {
        [
            "status": checksStatus(checks.status),
            "passing": checks.passing,
            "failing": checks.failing,
            "pending": checks.pending,
            "total": checks.total,
        ]
    }

    private static func checksStatus(_ status: GitRepositoryService.PRChecksStatus) -> String {
        switch status {
        case .none: "none"
        case .pending: "pending"
        case .success: "success"
        case .failure: "failure"
        }
    }

    private static func diffRowKind(_ kind: DiffDisplayRow.Kind) -> String {
        switch kind {
        case .hunk: "hunk"
        case .context,
             .commentSpacer: "context"
        case .addition: "addition"
        case .deletion: "deletion"
        case .collapsed: "collapsed"
        }
    }

    private static func refKind(_ kind: GitRef.Kind) -> String {
        switch kind {
        case .localBranch: "localBranch"
        case .remoteBranch: "remoteBranch"
        case .tag: "tag"
        case .head: "head"
        }
    }

    private static func iso(_ date: Date) -> String {
        date.formatted(isoFormat)
    }
}
