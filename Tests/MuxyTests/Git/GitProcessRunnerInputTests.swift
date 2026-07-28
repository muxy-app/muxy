import Foundation
import Testing

@testable import Muxy

@Suite("GitProcessRunner input")
struct GitProcessRunnerInputTests {
    @Test("streams standard input while draining output")
    func streamsInput() async throws {
        let input = Data("muxy image data".utf8)
        let result = try await GitProcessRunner.runResolved(
            ResolvedLaunch(
                executable: "/bin/cat",
                arguments: [],
                workingDirectory: nil
            ),
            stdinData: input
        )

        #expect(result.status == 0)
        #expect(result.stdoutData == input)
    }
}
