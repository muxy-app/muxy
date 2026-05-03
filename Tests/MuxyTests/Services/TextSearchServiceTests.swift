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
}
