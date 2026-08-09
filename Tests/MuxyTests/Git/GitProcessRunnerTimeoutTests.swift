import Foundation
import Testing

@testable import Muxy

@Suite("GitProcessRunner timeout")
struct GitProcessRunnerTimeoutTests {
    @Test("terminates a local git command when its timeout expires")
    func localGitCommandTimesOut() async throws {
        let repository = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-git-timeout-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repository, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: repository) }
        let initialized = try await GitProcessRunner.runGit(
            repoPath: repository.path,
            arguments: ["init", "-q"]
        )
        #expect(initialized.status == 0)

        await #expect(throws: GitProcessError.self) {
            try await GitProcessRunner.runGit(
                repoPath: repository.path,
                arguments: ["-c", "alias.wait=!sleep 10 &", "wait"],
                timeout: 0.5
            )
        }
    }
}
