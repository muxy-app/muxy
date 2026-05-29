import Foundation
import Testing

@testable import Muxy

@Suite("DiffViewerTabState")
@MainActor
struct DiffViewerTabStateTests {
    private func makeFile(path: String, xStatus: Character, yStatus: Character) -> GitStatusFile {
        GitStatusFile(path: path, oldPath: nil, xStatus: xStatus, yStatus: yStatus, additions: 1, deletions: 0, isBinary: false)
    }

    private func makeFile(path: String, xStatus: Character, yStatus: Character, additions: Int, deletions: Int) -> GitStatusFile {
        GitStatusFile(path: path, oldPath: nil, xStatus: xStatus, yStatus: yStatus, additions: additions, deletions: deletions, isBinary: false)
    }

    private func makeDiff() -> DiffCache.LoadedDiff {
        DiffCache.LoadedDiff(rows: [], additions: 1, deletions: 0, truncated: false)
    }

    private func makeTruncatedDiff() -> DiffCache.LoadedDiff {
        DiffCache.LoadedDiff(rows: [], additions: 1, deletions: 0, truncated: true)
    }

    @Test("cache key separates staged and unstaged variants")
    func cacheKeySeparatesVariants() {
        #expect(DiffViewerTabState.cacheKey(filePath: "Sources/App.swift", isStaged: true) == "staged:Sources/App.swift")
        #expect(DiffViewerTabState.cacheKey(filePath: "Sources/App.swift", isStaged: false) == "unstaged:Sources/App.swift")
    }

    @Test("commit source title and link include commit")
    func commitSourceTitleAndLinkIncludeCommit() throws {
        let url = try #require(URL(string: "https://github.com/muxy-app/muxy/commit/1234567890abcdef"))
        let source = DiffViewerTabState.Source.commit(DiffViewerTabState.CommitSource(
            hash: "1234567890abcdef",
            subject: "Fix diff viewer",
            webURL: url
        ))

        #expect(source.displayTitle == "Commit 1234567 Diff")
        #expect(source.link?.title == "Commit 1234567")
        #expect(source.link?.url == url)
    }

    @Test("pull request source title and link include pull request")
    func pullRequestSourceTitleAndLinkIncludePullRequest() throws {
        let url = try #require(URL(string: "https://github.com/muxy-app/muxy/pull/535"))
        let source = DiffViewerTabState.Source.pullRequest(DiffViewerTabState.PullRequestSource(
            number: 535,
            title: "New git diff viewer",
            baseRef: "refs/remotes/origin/main",
            headRef: "refs/muxy/pull/535/head",
            baseBranch: "main",
            webURL: url
        ))

        #expect(source.displayTitle == "PR #535 Diff")
        #expect(source.link?.title == "PR #535")
        #expect(source.link?.url == url)
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

    @Test("font size is shared and persisted across diff viewers")
    func fontSizeIsSharedAndPersistedAcrossDiffViewers() {
        UserDefaults.standard.removeObject(forKey: DiffViewerTabState.fontSizeDefaultsKey)
        let first = DiffViewerTabState(vcs: VCSTabState(projectPath: NSTemporaryDirectory()))
        let second = DiffViewerTabState(vcs: VCSTabState(projectPath: NSTemporaryDirectory()))

        #expect(first.fontSize == 13)
        #expect(second.fontSize == 13)

        first.adjustFontSize(by: 4)

        #expect(first.fontSize == 17)
        #expect(second.fontSize == 17)
        #expect(UserDefaults.standard.double(forKey: DiffViewerTabState.fontSizeDefaultsKey) == 17)
        #expect(DiffViewerTabState(vcs: VCSTabState(projectPath: NSTemporaryDirectory())).fontSize == 17)

        second.resetFontSize()

        #expect(first.fontSize == 13)
        #expect(second.fontSize == 13)
        UserDefaults.standard.removeObject(forKey: DiffViewerTabState.fontSizeDefaultsKey)
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
        #expect(state.activeCacheKey == cacheKey)
        vcs.diffCache.cancelAll()
    }

    @Test("sidebar auto scroll only follows diff scroll activation")
    func sidebarAutoScrollOnlyFollowsDiffScrollActivation() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let firstPath = "Sources/App.swift"
        let secondPath = "Sources/Other.swift"
        let secondCacheKey = DiffViewerTabState.cacheKey(filePath: secondPath, isStaged: false)

        vcs.files = [
            makeFile(path: firstPath, xStatus: " ", yStatus: "M"),
            makeFile(path: secondPath, xStatus: " ", yStatus: "M"),
        ]
        vcs.diffCache.store(makeDiff(), for: secondCacheKey, pinnedPaths: [])

        state.select(filePath: secondPath, isStaged: false)

        #expect(state.activeCacheKey == secondCacheKey)
        #expect(state.sidebarScrollRequestVersion == 0)

        state.activateFromDiffScroll(cacheKey: DiffViewerTabState.cacheKey(filePath: firstPath, isStaged: false))

        #expect(state.activeCacheKey == secondCacheKey)
        #expect(state.sidebarScrollRequestVersion == 0)

        state.activateFromDiffScroll(cacheKey: secondCacheKey)
        state.activateFromDiffScroll(cacheKey: DiffViewerTabState.cacheKey(filePath: firstPath, isStaged: false))

        #expect(state.sidebarScrollRequestVersion == 1)
        vcs.diffCache.cancelAll()
    }

