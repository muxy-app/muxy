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
