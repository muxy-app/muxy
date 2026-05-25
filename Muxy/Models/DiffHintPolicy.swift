enum DiffHintPolicy {
    static func hints(file: GitStatusFile, isStaged: Bool) -> GitRepositoryService.DiffHints {
        let isUntracked = file.xStatus == "?" && file.yStatus == "?"
        if isStaged {
            return GitRepositoryService.DiffHints(hasStaged: true, hasUnstaged: false, isUntrackedOrNew: isUntracked)
        }
        return GitRepositoryService.DiffHints(hasStaged: false, hasUnstaged: !isUntracked, isUntrackedOrNew: isUntracked)
    }
}
