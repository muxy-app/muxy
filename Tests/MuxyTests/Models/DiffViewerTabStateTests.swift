import Foundation
import Testing

@testable import Muxy

@Suite("DiffViewerTabState")
@MainActor
struct DiffViewerTabStateTests {
    private func makeFile(path: String, xStatus: Character, yStatus: Character) -> GitStatusFile {
        GitStatusFile(path: path, oldPath: nil, xStatus: xStatus, yStatus: yStatus, additions: 1, deletions: 0, isBinary: false)
    }

    private func makeDiff() -> DiffCache.LoadedDiff {
        DiffCache.LoadedDiff(rows: [], additions: 1, deletions: 0, truncated: false)
    }

    @Test("cache key separates staged and unstaged variants")
    func cacheKeySeparatesVariants() {
        #expect(DiffViewerTabState.cacheKey(filePath: "Sources/App.swift", isStaged: true) == "staged:Sources/App.swift")
        #expect(DiffViewerTabState.cacheKey(filePath: "Sources/App.swift", isStaged: false) == "unstaged:Sources/App.swift")
    }

    @Test("reconcile preserves selected path across staged buckets")
    func reconcilePreservesSelectedPathAcrossStagedBuckets() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let filePath = "Sources/App.swift"
        let cacheKey = DiffViewerTabState.cacheKey(filePath: filePath, isStaged: false)

        state.selectedFilePath = filePath
        state.selectedIsStaged = true
        vcs.files = [
            makeFile(path: "Sources/Other.swift", xStatus: "M", yStatus: " "),
            makeFile(path: filePath, xStatus: " ", yStatus: "M"),
        ]
        vcs.diffCache.store(makeDiff(), for: cacheKey, pinnedPaths: [])

        state.reconcileSelection()

        #expect(state.selectedFilePath == filePath)
        #expect(state.selectedIsStaged == false)
        #expect(!vcs.diffCache.isLoading(cacheKey))
        vcs.diffCache.cancelAll()
    }

    @Test("select uses cached diff without reloading")
    func selectUsesCachedDiffWithoutReloading() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let filePath = "Sources/App.swift"
        let cacheKey = DiffViewerTabState.cacheKey(filePath: filePath, isStaged: false)

        vcs.files = [makeFile(path: filePath, xStatus: " ", yStatus: "M")]
        vcs.diffCache.store(makeDiff(), for: cacheKey, pinnedPaths: [])

        state.select(filePath: filePath, isStaged: false)

        #expect(state.diff() != nil)
        #expect(!vcs.diffCache.isLoading(cacheKey))
        #expect(vcs.diffCache.error(for: cacheKey) == nil)
        vcs.diffCache.cancelAll()
    }

    @Test("reconcile reloads selected diff after cache eviction")
    func reconcileReloadsSelectedDiffAfterCacheEviction() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let filePath = "Sources/App.swift"
        let cacheKey = DiffViewerTabState.cacheKey(filePath: filePath, isStaged: false)

        vcs.files = [makeFile(path: filePath, xStatus: " ", yStatus: "M")]
        state.selectedFilePath = filePath
        state.selectedIsStaged = false

        state.reconcileSelection()

        #expect(vcs.diffCache.isLoading(cacheKey))
        vcs.diffCache.cancelAll()
    }

    @Test("word wrap stays enabled across file selection")
    func wordWrapStaysEnabledAcrossFileSelection() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let firstPath = "Sources/App.swift"
        let secondPath = "Sources/Other.swift"

        vcs.files = [
            makeFile(path: firstPath, xStatus: " ", yStatus: "M"),
            makeFile(path: secondPath, xStatus: " ", yStatus: "M"),
        ]
        vcs.diffCache.store(makeDiff(), for: DiffViewerTabState.cacheKey(filePath: secondPath, isStaged: false), pinnedPaths: [])
        state.wordWrap = true

        state.select(filePath: secondPath, isStaged: false)

        #expect(state.wordWrap)
        #expect(state.selectedFilePath == secondPath)
        vcs.diffCache.cancelAll()
    }

    @Test("font size is diff specific and resettable")
    func fontSizeIsDiffSpecificAndResettable() {
        let state = DiffViewerTabState(vcs: VCSTabState(projectPath: NSTemporaryDirectory()))

        #expect(state.fontSize == 13)
        state.adjustFontSize(by: 4)
        #expect(state.fontSize == 17)
        state.resetFontSize()
        #expect(state.fontSize == 13)
    }

    @Test("selecting current file still emits scroll request")
    func selectingCurrentFileStillEmitsScrollRequest() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let filePath = "Sources/App.swift"
        let cacheKey = DiffViewerTabState.cacheKey(filePath: filePath, isStaged: false)

        vcs.files = [makeFile(path: filePath, xStatus: " ", yStatus: "M")]
        vcs.diffCache.store(makeDiff(), for: cacheKey, pinnedPaths: [])
        state.select(filePath: filePath, isStaged: false)
        let firstVersion = state.scrollRequestVersion

        state.select(filePath: filePath, isStaged: false)

        #expect(state.scrollRequestVersion == firstVersion + 1)
        vcs.diffCache.cancelAll()
    }

    @Test("collapse state supports file and global toggles")
    func collapseStateSupportsFileAndGlobalToggles() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let firstPath = "Sources/App.swift"
        let secondPath = "Sources/Other.swift"

        vcs.files = [
            makeFile(path: firstPath, xStatus: " ", yStatus: "M"),
            makeFile(path: secondPath, xStatus: "M", yStatus: " "),
        ]

        state.toggleCollapsed(filePath: firstPath, isStaged: false)
        #expect(state.isCollapsed(filePath: firstPath, isStaged: false))
        state.expandAll()
        #expect(!state.isCollapsed(filePath: firstPath, isStaged: false))
        state.collapseAll()
        #expect(state.isCollapsed(filePath: firstPath, isStaged: false))
        #expect(state.isCollapsed(filePath: secondPath, isStaged: true))
    }

    @Test("tab area reuses one diff viewer tab per project")
    func tabAreaReusesSingleDiffViewerTab() {
        let projectPath = NSTemporaryDirectory()
        let vcs = VCSTabState(projectPath: projectPath)
        vcs.files = [
            makeFile(path: "a.swift", xStatus: " ", yStatus: "M"),
            makeFile(path: "b.swift", xStatus: " ", yStatus: "M"),
        ]
        let area = TabArea(projectPath: projectPath)

        area.createDiffViewerTab(vcs: vcs, filePath: "a.swift", isStaged: false)
        area.createDiffViewerTab(vcs: vcs, filePath: "b.swift", isStaged: false)

        let diffTabs = area.tabs.compactMap(\.content.diffViewerState)
        #expect(diffTabs.count == 1)
        #expect(diffTabs.first?.selectedFilePath == "b.swift")
        #expect(diffTabs.first?.selectedIsStaged == false)
        vcs.diffCache.cancelAll()
    }
}
