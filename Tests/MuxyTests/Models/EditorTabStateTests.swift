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
}
