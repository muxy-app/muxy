import Foundation
import Testing

@testable import Muxy

@Suite("GitStatusParser")
struct GitStatusParserTests {
    @Test("parseStatusPorcelain with empty data returns empty array")
    func parseEmpty() {
        let result = GitStatusParser.parseStatusPorcelain(Data(), stats: [:])
        #expect(result.isEmpty)
    }

    @Test("parseStatusPorcelain parses modified file")
    func parseModified() {
        let raw = "M  src/file.swift\0"
        let data = Data(raw.utf8)
        let result = GitStatusParser.parseStatusPorcelain(data, stats: [:])

        #expect(result.count == 1)
        #expect(result[0].path == "src/file.swift")
        #expect(result[0].xStatus == "M")
        #expect(result[0].yStatus == " ")
        #expect(result[0].oldPath == nil)
    }

    @Test("parseStatusPorcelain parses untracked file")
    func parseUntracked() {
        let raw = "?? newfile.txt\0"
        let data = Data(raw.utf8)
        let result = GitStatusParser.parseStatusPorcelain(data, stats: [:])

        #expect(result.count == 1)
        #expect(result[0].path == "newfile.txt")
        #expect(result[0].xStatus == "?")
        #expect(result[0].yStatus == "?")
    }

    @Test("parseStatusPorcelain parses renamed file")
    func parseRenamed() {
        let raw = "R  old.swift\0new.swift\0"
        let data = Data(raw.utf8)
        let result = GitStatusParser.parseStatusPorcelain(data, stats: [:])

        #expect(result.count == 1)
        #expect(result[0].path == "new.swift")
        #expect(result[0].oldPath == "old.swift")
        #expect(result[0].xStatus == "R")
    }

    @Test("parseStatusPorcelain parses unstaged renamed file")
    func parseUnstagedRenamed() {
        let raw = " R old.swift\0new.swift\0"
        let data = Data(raw.utf8)
        let result = GitStatusParser.parseStatusPorcelain(data, stats: [:])

        #expect(result.count == 1)
        #expect(result[0].path == "new.swift")
        #expect(result[0].oldPath == "old.swift")
        #expect(result[0].xStatus == " ")
        #expect(result[0].yStatus == "R")
    }

    @Test("parseStatusPorcelain merges numstat stats")
    func parseWithNumstat() {
        let raw = "M  file.swift\0"
        let data = Data(raw.utf8)
        let stats = ["file.swift": NumstatEntry(additions: 10, deletions: 3, isBinary: false)]
        let result = GitStatusParser.parseStatusPorcelain(data, stats: stats)

        #expect(result[0].additions == 10)
        #expect(result[0].deletions == 3)
        #expect(result[0].isBinary == false)
    }

    @Test("parseStatusPorcelain sorts output by path")
    func parseSorted() {
        let raw = "M  z.swift\0M  a.swift\0"
        let data = Data(raw.utf8)
        let result = GitStatusParser.parseStatusPorcelain(data, stats: [:])

        #expect(result.count == 2)
        #expect(result[0].path == "a.swift")
        #expect(result[1].path == "z.swift")
    }

    @Test("parseStatusPorcelain skips short tokens")
    func parseSkipsShortTokens() {
        let raw = "M  valid.swift\0ab\0"
        let data = Data(raw.utf8)
        let result = GitStatusParser.parseStatusPorcelain(data, stats: [:])

        #expect(result.count == 1)
        #expect(result[0].path == "valid.swift")
    }

    @Test("parseNumstat basic entry")
    func parseNumstatBasic() {
        let output = "5\t3\tfile.swift"
        let stats = GitStatusParser.parseNumstat(output)

        #expect(stats["file.swift"]?.additions == 5)
        #expect(stats["file.swift"]?.deletions == 3)
        #expect(stats["file.swift"]?.isBinary == false)
    }

    @Test("parseNumstat binary file")
    func parseNumstatBinary() {
        let output = "-\t-\tbinary.png"
        let stats = GitStatusParser.parseNumstat(output)

        #expect(stats["binary.png"]?.isBinary == true)
        #expect(stats["binary.png"]?.additions == nil)
        #expect(stats["binary.png"]?.deletions == nil)
    }

    @Test("parseNumstat multiple entries")
    func parseNumstatMultiple() {
        let output = """
        5\t3\tfile1.swift
        10\t0\tfile2.swift
        """
        let stats = GitStatusParser.parseNumstat(output)
        #expect(stats["file1.swift"] != nil)
        #expect(stats["file2.swift"] != nil)
        #expect(stats["file2.swift"]?.additions == 10)
    }

    @Test("normalizeNumstatPath simple rename")
    func normalizeSimpleRename() {
        #expect(GitStatusParser.normalizeNumstatPath("old.swift => new.swift") == "new.swift")
    }

    @Test("normalizeNumstatPath brace rename")
    func normalizeBraceRename() {
        #expect(GitStatusParser.normalizeNumstatPath("src/{old => new}/file.swift") == "src/new/file.swift")
    }

    @Test("normalizeNumstatPath no rename passthrough")
    func normalizeNoRename() {
        #expect(GitStatusParser.normalizeNumstatPath("file.swift") == "file.swift")
    }
}
