import Foundation
import Testing

@testable import Muxy

@Suite("FileTreeState")
@MainActor
struct FileTreeStateTests {
    @Test("moveSelection from nil selects first entry when delta is positive")
    func moveSelectionFromNilSelectsFirst() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        state.moveSelection(by: 1)

        #expect(state.selectedFilePath == fixture.path("dir-a"))
    }

    @Test("moveSelection from nil selects last entry when delta is negative")
    func moveSelectionFromNilSelectsLast() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        state.moveSelection(by: -1)

        #expect(state.selectedFilePath == fixture.path("file-2.txt"))
    }

    @Test("moveSelection clamps at top boundary")
    func moveSelectionClampsAtTop() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        state.selectOnly(fixture.path("dir-a"))
        state.moveSelection(by: -5)

        #expect(state.selectedFilePath == fixture.path("dir-a"))
    }

    @Test("moveSelection clamps at bottom boundary")
    func moveSelectionClampsAtBottom() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        state.selectOnly(fixture.path("file-2.txt"))
        state.moveSelection(by: 5)

        #expect(state.selectedFilePath == fixture.path("file-2.txt"))
    }

    @Test("moveSelection advances by one")
    func moveSelectionAdvancesByOne() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        state.selectOnly(fixture.path("dir-a"))
        state.moveSelection(by: 1)

        #expect(state.selectedFilePath == fixture.path("dir-b"))
    }

    @Test("expandOrDescend expands a collapsed directory")
    func expandOrDescendExpandsCollapsed() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let dirAPath = fixture.path("dir-a")
        state.selectOnly(dirAPath)
        state.expandOrDescend()

        #expect(state.expanded.contains(dirAPath))
        #expect(state.selectedFilePath == dirAPath)
    }

    @Test("expandOrDescend moves selection into expanded directory")
    func expandOrDescendMovesIntoExpanded() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let dirAPath = fixture.path("dir-a")
        state.expand(path: dirAPath)
        try await waitForChildrenLoaded(state, of: dirAPath)
        state.selectOnly(dirAPath)

        state.expandOrDescend()

        #expect(state.selectedFilePath == fixture.path("dir-a/inner.txt"))
    }

    @Test("expandOrDescend is a no-op on a file")
    func expandOrDescendNoOpOnFile() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let filePath = fixture.path("file-1.txt")
        state.selectOnly(filePath)
        state.expandOrDescend()

        #expect(state.selectedFilePath == filePath)
        #expect(!state.expanded.contains(filePath))
    }

    @Test("collapseOrJumpToParent collapses an expanded directory")
    func collapseOrJumpCollapsesDirectory() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let dirAPath = fixture.path("dir-a")
        state.expand(path: dirAPath)
        try await waitForChildrenLoaded(state, of: dirAPath)
        state.selectOnly(dirAPath)

        state.collapseOrJumpToParent()

        #expect(!state.expanded.contains(dirAPath))
        #expect(state.selectedFilePath == dirAPath)
    }

    @Test("collapseOrJumpToParent jumps to parent directory from child")
    func collapseOrJumpJumpsToParent() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let dirAPath = fixture.path("dir-a")
        let childPath = fixture.path("dir-a/inner.txt")
        state.expand(path: dirAPath)
        try await waitForChildrenLoaded(state, of: dirAPath)
        state.selectOnly(childPath)

        state.collapseOrJumpToParent()

        #expect(state.selectedFilePath == dirAPath)
    }

    @Test("collapseOrJumpToParent does not move selection at root level")
    func collapseOrJumpStaysAtRoot() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let filePath = fixture.path("file-1.txt")
        state.selectOnly(filePath)

        state.collapseOrJumpToParent()

        #expect(state.selectedFilePath == filePath)
    }

    @Test("activateSelection opens a file via the closure")
    func activateSelectionOpensFile() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let filePath = fixture.path("file-1.txt")
        state.selectOnly(filePath)

        var opened: [String] = []
        state.activateSelection(open: { opened.append($0) })

        #expect(opened == [filePath])
    }

    @Test("activateSelection toggles a directory instead of opening")
    func activateSelectionTogglesDirectory() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let dirAPath = fixture.path("dir-a")
        state.selectOnly(dirAPath)

        var opened: [String] = []
        state.activateSelection(open: { opened.append($0) })

        #expect(opened.isEmpty)
        #expect(state.expanded.contains(dirAPath))
    }

    @Test("activateSelection does nothing when selection is nil")
    func activateSelectionNoOpWhenNil() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        var opened: [String] = []
        state.activateSelection(open: { opened.append($0) })

        #expect(opened.isEmpty)
    }

    @Test("entry(at:) resolves a root-level entry")
    func entryAtResolvesRootEntry() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let entry = state.entry(at: fixture.path("file-1.txt"))

        #expect(entry?.name == "file-1.txt")
        #expect(entry?.isDirectory == false)
    }

    @Test("entry(at:) resolves a nested entry under expanded directory")
    func entryAtResolvesNestedEntry() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let dirAPath = fixture.path("dir-a")
        state.expand(path: dirAPath)
        try await waitForChildrenLoaded(state, of: dirAPath)

        let entry = state.entry(at: fixture.path("dir-a/inner.txt"))

        #expect(entry?.name == "inner.txt")
        #expect(entry?.isDirectory == false)
    }

    @Test("isIgnoredFile flags a dotfile")
    func isIgnoredFileFlagsDotfile() {
        let state = FileTreeState(rootPath: "/tmp")

        #expect(state.isIgnoredFile(makeEntry(name: ".github", isDirectory: true)))
    }

    @Test("isIgnoredFile flags built-in noise names")
    func isIgnoredFileFlagsBuiltInNoise() {
        let state = FileTreeState(rootPath: "/tmp")

        #expect(state.isIgnoredFile(makeEntry(name: "node_modules", isDirectory: true)))
        #expect(state.isIgnoredFile(makeEntry(name: "yarn.lock", isDirectory: false)))
    }

    @Test("isIgnoredFile flags a git-ignored entry")
    func isIgnoredFileFlagsGitIgnored() {
        let state = FileTreeState(rootPath: "/tmp")

        #expect(state.isIgnoredFile(makeEntry(name: "build.log", isDirectory: false, isIgnored: true)))
    }

    @Test("isIgnoredFile keeps a normal file")
    func isIgnoredFileKeepsNormalFile() {
        let state = FileTreeState(rootPath: "/tmp")

        #expect(!state.isIgnoredFile(makeEntry(name: "README.md", isDirectory: false)))
    }

    @Test("hideIgnoredFiles persists across instances via injected defaults")
    func hideIgnoredFilesPersistsAcrossInstances() throws {
        let (defaults, suiteName) = try makeIsolatedDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let first = FileTreeState(rootPath: "/tmp", defaults: defaults)
        #expect(!first.hideIgnoredFiles)
        first.hideIgnoredFiles = true

        let second = FileTreeState(rootPath: "/tmp", defaults: defaults)
        #expect(second.hideIgnoredFiles)
    }

    @Test("hideIgnoredFiles filters dotfiles and built-in noise from the root")
    func hideIgnoredFilesFiltersRoot() async throws {
        let fixture = try NoiseFixture()
        defer { fixture.cleanup() }
        let (defaults, suiteName) = try makeIsolatedDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let state = FileTreeState(rootPath: fixture.rootPath, defaults: defaults)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        #expect(Set(state.visibleRootEntries().map(\.name))
            == [".config", ".hidden.txt", "node_modules", "visible.txt"])

        state.hideIgnoredFiles = true

        #expect(state.visibleRootEntries().map(\.name) == ["visible.txt"])
    }

    @Test("revealFile keeps a filtered entry and its parent visible")
    func revealFileExemptsSelectedPath() async throws {
        let fixture = try NoiseFixture()
        defer { fixture.cleanup() }
        let (defaults, suiteName) = try makeIsolatedDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let state = FileTreeState(rootPath: fixture.rootPath, defaults: defaults)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)
        state.hideIgnoredFiles = true

        state.revealFile(at: fixture.path(".config/app.json"))
        try await waitForChildrenLoaded(state, of: fixture.path(".config"))

        let visibleNames = state.flatVisibleRows().compactMap { row -> String? in
            if case let .entry(entry, _) = row { return entry.name }
            return nil
        }
        #expect(visibleNames.contains(".config"))
        #expect(visibleNames.contains("app.json"))
        #expect(visibleNames.contains("visible.txt"))
        #expect(!visibleNames.contains("node_modules"))
    }

    @Test("setRootPath resets loaded state and reloads new root")
    func setRootPathResetsLoadedState() async throws {
        let first = try TreeFixture()
        let second = try TreeFixture()
        defer {
            first.cleanup()
            second.cleanup()
        }

        let state = FileTreeState(rootPath: first.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)
        state.selectOnly(first.path("file-1.txt"))

        state.setRootPath(second.rootPath)
        try await waitForRootLoaded(state)

        #expect(state.rootPath == second.rootPath)
        #expect(state.selectedFilePath == nil)
        #expect(state.selectedPaths.isEmpty)
        #expect(state.selectionAnchorPath == nil)
        #expect(state.visibleRootEntries().map(\.absolutePath).contains(second.path("file-1.txt")))
    }

    @Test("toggle selection, range extension, and clearing update selection state")
    func selectionHelpersUpdateSelectionState() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let first = fixture.path("dir-a")
        let second = fixture.path("dir-b")
        let missing = fixture.path("missing.txt")

        state.toggleSelection(first)
        #expect(state.selectedFilePath == first)
        #expect(state.isPathSelected(first))
        #expect(state.selectionAnchorPath == first)

        state.toggleSelection(first)
        #expect(!state.isPathSelected(first))
        #expect(state.selectedFilePath == nil)

        state.extendSelection(to: missing)
        #expect(state.selectedFilePath == missing)
        #expect(state.isPathSelected(missing))

        state.selectionAnchorPath = first
        state.extendSelection(to: second)
        #expect(state.selectedFilePath == second)
        #expect(state.selectedPaths == [first, second])

        state.clearSelection()
        #expect(state.selectedFilePath == nil)
        #expect(state.selectedPaths.isEmpty)
        #expect(state.selectionAnchorPath == nil)
    }

    @Test("flat rows include pending root and child entries with stable ids")
    func flatRowsIncludePendingEntries() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let rootToken = UUID()
        state.pendingNewEntry = FileTreeState.PendingNewEntry(
            parentPath: fixture.rootPath,
            kind: .file,
            token: rootToken
        )
        let rootRows = state.flatVisibleRows()
        #expect(rootRows.contains { $0.id == "p:\(rootToken.uuidString)" })

        let dirAPath = fixture.path("dir-a")
        state.expand(path: dirAPath)
        try await waitForChildrenLoaded(state, of: dirAPath)
        let childToken = UUID()
        state.pendingNewEntry = FileTreeState.PendingNewEntry(
            parentPath: dirAPath,
            kind: .folder,
            token: childToken
        )

        let rows = state.flatVisibleRows()
        #expect(rows.contains { $0.id == "e:\(dirAPath)" })
        #expect(rows.contains { $0.id == "p:\(childToken.uuidString)" })
    }

    @Test("scroll target, status accessors, parent paths, and directory refreshes are stable")
    func accessorsAndRefreshesAreStable() async throws {
        let fixture = try TreeFixture()
        defer { fixture.cleanup() }

        let state = FileTreeState(rootPath: fixture.rootPath)
        state.loadRootIfNeeded()
        try await waitForRootLoaded(state)

        let dirAPath = fixture.path("dir-a")
        let childPath = fixture.path("dir-a/inner.txt")
        #expect(state.parentDirectory(of: childPath + "/") == dirAPath)
        #expect(state.status(for: childPath) == nil)
        #expect(!state.directoryHasChanges(dirAPath))

        state.moveSelection(by: 1)
        #expect(state.pendingScrollTarget == dirAPath)
        state.consumeScrollTarget()
        #expect(state.pendingScrollTarget == nil)

        state.refreshDirectory(path: fixture.rootPath + "/")
        try await waitForRootLoaded(state)
        state.refreshDirectory(path: dirAPath + "/")
        try await waitForChildrenLoaded(state, of: dirAPath)
        state.refresh()
    }

    private func makeIsolatedDefaults() throws -> (defaults: UserDefaults, suiteName: String) {
        let suiteName = "FileTreeStateTests-\(UUID().uuidString)"
        return (try #require(UserDefaults(suiteName: suiteName)), suiteName)
    }

    private func makeEntry(name: String, isDirectory: Bool, isIgnored: Bool = false) -> FileTreeEntry {
        FileTreeEntry(
            name: name,
            absolutePath: "/tmp/\(name)",
            relativePath: name,
            isDirectory: isDirectory,
            isIgnored: isIgnored
        )
    }

    private func waitForRootLoaded(_ state: FileTreeState) async throws {
        for _ in 0 ..< 400 {
            if !state.visibleRootEntries().isEmpty { return }
            try await Task.sleep(nanoseconds: 5_000_000)
        }
        throw FileTreeStateTestError.timeout("FileTreeState root entries never loaded")
    }

    private func waitForChildrenLoaded(_ state: FileTreeState, of path: String) async throws {
        for _ in 0 ..< 400 {
            if state.children[path] != nil { return }
            try await Task.sleep(nanoseconds: 5_000_000)
        }
        throw FileTreeStateTestError.timeout("FileTreeState children of \(path) never loaded")
    }
}

