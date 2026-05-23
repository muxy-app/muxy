import Foundation
import Testing

@testable import Muxy

@Suite("GitRepositoryService diff preview")
struct GitRepositoryServiceDiffPreviewTests {
    @Test("untracked preview reads only limited lines")
    func untrackedPreviewReadsOnlyLimitedLines() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let fileName = "large.txt"
        let fileURL = directory.appendingPathComponent(fileName)
        let content = (0 ..< 2_500).map { "line \($0)" }.joined(separator: "\n")
        try content.write(to: fileURL, atomically: true, encoding: .utf8)

        let result = try await GitRepositoryService().patchAndCompare(
            repoPath: directory.path,
            filePath: fileName,
            lineLimit: 100,
            hints: GitRepositoryService.DiffHints(hasStaged: false, hasUnstaged: false, isUntrackedOrNew: true)
        )

        #expect(result.additions == 100)
        #expect(result.truncated)
        #expect(result.rows.count == 101)
        #expect(result.rows.last?.newLineNumber == 100)
    }
}