    @Test("source diffs reconcile against source files")
    func sourceDiffsReconcileAgainstSourceFiles() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        state.source = .range(baseRef: "main", headRef: "feature", title: "Feature")
        vcs.files = [makeFile(path: "WorkingTree.swift", xStatus: " ", yStatus: "M")]
        state.sourceFiles = [makeFile(path: "SourceDiff.swift", xStatus: "M", yStatus: " ")]

        state.reconcileSelection()

        #expect(state.selectedFilePath == "SourceDiff.swift")
        #expect(state.selectedIsStaged == false)
        vcs.diffCache.cancelAll()
        state.diffCache.cancelAll()
    }

    @Test("commit source loads changed files and source diff")
    func commitSourceLoadsChangedFilesAndSourceDiff() async throws {
        let repo = try DiffViewerGitFixture()
        defer { repo.cleanup() }

        try repo.write("App.swift", contents: "let value = 1\n")
        _ = try repo.commit(file: "App.swift", message: "base")
        try repo.write("App.swift", contents: "let value = 2\n")
        let commit = try repo.commit(file: "App.swift", message: "change")

        let vcs = VCSTabState(projectPath: repo.path)
        let state = DiffViewerTabState(
            vcs: vcs,
            source: .commit(.init(hash: commit, subject: "change", webURL: nil))
        )

        state.refresh(forceFull: false)
        try await waitForSourceFiles(state)

        #expect(state.files.map(\.path) == ["App.swift"])
        #expect(state.stagedFiles.isEmpty)
        #expect(state.unstagedFiles.map(\.path) == ["App.swift"])
        #expect(state.selectedFilePath == "App.swift")
        #expect(state.selectedDisplayTitle == "App.swift")

        try await waitForDiff(state)

        #expect(state.diff()?.additions == 1)
        #expect(state.diff()?.deletions == 1)
        #expect(!state.isLoading())
        #expect(state.error() == nil)
        state.prepareForClose()
    }

    @Test("range and pull request source paths handle file loading variants")
    func rangeAndPullRequestSourcesHandleFileLoadingVariants() async throws {
        let repo = try DiffViewerGitFixture()
        defer { repo.cleanup() }

        try repo.write("App.swift", contents: "let value = 1\n")
        let base = try repo.commit(file: "App.swift", message: "base")
        try repo.write("App.swift", contents: "let value = 2\n")
        let head = try repo.commit(file: "App.swift", message: "head")

        let vcs = VCSTabState(projectPath: repo.path)
        let state = DiffViewerTabState(
            vcs: vcs,
            source: .range(baseRef: base, headRef: head, title: "Feature Diff")
        )

        #expect(state.displayTitle == "Feature Diff")
        #expect(state.source.link == nil)
        #expect(state.selectedDisplayTitle == "No file selected")

        state.refresh(forceFull: true)
        try await waitForSourceFiles(state)
        try await waitForDiff(state)
        #expect(state.diff()?.truncated == false)

        state.setSource(.pullRequest(.init(
            number: 1,
            title: "PR",
            baseRef: nil,
            headRef: nil,
            baseBranch: "main",
            webURL: nil
        )))
        try await waitForSourceLoadFinished(state)
        #expect(state.sourceFiles.isEmpty)
        #expect(state.filesError != nil || state.selectedFilePath == nil)
        state.prepareForClose()
    }

    @Test("prepareForClose clears source and working tree diff loads")
    func prepareForCloseClearsSourceAndWorkingTreeDiffLoads() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let sourceKey = DiffViewerTabState.cacheKey(filePath: "SourceDiff.swift", isStaged: false)
        let workingTreeKey = DiffViewerTabState.cacheKey(filePath: "WorkingTree.swift", isStaged: false)

        state.diffCache.markLoading(sourceKey)
        state.diffCache.store(makeDiff(), for: sourceKey, pinnedPaths: [])
        vcs.diffCache.store(makeDiff(), for: workingTreeKey, pinnedPaths: [])
        vcs.diffCache.store(makeDiff(), for: "WorkingTree.swift", pinnedPaths: [])

        state.prepareForClose()

        #expect(!state.diffCache.hasDiff(for: sourceKey))
        #expect(!state.diffCache.isLoading(sourceKey))
        #expect(!vcs.diffCache.hasDiff(for: workingTreeKey))
        #expect(vcs.diffCache.hasDiff(for: "WorkingTree.swift"))
        vcs.diffCache.cancelAll()
        state.diffCache.cancelAll()
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

    @Test("large preview diffs stay collapsed until explicitly loaded")
    func largePreviewDiffsStayCollapsedUntilExplicitlyLoaded() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let filePath = "Sources/Large.swift"
        let cacheKey = DiffViewerTabState.cacheKey(filePath: filePath, isStaged: false)

        vcs.files = [makeFile(path: filePath, xStatus: " ", yStatus: "M")]
        vcs.diffCache.store(makeTruncatedDiff(), for: cacheKey, pinnedPaths: [])

        state.reconcileLargeDiffCollapse()

        #expect(state.isCollapsed(filePath: filePath, isStaged: false))

        state.expandAll()

        #expect(state.isCollapsed(filePath: filePath, isStaged: false))

        state.loadFullDiff(filePath: filePath, isStaged: false)

        #expect(!state.isCollapsed(filePath: filePath, isStaged: false))
        #expect(state.manuallyLoadedCacheKeys.contains(cacheKey))
        #expect(vcs.diffCache.isLoading(cacheKey))
        vcs.diffCache.cancelAll()
    }

    @Test("large diff stats collapse before preview loads")
    func largeDiffStatsCollapseBeforePreviewLoads() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let filePath = "Generated.swift"

        vcs.files = [makeFile(path: filePath, xStatus: " ", yStatus: "M", additions: 900, deletions: 100)]
        state.reconcileLargeDiffCollapse()

        #expect(state.isCollapsed(filePath: filePath, isStaged: false))
        vcs.diffCache.cancelAll()
    }

    @Test("staged added file loads through new file preview path")
    func stagedAddedFileLoadsThroughNewFilePreviewPath() {
        let vcs = VCSTabState(projectPath: NSTemporaryDirectory())
        let state = DiffViewerTabState(vcs: vcs)
        let filePath = "Large.txt"

        vcs.files = [makeFile(path: filePath, xStatus: "A", yStatus: " ")]

        state.select(filePath: filePath, isStaged: true)

        let cacheKey = DiffViewerTabState.cacheKey(filePath: filePath, isStaged: true)
        #expect(vcs.diffCache.isLoading(cacheKey))
        vcs.diffCache.cancelAll()
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

    private func waitForSourceFiles(_ state: DiffViewerTabState) async throws {
        for _ in 0 ..< 400 {
            if !state.sourceFiles.isEmpty { return }
            try await Task.sleep(nanoseconds: 5_000_000)
        }
        throw DiffViewerTabStateTestError.timeout("source files never loaded")
    }

    private func waitForSourceLoadFinished(_ state: DiffViewerTabState) async throws {
        for _ in 0 ..< 400 {
            if !state.isLoadingFiles { return }
            try await Task.sleep(nanoseconds: 5_000_000)
        }
        throw DiffViewerTabStateTestError.timeout("source files load never finished")
    }

    private func waitForDiff(_ state: DiffViewerTabState) async throws {
        for _ in 0 ..< 400 {
            if state.diff() != nil { return }
            if state.error() != nil { return }
            try await Task.sleep(nanoseconds: 5_000_000)
        }
        throw DiffViewerTabStateTestError.timeout("diff never loaded")
    }
}

