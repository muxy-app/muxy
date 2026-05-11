import Foundation
import Testing

@testable import Muxy

@Suite("TextSearchService")
struct TextSearchServiceTests {
    @Test("parses ripgrep match line")
    func parsesMatch() throws {
        let line = #"{"type":"match","data":{"path":{"text":"/proj/src/foo.swift"},"lines":{"text":"  let answer = 42\n"},"line_number":12,"absolute_offset":120,"submatches":[{"match":{"text":"answer"},"start":6,"end":12}]}}"#
        let match = try #require(TextSearchService.parseLine(line, projectPath: "/proj"))

        #expect(match.absolutePath == "/proj/src/foo.swift")
        #expect(match.relativePath == "src/foo.swift")
        #expect(match.lineNumber == 12)
        #expect(match.lineText == "  let answer = 42")
        #expect(match.matchStart == 6)
        #expect(match.matchEnd == 12)
        #expect(match.column == 7)
    }

    @Test("returns nil for non-match types")
    func ignoresBeginAndEnd() throws {
        let begin = #"{"type":"begin","data":{"path":{"text":"/proj/x"}}}"#
        let end = #"{"type":"end","data":{"path":{"text":"/proj/x"},"stats":{}}}"#
        #expect(TextSearchService.parseLine(begin, projectPath: "/proj") == nil)
        #expect(TextSearchService.parseLine(end, projectPath: "/proj") == nil)
    }

    @Test("falls back to absolute path when not under project")
    func absoluteWhenOutsideProject() throws {
        let line = #"{"type":"match","data":{"path":{"text":"/other/foo.swift"},"lines":{"text":"hit\n"},"line_number":1,"absolute_offset":0,"submatches":[{"match":{"text":"hit"},"start":0,"end":3}]}}"#
        let match = try #require(TextSearchService.parseLine(line, projectPath: "/proj"))
        #expect(match.relativePath == "/other/foo.swift")
    }

    @Test("computes column for multibyte characters")
    func multibyteColumn() {
        let column = TextSearchService.columnFromUTF8Offset(4, in: "héllo world")
        #expect(column == 4)
    }

    @Test("parses Korean ripgrep match offsets")
    func parsesKoreanMatchOffsets() throws {
        let line = #"{"type":"match","data":{"path":{"text":"/proj/greeting.md"},"lines":{"text":"안녕하세요\n"},"line_number":1,"absolute_offset":0,"submatches":[{"match":{"text":"안녕"},"start":0,"end":6}]}}"#
        let match = try #require(TextSearchService.parseLine(line, projectPath: "/proj"))

        #expect(match.relativePath == "greeting.md")
        #expect(match.lineNumber == 1)
        #expect(match.lineText == "안녕하세요")
        #expect(match.matchStart == 0)
        #expect(match.matchEnd == 6)
        #expect(match.column == 1)
    }

    @Test("searches Korean text through ripgrep")
    func searchesKoreanText() async throws {
        guard TextSearchService.ripgrepExecutableURL() != nil else { return }
        let directory = try makeSearchFixture()
        defer { try? FileManager.default.removeItem(at: directory) }

        let matches = await TextSearchService.search(query: "안녕", in: directory.path)

        #expect(matches.contains { $0.lineText == "안녕하세요" })
    }

    @Test("keeps regex search semantics")
    func keepsRegexSearchSemantics() async throws {
        guard TextSearchService.ripgrepExecutableURL() != nil else { return }
        let directory = try makeSearchFixture()
        defer { try? FileManager.default.removeItem(at: directory) }

        let matches = await TextSearchService.search(query: "foo.*bar", in: directory.path)

        #expect(matches.contains { $0.lineText == "foo123bar" })
    }

    private func makeSearchFixture() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-text-search-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = directory.appendingPathComponent("test.md")
        try """
        안녕하세요
        foo123bar
        """.write(to: file, atomically: true, encoding: .utf8)
        return directory
    }
}
