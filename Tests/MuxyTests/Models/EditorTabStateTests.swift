import Foundation
import Testing

@testable import Muxy

@Suite("EditorTabState")
@MainActor
struct EditorTabStateTests {
    @Test("markdown tabs enable split scroll sync by default")
    func markdownTabsEnableSplitScrollSyncByDefault() throws {
        let tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        let fileURL = tempDirectory.appendingPathComponent("notes.md")
        try "# Hello\n".write(to: fileURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let state = EditorTabState(projectPath: tempDirectory.path, filePath: fileURL.path)

        #expect(state.isMarkdownFile)
        #expect(state.markdownViewMode == .preview)
        #expect(state.markdownScrollSyncEnabled)
    }

    @Test("reloadFromDisk picks up external file changes")
    func reloadFromDiskPicksUpExternalChanges() async throws {
        let tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        let fileURL = tempDirectory.appendingPathComponent("notes.md")
        try "# Old\n".write(to: fileURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let state = EditorTabState(projectPath: tempDirectory.path, filePath: fileURL.path)

        try await waitForLoad(state)
        #expect(state.backingStore?.fullText() == "# Old\n")
        let initialPreviewVersion = state.previewRefreshVersion

        try "# New\n".write(to: fileURL, atomically: true, encoding: .utf8)
        state.reloadFromDisk()
        try await waitForLoad(state)

        #expect(state.backingStore?.fullText() == "# New\n")
        #expect(state.previewRefreshVersion > initialPreviewVersion)
    }

    @Test("saveFileAsync preserves a single-level symlink and writes through to target")
    func saveFileAsyncPreservesSingleLevelSymlink() async throws {
        let tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        let realFile = tempDirectory.appendingPathComponent("real.txt")
        let symlinkFile = tempDirectory.appendingPathComponent("link.txt")
        try "original\n".write(to: realFile, atomically: true, encoding: .utf8)
        try FileManager.default.createSymbolicLink(at: symlinkFile, withDestinationURL: realFile)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let state = EditorTabState(projectPath: tempDirectory.path, filePath: symlinkFile.path)
        try await waitForLoad(state)
        state.backingStore?.loadFromText("updated\n")
        state.markModified()
        try await state.saveFileAsync()

        let attrs = try FileManager.default.attributesOfItem(atPath: symlinkFile.path)
        #expect(attrs[.type] as? FileAttributeType == .typeSymbolicLink)
        let dest = try FileManager.default.destinationOfSymbolicLink(atPath: symlinkFile.path)
        #expect(dest == realFile.path)
        #expect(try String(contentsOf: realFile, encoding: .utf8) == "updated\n")
    }

    @Test("saveFileAsync writes through a multi-level symlink chain")
    func saveFileAsyncTraversesMultiLevelSymlinks() async throws {
        let tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        let realFile = tempDirectory.appendingPathComponent("c.txt")
        let mid = tempDirectory.appendingPathComponent("b.txt")
        let top = tempDirectory.appendingPathComponent("a.txt")
        try "v1\n".write(to: realFile, atomically: true, encoding: .utf8)
        try FileManager.default.createSymbolicLink(at: mid, withDestinationURL: realFile)
        try FileManager.default.createSymbolicLink(at: top, withDestinationURL: mid)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let state = EditorTabState(projectPath: tempDirectory.path, filePath: top.path)
        try await waitForLoad(state)
        state.backingStore?.loadFromText("v2\n")
        state.markModified()
        try await state.saveFileAsync()

        let topAttrs = try FileManager.default.attributesOfItem(atPath: top.path)
        let midAttrs = try FileManager.default.attributesOfItem(atPath: mid.path)
        #expect(topAttrs[.type] as? FileAttributeType == .typeSymbolicLink)
        #expect(midAttrs[.type] as? FileAttributeType == .typeSymbolicLink)
        #expect(try String(contentsOf: realFile, encoding: .utf8) == "v2\n")
    }

    @Test("saveFileAsync resolves a relative symlink target")
    func saveFileAsyncResolvesRelativeSymlink() async throws {
        let tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let subdir = tempDirectory.appendingPathComponent("sub")
        try FileManager.default.createDirectory(at: subdir, withIntermediateDirectories: true)
        let realFile = tempDirectory.appendingPathComponent("target.txt")
        let link = subdir.appendingPathComponent("link.txt")
        try "before\n".write(to: realFile, atomically: true, encoding: .utf8)
        try FileManager.default.createSymbolicLink(atPath: link.path, withDestinationPath: "../target.txt")
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let state = EditorTabState(projectPath: tempDirectory.path, filePath: link.path)
        try await waitForLoad(state)
        state.backingStore?.loadFromText("after\n")
        state.markModified()
        try await state.saveFileAsync()

        let linkAttrs = try FileManager.default.attributesOfItem(atPath: link.path)
        #expect(linkAttrs[.type] as? FileAttributeType == .typeSymbolicLink)
        let dest = try FileManager.default.destinationOfSymbolicLink(atPath: link.path)
        #expect(dest == "../target.txt")
        #expect(try String(contentsOf: realFile, encoding: .utf8) == "after\n")
    }

    @Test("saveFileAsync writes through to a plain file as a regression guard")
    func saveFileAsyncWritesPlainFileUnchanged() async throws {
        let tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        let fileURL = tempDirectory.appendingPathComponent("plain.txt")
        try "first\n".write(to: fileURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let state = EditorTabState(projectPath: tempDirectory.path, filePath: fileURL.path)
        try await waitForLoad(state)
        state.backingStore?.loadFromText("second\n")
        state.markModified()
        try await state.saveFileAsync()

        let attrs = try FileManager.default.attributesOfItem(atPath: fileURL.path)
        #expect(attrs[.type] as? FileAttributeType == .typeRegular)
        #expect(try String(contentsOf: fileURL, encoding: .utf8) == "second\n")
    }

    private func waitForLoad(_ state: EditorTabState, timeout: TimeInterval = 2.0) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while state.isLoading || state.isIncrementalLoading {
            if Date() >= deadline {
                throw NSError(domain: "EditorTabStateTests", code: -1, userInfo: [NSLocalizedDescriptionKey: "Load did not finish in time"])
            }
            try await Task.sleep(nanoseconds: 20_000_000)
        }
    }
}
