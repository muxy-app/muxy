import Foundation
import Testing

@testable import Muxy

@Suite("TextSearchService")
struct TextSearchServiceTests {
    @Test("parses a vimgrep line")
    func parsesVimgrepLine() throws {
        let line: Substring = "/proj/src/foo.swift:12:7:  let answer = 42"
        let match = try #require(TextSearchService.parseVimgrepLine(
            line, projectPath: "/proj", patternByteLength: 6
        ))

        #expect(match.absolutePath == "/proj/src/foo.swift")
        #expect(match.relativePath == "src/foo.swift")
        #expect(match.lineNumber == 12)
        #expect(match.lineText == "  let answer = 42")
        #expect(match.matchByteStart == 6)
        #expect(match.matchByteLength == 6)
        #expect(match.column == 7)
    }

    @Test("falls back to absolute path when not under project")
    func absoluteWhenOutsideProject() throws {
        let line: Substring = "/other/foo.swift:1:1:hit"
        let match = try #require(TextSearchService.parseVimgrepLine(
            line, projectPath: "/proj", patternByteLength: 3
        ))
        #expect(match.relativePath == "/other/foo.swift")
    }

    @Test("preserves text containing colons")
    func keepsColonsInLineText() throws {
        let line: Substring = "/proj/x.swift:3:5:    a: Int = 1"
        let match = try #require(TextSearchService.parseVimgrepLine(
            line, projectPath: "/proj", patternByteLength: 1
        ))
        #expect(match.lineText == "    a: Int = 1")
        #expect(match.column == 5)
    }

    @Test("clamps match length to remaining bytes on the line")
    func clampsLength() throws {
        let line: Substring = "/proj/x.swift:1:5:abcd"
        let match = try #require(TextSearchService.parseVimgrepLine(
            line, projectPath: "/proj", patternByteLength: 100
        ))
        #expect(match.matchByteStart == 4)
        #expect(match.matchByteLength == 0)
    }

    @Test("searches Korean text through ripgrep")
    func searchesKoreanText() async throws {
        guard TextSearchService.ripgrepExecutableURL() != nil else { return }
        let directory = try makeSearchFixture()
        defer { try? FileManager.default.removeItem(at: directory) }

        let matches = await TextSearchService.search(query: "안녕", in: directory.path)

        #expect(matches.contains { $0.lineText == "안녕하세요" })
    }

    @Test("treats query as a literal (no regex)")
    func literalQuery() async throws {
        guard TextSearchService.ripgrepExecutableURL() != nil else { return }
        let directory = try makeSearchFixture()
        defer { try? FileManager.default.removeItem(at: directory) }

        let matches = await TextSearchService.search(query: "foo.*bar", in: directory.path)
        #expect(matches.isEmpty)

        let literal = await TextSearchService.search(query: "foo123bar", in: directory.path)
        #expect(literal.contains { $0.lineText == "foo123bar" })
    }

    @Test("starting a new search cancels the in-flight one")
    func newSearchCancelsPrevious() async throws {
        let directory = try makeSearchFixture()
        defer { try? FileManager.default.removeItem(at: directory) }
        let executable = try makeFakeRipgrep()
        defer { try? FileManager.default.removeItem(at: executable.deletingLastPathComponent()) }

        let coordinator = SearchCoordinator()
        let firstPattern = Data("slow".utf8)
        let secondPattern = Data("안녕".utf8)
        let first = Task {
            await coordinator.run(
                executable: executable,
                patternData: firstPattern,
                patternByteLength: firstPattern.count,
                projectPath: directory.path,
                options: TextSearchOptions()
            )
        }
        try await Task.sleep(for: .milliseconds(50))

        let secondResults = await coordinator.run(
            executable: executable,
            patternData: secondPattern,
            patternByteLength: secondPattern.count,
            projectPath: directory.path,
            options: TextSearchOptions()
        )
        let firstResults = await first.value

        #expect(secondResults.contains { $0.lineText == "안녕하세요" })
        #expect(firstResults.isEmpty)
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

    private func makeFakeRipgrep() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-fake-rg-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let executable = directory.appendingPathComponent("rg")
        try """
        #!/bin/sh
        project=""
        for arg in "$@"; do
          project="$arg"
        done
        pattern="$(cat)"
        if [ "$pattern" = "slow" ]; then
          sleep 2
          printf "%s/test.md:2:1:foo123bar\\n" "$project"
        else
          printf "%s/test.md:1:1:안녕하세요\\n" "$project"
        fi
        """.write(to: executable, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: executable.path)
        return executable
    }
}