private enum DiffViewerTabStateTestError: Error {
    case timeout(String)
}

private final class DiffViewerGitFixture {
    let url: URL
    var path: String { url.path }

    init() throws {
        url = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-diff-viewer-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        try runGit(args: ["init", "-q", "-b", "main"])
        try runGit(args: ["config", "user.email", "test@example.com"])
        try runGit(args: ["config", "user.name", "Test"])
        try runGit(args: ["config", "commit.gpgsign", "false"])
    }

    func write(_ relativePath: String, contents: String) throws {
        let fileURL = url.appendingPathComponent(relativePath)
        try FileManager.default.createDirectory(
            at: fileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try contents.write(to: fileURL, atomically: true, encoding: .utf8)
    }

    func commit(file: String, message: String) throws -> String {
        try runGit(args: ["add", file])
        try runGit(args: ["commit", "-q", "-m", message])
        return try runGit(args: ["rev-parse", "HEAD"])
    }

    func cleanup() {
        try? FileManager.default.removeItem(at: url)
    }

    @discardableResult
    private func runGit(args: [String]) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", path] + args
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe
        try process.run()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        let output = String(data: data, encoding: .utf8) ?? ""
        if process.terminationStatus != 0 {
            throw NSError(
                domain: "DiffViewerGitFixture",
                code: Int(process.terminationStatus),
                userInfo: [NSLocalizedDescriptionKey: output]
            )
        }
        return output.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
