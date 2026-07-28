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

    @Test("timeout terminates a process stalled before reading standard input")
    func cancelsStalledInput() async {
        let input = Data(count: 4 * 1_024 * 1_024)
        let clock = ContinuousClock()
        let start = clock.now

        await #expect(throws: SSHCommandError.self) {
            try await SSHCommandRunner.withTimeout(0.05) {
                try await GitProcessRunner.runResolved(
                    ResolvedLaunch(
                        executable: "/bin/sh",
                        arguments: ["-c", "exec /bin/sleep 30"],
                        workingDirectory: nil
                    ),
                    stdinData: input
                )
            }
        }

        #expect(start.duration(to: clock.now) < .seconds(10))
    }

    @Test("timeout force kills a SIGTERM-resistant process that never reads input")
    func forceKillsTermResistantNonReader() async {
        let input = Data(count: 8 * 1_024 * 1_024)
        let clock = ContinuousClock()
        let start = clock.now

        await #expect(throws: SSHCommandError.self) {
            try await SSHCommandRunner.withTimeout(0.05) {
                try await GitProcessRunner.runResolved(
                    ResolvedLaunch(
                        executable: "/bin/sh",
                        arguments: ["-c", "trap '' TERM; while :; do :; done"],
                        workingDirectory: nil
                    ),
                    stdinData: input
                )
            }
        }

        #expect(start.duration(to: clock.now) < .seconds(10))
    }

    @Test("immediate cancellation still force kills before process attachment")
    func forceKillsAfterImmediateCancellation() async throws {
        let input = Data(count: 8 * 1_024 * 1_024)
        let clock = ContinuousClock()
        let start = clock.now
        let task = Task {
            try await GitProcessRunner.runResolved(
                ResolvedLaunch(
                    executable: "/bin/sh",
                    arguments: ["-c", "trap '' TERM; while :; do :; done"],
                    workingDirectory: nil
                ),
                stdinData: input
            )
        }

        task.cancel()
        let result = try await task.value

        #expect(result.truncated)
        #expect(start.duration(to: clock.now) < .seconds(10))
    }
}
