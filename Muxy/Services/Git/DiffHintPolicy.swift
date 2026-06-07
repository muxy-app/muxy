enum DiffHintPolicy {
    static func hints(file: GitStatusFile, isStaged: Bool) -> GitRepositoryService.DiffHints {
        let isUntrackedOrNew = (file.xStatus == "?" && file.yStatus == "?") || (!isStaged && file.xStatus == "A")
        if isStaged {
            return GitRepositoryService.DiffHints(hasStaged: true, hasUnstaged: false, isUntrackedOrNew: isUntrackedOrNew)
        }
        return GitRepositoryService.DiffHints(hasStaged: false, hasUnstaged: !isUntrackedOrNew, isUntrackedOrNew: isUntrackedOrNew)
    }
}
