import Foundation
import Testing

@testable import Muxy

@Suite("GitProcessRunner timeout")
struct GitProcessRunnerTimeoutTests {
    @Test("terminates a local git command when its timeout expires")
    func localGitCommandTimesOut() async {
        await #expect(throws: GitProcessError.self) {
            try await GitProcessRunner.runGit(
                repoPath: NSTemporaryDirectory(),
                arguments: ["-c", "alias.wait=!sleep 10 &", "wait"],
                timeout: 0.05
            )
        }
    }
}
