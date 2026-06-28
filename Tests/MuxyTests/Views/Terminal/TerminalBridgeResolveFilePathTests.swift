import Foundation
import Testing
@testable import Muxy

@Suite("TerminalBridge.resolveFilePath")
@MainActor
struct TerminalBridgeResolveFilePathTests {
    private func makeTempDir() -> URL {
        let url = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-resolve-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func writeFile(_ url: URL, name: String) -> String {
        let path = url.appendingPathComponent(name).path
        FileManager.default.createFile(atPath: path, contents: Data())
        return path
    }

    @Test func resolvesAbsolutePath() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "a.txt")
        #expect(TerminalBridge.resolveFilePath(file, projectPath: "/unused") == file)
    }

    @Test func resolvesRelativeToProject() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        _ = writeFile(dir, name: "b.txt")
        let resolved = TerminalBridge.resolveFilePath("b.txt", projectPath: dir.path)
        #expect(resolved == dir.appendingPathComponent("b.txt").path)
    }

    @Test func stripsQuotesAndBrackets() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        _ = writeFile(dir, name: "c.txt")
        #expect(TerminalBridge.resolveFilePath("\"c.txt\"", projectPath: dir.path) != nil)
        #expect(TerminalBridge.resolveFilePath("(c.txt)", projectPath: dir.path) != nil)
        #expect(TerminalBridge.resolveFilePath("<c.txt>", projectPath: dir.path) != nil)
    }

    @Test func rejectsDirectory() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let sub = dir.appendingPathComponent("sub")
        try FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
        #expect(TerminalBridge.resolveFilePath("sub", projectPath: dir.path) == nil)
    }

    @Test func rejectsMissingPath() {
        #expect(TerminalBridge.resolveFilePath("does-not-exist.xyz", projectPath: "/tmp") == nil)
    }

    @Test func rejectsEmptyAndWhitespace() {
        #expect(TerminalBridge.resolveFilePath("", projectPath: "/tmp") == nil)
        #expect(TerminalBridge.resolveFilePath("   \t", projectPath: "/tmp") == nil)
    }

    @Test func expandsTilde() throws {
        let home = NSString(string: "~").expandingTildeInPath
        let name = "muxy-tilde-\(UUID().uuidString).txt"
        let path = (home as NSString).appendingPathComponent(name)
        FileManager.default.createFile(atPath: path, contents: Data())
        defer { try? FileManager.default.removeItem(atPath: path) }
        let resolved = TerminalBridge.resolveFilePath("~/\(name)", projectPath: "/unused")
        #expect(resolved == path)
    }

    @Test func resolvesLocalFilePathFromFileURL() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "doc.md")
        let url = URL(fileURLWithPath: file)
        #expect(TerminalBridge.resolveLocalFilePath(from: url, projectPath: "/unused") == file)
    }

    @Test func resolvesLocalFilePathFromSchemelessAbsolutePath() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "notes.md")
        let url = try #require(URL(string: file))
        #expect(TerminalBridge.resolveLocalFilePath(from: url, projectPath: "/unused") == file)
    }

    @Test func resolvesLocalFilePathFromSchemelessRelativePath() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "readme.md")
        let url = try #require(URL(string: "readme.md"))
        #expect(TerminalBridge.resolveLocalFilePath(from: url, projectPath: dir.path) == file)
    }

    @Test func resolvesLocalFilePathRejectsHttpURL() throws {
        let url = try #require(URL(string: "https://example.com/readme.md"))
        #expect(TerminalBridge.resolveLocalFilePath(from: url, projectPath: "/tmp") == nil)
    }

    @Test func resolvesLocalFilePathRejectsDirectoryFileURL() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let url = URL(fileURLWithPath: dir.path)
        #expect(TerminalBridge.resolveLocalFilePath(from: url, projectPath: "/unused") == nil)
    }

    @Test func resolvesFileLocationWithLineSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "results.md")
        let url = try #require(URL(string: "\(file):12"))
        let location = TerminalBridge.resolveFileLocation(from: url, projectPath: "/unused")
        #expect(location == .init(path: file, line: 12, column: nil))
    }

    @Test func resolvesFileLocationWithLineAndColumnSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "main.swift")
        let url = try #require(URL(string: "\(file):42:7"))
        let location = TerminalBridge.resolveFileLocation(from: url, projectPath: "/unused")
        #expect(location == .init(path: file, line: 42, column: 7))
    }

    @Test func resolvesFileLocationRelativeWithLineSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let sub = dir.appendingPathComponent("reviews")
        try FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
        let file = writeFile(sub, name: "results.md")
        let url = try #require(URL(string: "reviews/results.md:12"))
        let location = TerminalBridge.resolveFileLocation(from: url, projectPath: dir.path)
        #expect(location == .init(path: file, line: 12, column: nil))
    }

    @Test func resolvesFileLocationRootLevelDottedNameWithLineSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "README.md")
        let url = try #require(URL(string: "README.md:10"))
        let location = TerminalBridge.resolveFileLocation(from: url, projectPath: dir.path)
        #expect(location == .init(path: file, line: 10, column: nil))
    }

    @Test func resolvesFileLocationRootLevelDottedNameWithLineAndColumnSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "main.swift")
        let url = try #require(URL(string: "main.swift:42:7"))
        let location = TerminalBridge.resolveFileLocation(from: url, projectPath: dir.path)
        #expect(location == .init(path: file, line: 42, column: 7))
    }

    @Test func resolvesFileLocationRootLevelExtensionlessNameWithLineSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "Makefile")
        let url = try #require(URL(string: "Makefile:12"))
        let location = TerminalBridge.resolveFileLocation(from: url, projectPath: dir.path)
        #expect(location == .init(path: file, line: 12, column: nil))
    }

    @Test func resolvesFileLocationWithoutSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "plain.md")
        let url = try #require(URL(string: file))
        let location = TerminalBridge.resolveFileLocation(from: url, projectPath: "/unused")
        #expect(location == .init(path: file, line: nil, column: nil))
    }

    @Test func resolvesFileLocationPrefersRealFileWithColonInName() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "foo:12")
        let url = try #require(URL(string: file))
        let location = TerminalBridge.resolveFileLocation(from: url, projectPath: "/unused")
        #expect(location == .init(path: file, line: nil, column: nil))
    }

    @Test func resolvesTokenFileLocationWithLineSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let sub = dir.appendingPathComponent("Sources")
        try FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
        let file = writeFile(sub, name: "main.swift")
        let location = TerminalBridge.resolveFileLocation(from: "Sources/main.swift:42", projectPath: dir.path)
        #expect(location == .init(path: file, line: 42, column: nil))
    }

    @Test func resolvesTokenFileLocationWithLineAndColumnSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let sub = dir.appendingPathComponent("Sources")
        try FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
        let file = writeFile(sub, name: "main.swift")
        let location = TerminalBridge.resolveFileLocation(from: "Sources/main.swift:42:7", projectPath: dir.path)
        #expect(location == .init(path: file, line: 42, column: 7))
    }

    @Test func resolvesTokenFileLocationPrefersRealFileWithColonInName() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = writeFile(dir, name: "foo:12")
        let location = TerminalBridge.resolveFileLocation(from: "foo:12", projectPath: dir.path)
        #expect(location == .init(path: file, line: nil, column: nil))
    }

    @Test func resolvesWrappedTokenFileLocationFromPreviousLineSuffix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let services = dir.appendingPathComponent("muxy/Tests/MuxyTests/Services")
        try FileManager.default.createDirectory(at: services, withIntermediateDirectories: true)
        let file = writeFile(services, name: "TerminalBridgeResolveFilePathTests.swift")
        let lines = [
            "test file: muxy/Tests/MuxyTests/Services/",
            "  TerminalBridgeResolveFilePathTests.swift:179",
        ]
        let candidate = try #require(GhosttyTerminalNSView.wrappedFileTokenCandidates(
            word: "TerminalBridgeResolveFilePathTests.swift:179",
            row: 1,
            lines: lines
        ).first)
        let location = TerminalBridge.resolveFileLocation(from: candidate, projectPath: dir.path)
        #expect(location == .init(path: file, line: 179, column: nil))
    }

    @Test func resolvesWrappedTokenFileLocationFromNextLinePrefix() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let services = dir.appendingPathComponent("muxy/Tests/MuxyTests/Services")
        try FileManager.default.createDirectory(at: services, withIntermediateDirectories: true)
        let file = writeFile(services, name: "TerminalBridgeResolveFilePathTests.swift")
        let lines = [
            "test file: muxy/Tests/MuxyTests/Services/",
            "  TerminalBridgeResolveFilePathTests.swift:179",
        ]
        let candidate = try #require(GhosttyTerminalNSView.wrappedFileTokenCandidates(
            word: "muxy/Tests/MuxyTests/Services/",
            row: 0,
            lines: lines
        ).first)
        let location = TerminalBridge.resolveFileLocation(from: candidate, projectPath: dir.path)
        #expect(location == .init(path: file, line: 179, column: nil))
    }

    @Test func resolvesMultiLineWrappedTokenFileLocationFromFirstSegment() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let services = dir.appendingPathComponent("Tests/MuxyTests/Services")
        try FileManager.default.createDirectory(at: services, withIntermediateDirectories: true)
        let file = writeFile(services, name: "TerminalBridgeResolveFilePathTests.swift")
        let lines = [
            "test file: Tests/",
            "    MuxyTests/Services/",
            "    TerminalBridgeResolveFilePathTests.swift",
            "    :210",
        ]
        let candidate = try #require(GhosttyTerminalNSView.wrappedFileTokenCandidates(
            word: "Tests",
            row: 0,
            lines: lines
        ).first)
        let location = TerminalBridge.resolveFileLocation(from: candidate, projectPath: dir.path)
        #expect(location == .init(path: file, line: 210, column: nil))
    }

    @Test func resolvesMultiLineWrappedTokenFileLocationFromMiddleDirectoryWord() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let services = dir.appendingPathComponent("Tests/MuxyTests/Services")
        try FileManager.default.createDirectory(at: services, withIntermediateDirectories: true)
        let file = writeFile(services, name: "TerminalBridgeResolveFilePathTests.swift")
        let lines = [
            "test file: Tests/",
            "    MuxyTests/Services/",
            "    TerminalBridgeResolveFilePathTests.swift",
            "    :210",
        ]
        let candidate = try #require(GhosttyTerminalNSView.wrappedFileTokenCandidates(
            word: "Services",
            row: 1,
            lines: lines
        ).first)
        let location = TerminalBridge.resolveFileLocation(from: candidate, projectPath: dir.path)
        #expect(location == .init(path: file, line: 210, column: nil))
    }

    @Test func resolvesMultiLineWrappedTokenFileLocationFromFileNameSegment() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let services = dir.appendingPathComponent("Tests/MuxyTests/Services")
        try FileManager.default.createDirectory(at: services, withIntermediateDirectories: true)
        let file = writeFile(services, name: "TerminalBridgeResolveFilePathTests.swift")
        let lines = [
            "test file: Tests/",
            "    MuxyTests/Services/",
            "    TerminalBridgeResolveFilePathTests.swift",
            "    :210",
        ]
        let candidate = try #require(GhosttyTerminalNSView.wrappedFileTokenCandidates(
            word: "TerminalBridgeResolveFilePathTests.swift",
            row: 2,
            lines: lines
        ).first)
        let location = TerminalBridge.resolveFileLocation(from: candidate, projectPath: dir.path)
        #expect(location == .init(path: file, line: 210, column: nil))
    }

    @Test func resolvesMultiLineWrappedTokenFileLocationFromLineSuffixSegment() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let services = dir.appendingPathComponent("Tests/MuxyTests/Services")
        try FileManager.default.createDirectory(at: services, withIntermediateDirectories: true)
        let file = writeFile(services, name: "TerminalBridgeResolveFilePathTests.swift")
        let lines = [
            "test file: Tests/",
            "    MuxyTests/Services/",
            "    TerminalBridgeResolveFilePathTests.swift",
            "    :210",
        ]
        let candidate = try #require(GhosttyTerminalNSView.wrappedFileTokenCandidates(
            word: "210",
            row: 3,
            lines: lines
        ).first)
        let location = TerminalBridge.resolveFileLocation(from: candidate, projectPath: dir.path)
        #expect(location == .init(path: file, line: 210, column: nil))
    }

    @Test func resolvesWrappedAbsolutePathSplitInsideName() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let parent = dir.appendingPathComponent("dev/_references")
        try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: true)
        let file = writeFile(parent, name: "muxy-extensions.broken-20260625110716")
        let lines = [
            "moved to:",
            "    \(parent.path)/muxy-",
            "    extensions.broken-20260625110716",
        ]
        let candidate = try #require(GhosttyTerminalNSView.wrappedFileTokenCandidates(
            word: "extensions.broken-20260625110716",
            row: 2,
            lines: lines
        ).first)
        let location = TerminalBridge.resolveFileLocation(from: candidate, projectPath: "/unused")
        #expect(location == .init(path: file, line: nil, column: nil))
    }

    @Test func resolvesFileLocationRejectsUnresolvableSchemeless() throws {
        let url = try #require(URL(string: "/tmp/does-not-exist-\(UUID().uuidString).md:12"))
        #expect(TerminalBridge.resolveFileLocation(from: url, projectPath: "/unused") == nil)
    }

    @Test func resolvesFileLocationRejectsHttpURL() throws {
        let url = try #require(URL(string: "https://example.com/readme.md:12"))
        #expect(TerminalBridge.resolveFileLocation(from: url, projectPath: "/tmp") == nil)
    }

    @Test func relativePathInsideProject() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let sub = dir.appendingPathComponent("Sources")
        try FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
        let file = writeFile(sub, name: "main.swift")

        #expect(TerminalBridge.relativePath(file, inside: dir.path) == "Sources/main.swift")
    }

    @Test func relativePathRejectsProjectRootAndOutsideProject() throws {
        let dir = makeTempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let outside = writeFile(URL(fileURLWithPath: NSTemporaryDirectory()), name: "outside-\(UUID().uuidString).txt")
        defer { try? FileManager.default.removeItem(atPath: outside) }

        #expect(TerminalBridge.relativePath(dir.path, inside: dir.path) == nil)
        #expect(TerminalBridge.relativePath(outside, inside: dir.path) == nil)
    }

    @Test func stripsLineSuffixOnlyWhenTrailingNumeric() {
        #expect(TerminalBridge.stripLineColumnSuffix(from: "a.txt:12") == .init(path: "a.txt", line: 12, column: nil))
        #expect(TerminalBridge.stripLineColumnSuffix(from: "a.txt:12:7") == .init(path: "a.txt", line: 12, column: 7))
        #expect(TerminalBridge.stripLineColumnSuffix(from: "a.txt:12:notnum") == nil)
        #expect(TerminalBridge.stripLineColumnSuffix(from: "a.txt") == nil)
        #expect(TerminalBridge.stripLineColumnSuffix(from: ":12") == nil)
    }

    @Test func treatsWebAndOpaqueSchemesAsExternalLinks() throws {
        #expect(TerminalBridge.isExternalLink(try #require(URL(string: "https://example.com/x"))))
        #expect(TerminalBridge.isExternalLink(try #require(URL(string: "mailto:a@b.com"))))
        #expect(TerminalBridge.isExternalLink(try #require(URL(string: "tel:+123"))))
        #expect(TerminalBridge.isExternalLink(try #require(URL(string: "vscode://file/x"))))
        #expect(TerminalBridge.isExternalLink(try #require(URL(string: "ssh:host"))))
        #expect(TerminalBridge.isExternalLink(try #require(URL(string: "spotify:track:abc"))))
    }

    @Test func treatsSchemelessAndMisparsedPathsAsNonExternal() throws {
        #expect(!TerminalBridge.isExternalLink(try #require(URL(string: "/tmp/x.md:12"))))
        #expect(!TerminalBridge.isExternalLink(try #require(URL(string: "notes.md:5"))))
        #expect(!TerminalBridge.isExternalLink(try #require(URL(string: "reviews/results.md:12"))))
        #expect(!TerminalBridge.isExternalLink(try #require(URL(string: "main.swift:42:7"))))
        #expect(!TerminalBridge.isExternalLink(try #require(URL(string: "Makefile:12"))))
    }
}
