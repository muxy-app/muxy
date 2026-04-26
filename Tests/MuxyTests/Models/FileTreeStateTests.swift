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
