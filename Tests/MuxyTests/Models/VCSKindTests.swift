import Foundation
import Testing

@testable import Muxy

@Suite("VCSKind")
struct VCSKindTests {
    @Test("detect returns git for a directory containing only .git")
    func detectGitOnly() async throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }
        try makeDirectory(at: tempDir.appendingPathComponent(".git"))

        let kind = await VCSKind.detect(at: tempDir.path)

        #expect(kind == .git)
    }

    @Test("detect returns jjNative for a directory containing only .jj")
    func detectJJNative() async throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }
        try makeDirectory(at: tempDir.appendingPathComponent(".jj"))

        let kind = await VCSKind.detect(at: tempDir.path)

        #expect(kind == .jjNative)
    }

    @Test("detect returns jjColocated for a directory containing both .jj and .git")
    func detectJJColocated() async throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }
        try makeDirectory(at: tempDir.appendingPathComponent(".jj"))
        try makeDirectory(at: tempDir.appendingPathComponent(".git"))

        let kind = await VCSKind.detect(at: tempDir.path)

        #expect(kind == .jjColocated)
    }

    @Test("detect returns nil for a directory with no VCS markers")
    func detectNilForNonRepo() async throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let kind = await VCSKind.detect(at: tempDir.path)

        #expect(kind == nil)
    }

    @Test("detect finds VCS from a nested subdirectory")
    func detectFromNestedSubdirectory() async throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }
        try makeDirectory(at: tempDir.appendingPathComponent(".git"))
        let nested = tempDir.appendingPathComponent("src/nested", isDirectory: true)
        try makeDirectory(at: nested)

        let kind = await VCSKind.detect(at: nested.path)

        #expect(kind == .git)
    }

    @Test("detect prefers jj over git when both are present at the same level")
    func detectPrefersJJWhenBothPresent() async throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }
        try makeDirectory(at: tempDir.appendingPathComponent(".jj"))
        try makeDirectory(at: tempDir.appendingPathComponent(".git"))

        let kind = await VCSKind.detect(at: tempDir.path)

        #expect(kind == .jjColocated)
    }

    @Test("isJujutsu returns true for jjColocated and jjNative")
    func isJujutsuForJJVariants() {
        #expect(VCSKind.jjColocated.isJujutsu)
        #expect(VCSKind.jjNative.isJujutsu)
    }

    @Test("isJujutsu returns false for git")
    func isJujutsuForGit() {
        #expect(!VCSKind.git.isJujutsu)
    }

    @Test("displayName returns correct values")
    func displayNames() {
        #expect(VCSKind.git.displayName == "Git")
        #expect(VCSKind.jjColocated.displayName == "Jujutsu (colocated)")
        #expect(VCSKind.jjNative.displayName == "Jujutsu")
    }

    @Test("allCases contains expected values")
    func allCases() {
        let cases = Set(VCSKind.allCases)
        #expect(cases == [.git, .jjColocated, .jjNative])
    }

    @Test("rawValue round-trips correctly")
    func rawValueRoundTrip() {
        #expect(VCSKind.git.rawValue == "git")
        #expect(VCSKind.jjColocated.rawValue == "jjColocated")
        #expect(VCSKind.jjNative.rawValue == "jjNative")
        #expect(VCSKind(rawValue: "git") == .git)
        #expect(VCSKind(rawValue: "jjColocated") == .jjColocated)
        #expect(VCSKind(rawValue: "jjNative") == .jjNative)
        #expect(VCSKind(rawValue: "unknown") == nil)
    }

    private func makeTempDir() throws -> URL {
        let url = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-vcskind-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func makeDirectory(at url: URL) throws {
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    }
}
