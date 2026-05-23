import Foundation

@MainActor
@Observable
final class DiffViewerTabState: Identifiable {
    let id = UUID()
    let vcs: VCSTabState
    let projectPath: String
    var mode: VCSTabState.ViewMode
    var selectedFilePath: String?
    var selectedIsStaged = false
    var wordWrap = false
    var fontSize: CGFloat = 13
    var scrollRequestVersion = 0
    var collapsedCacheKeys: Set<String> = []
    var manuallyLoadedCacheKeys: Set<String> = []
    var activeCacheKey: String?

    var displayTitle: String {
        "Git Diff"
    }

    var selectedDisplayTitle: String {
        guard let selectedFilePath else { return "No file selected" }
        return (selectedFilePath as NSString).lastPathComponent
    }

    var selectedCacheKey: String? {
        guard let selectedFilePath else { return nil }
        return Self.cacheKey(filePath: selectedFilePath, isStaged: selectedIsStaged)
    }

    init(vcs: VCSTabState, filePath: String? = nil, isStaged: Bool = false) {
        self.vcs = vcs
        projectPath = vcs.projectPath
        mode = vcs.mode
        selectInitialFile(filePath: filePath, isStaged: isStaged)
    }

    func refresh(forceFull: Bool) {
        loadAllDiffs(forceFull: forceFull)
    }

    func loadFullDiff(filePath: String, isStaged: Bool) {
        let cacheKey = Self.cacheKey(filePath: filePath, isStaged: isStaged)
        manuallyLoadedCacheKeys.insert(cacheKey)
        collapsedCacheKeys.remove(cacheKey)
        loadDiff(filePath: filePath, isStaged: isStaged, forceFull: true)
    }

    func select(filePath: String, isStaged: Bool) {
        guard selectedFilePath != filePath || selectedIsStaged != isStaged else {
            scrollRequestVersion &+= 1
            loadSelectedDiff(forceFull: false)
            return
        }
        selectedFilePath = filePath
        selectedIsStaged = isStaged
        activeCacheKey = Self.cacheKey(filePath: filePath, isStaged: isStaged)
        scrollRequestVersion &+= 1
        loadSelectedDiff(forceFull: false)
    }

    func loadAllDiffs(forceFull: Bool = false) {
        for file in vcs.stagedFiles {
            loadDiff(filePath: file.path, isStaged: true, forceFull: forceFull)
        }
        for file in vcs.unstagedFiles {
            loadDiff(filePath: file.path, isStaged: false, forceFull: forceFull)
        }
    }

    func adjustFontSize(by delta: CGFloat) {
        fontSize = min(28, max(9, fontSize + delta))
    }

    func resetFontSize() {
        fontSize = 13
    }

    func isCollapsed(filePath: String, isStaged: Bool) -> Bool {
        collapsedCacheKeys.contains(Self.cacheKey(filePath: filePath, isStaged: isStaged))
    }

    func toggleCollapsed(filePath: String, isStaged: Bool) {
        let cacheKey = Self.cacheKey(filePath: filePath, isStaged: isStaged)
        if isLargeUnloadedDiff(cacheKey) {
            loadFullDiff(filePath: filePath, isStaged: isStaged)
            return
        }
        if collapsedCacheKeys.contains(cacheKey) {
            collapsedCacheKeys.remove(cacheKey)
        } else {
            collapsedCacheKeys.insert(cacheKey)
        }
    }

    func collapseAll() {
        collapsedCacheKeys = allCacheKeys
    }

    func expandAll() {
        collapsedCacheKeys = Set(allCacheKeys.filter(isLargeUnloadedDiff))
    }

    func reconcileLargeDiffCollapse() {
        collapsedCacheKeys.formUnion(allCacheKeys.filter(isLargeUnloadedDiff))
    }

    func reconcileSelection() {
        if let selectedFilePath, contains(filePath: selectedFilePath, isStaged: selectedIsStaged) {
            loadSelectedDiff(forceFull: false)
            return
        }
        if let selectedFilePath, contains(filePath: selectedFilePath, isStaged: !selectedIsStaged) {
            select(filePath: selectedFilePath, isStaged: !selectedIsStaged)
            return
        }
        if let first = vcs.stagedFiles.first {
            select(filePath: first.path, isStaged: true)
            return
        }
        if let first = vcs.unstagedFiles.first {
            select(filePath: first.path, isStaged: false)
            return
        }
        selectedFilePath = nil
    }

    func diff() -> DiffCache.LoadedDiff? {
        guard let selectedCacheKey else { return nil }
        return vcs.diffCache.diff(for: selectedCacheKey)
    }

    func isLoading() -> Bool {
        guard let selectedCacheKey else { return false }
        return vcs.diffCache.isLoading(selectedCacheKey)
    }

    func error() -> String? {
        guard let selectedCacheKey else { return nil }
        return vcs.diffCache.error(for: selectedCacheKey)
    }

    private func selectInitialFile(filePath: String?, isStaged: Bool) {
        if let filePath, contains(filePath: filePath, isStaged: isStaged) {
            selectedFilePath = filePath
            selectedIsStaged = isStaged
            loadSelectedDiff(forceFull: false)
            return
        }
        reconcileSelection()
    }

    private func loadSelectedDiff(forceFull: Bool) {
        guard let selectedFilePath else { return }
        loadDiff(filePath: selectedFilePath, isStaged: selectedIsStaged, forceFull: forceFull)
    }

    private func loadDiff(filePath: String, isStaged: Bool, forceFull: Bool) {
        vcs.loadDiffWithHints(
            filePath: filePath,
            hints: diffHints(filePath: filePath, isStaged: isStaged),
            cacheKey: Self.cacheKey(filePath: filePath, isStaged: isStaged),
            pinnedPaths: allCacheKeys,
            forceFull: forceFull
        )
    }

    private var allCacheKeys: Set<String> {
        Set(vcs.stagedFiles.map { Self.cacheKey(filePath: $0.path, isStaged: true) } +
            vcs.unstagedFiles.map { Self.cacheKey(filePath: $0.path, isStaged: false) })
    }

    private func contains(filePath: String, isStaged: Bool) -> Bool {
        if isStaged {
            return vcs.stagedFiles.contains { $0.path == filePath }
        }
        return vcs.unstagedFiles.contains { $0.path == filePath }
    }

    private func diffHints(filePath: String, isStaged: Bool) -> GitRepositoryService.DiffHints {
        guard let file = vcs.files.first(where: { $0.path == filePath }) else {
            return GitRepositoryService.DiffHints(hasStaged: isStaged, hasUnstaged: !isStaged, isUntrackedOrNew: false)
        }
        let untrackedOrNew = (file.xStatus == "?" && file.yStatus == "?") || file.xStatus == "A"
        if isStaged {
            return GitRepositoryService.DiffHints(hasStaged: true, hasUnstaged: false, isUntrackedOrNew: untrackedOrNew)
        }
        return GitRepositoryService.DiffHints(hasStaged: false, hasUnstaged: !untrackedOrNew, isUntrackedOrNew: untrackedOrNew)
    }

    private func isLargeUnloadedDiff(_ cacheKey: String) -> Bool {
        vcs.diffCache.diff(for: cacheKey)?.truncated == true && !manuallyLoadedCacheKeys.contains(cacheKey)
    }

    static func cacheKey(filePath: String, isStaged: Bool) -> String {
        "\(isStaged ? "staged" : "unstaged"):\(filePath)"
    }
}
