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
}