private enum FileTreeStateTestError: Error {
    case timeout(String)
}

@MainActor
private final class TreeFixture {
    let rootURL: URL

    var rootPath: String { rootURL.path }

    init() throws {
        let fm = FileManager.default
        rootURL = fm.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try fm.createDirectory(at: rootURL, withIntermediateDirectories: true)
        try fm.createDirectory(at: rootURL.appendingPathComponent("dir-a"), withIntermediateDirectories: true)
        try fm.createDirectory(at: rootURL.appendingPathComponent("dir-b"), withIntermediateDirectories: true)
        try "inner".write(
            to: rootURL.appendingPathComponent("dir-a/inner.txt"),
            atomically: true,
            encoding: .utf8
        )
        try "one".write(
            to: rootURL.appendingPathComponent("file-1.txt"),
            atomically: true,
            encoding: .utf8
        )
        try "two".write(
            to: rootURL.appendingPathComponent("file-2.txt"),
            atomically: true,
            encoding: .utf8
        )
    }

    func path(_ relative: String) -> String {
        rootURL.appendingPathComponent(relative).path
    }

    func cleanup() {
        try? FileManager.default.removeItem(at: rootURL)
    }
}

@MainActor
private final class NoiseFixture {
    let rootURL: URL

    var rootPath: String { rootURL.path }

    init() throws {
        let fm = FileManager.default
        rootURL = fm.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try fm.createDirectory(at: rootURL, withIntermediateDirectories: true)
        try fm.createDirectory(
            at: rootURL.appendingPathComponent("node_modules"),
            withIntermediateDirectories: true
        )
        try fm.createDirectory(
            at: rootURL.appendingPathComponent(".config"),
            withIntermediateDirectories: true
        )
        try "{}".write(
            to: rootURL.appendingPathComponent(".config/app.json"),
            atomically: true,
            encoding: .utf8
        )
        try "secret".write(
            to: rootURL.appendingPathComponent(".hidden.txt"),
            atomically: true,
            encoding: .utf8
        )
        try "visible".write(
            to: rootURL.appendingPathComponent("visible.txt"),
            atomically: true,
            encoding: .utf8
        )
    }

    func path(_ relative: String) -> String {
        rootURL.appendingPathComponent(relative).path
    }

    func cleanup() {
        try? FileManager.default.removeItem(at: rootURL)
    }
}
