import Foundation
import Testing

@testable import Muxy

@Suite("Repository AI text generator")
struct RepositoryAITextGeneratorTests {
    @Test("local launches preserve structured arguments and inject the login-shell PATH")
    func localLaunch() {
        let invocation = AIAgentInvocation(
            executable: "claude",
            arguments: ["--print", "metadata; touch /tmp/ignored"],
            environment: ["MUXY_TEST": "value with spaces"]
        )

        let launch = RepositoryAITextGenerator().resolvedLaunch(
            invocation: invocation,
            workingDirectory: "/tmp/muxy repository",
            context: .local
        )

        #expect(launch.executable == "/usr/bin/env")
        #expect(launch.workingDirectory == "/tmp/muxy repository")
        #expect(launch.arguments == [
            "MUXY_PANE_ID=",
            "MUXY_TEST=value with spaces",
            "PATH=\(LoginShellPath.current)",
            "claude",
            "--print",
            "metadata; touch /tmp/ignored",
        ])
    }

    @Test("remote launches use non-interactive SSH without flattening provider arguments locally")
    func remoteLaunch() {
        let destination = SSHDestination(host: "example.com", remoteRoot: "/srv")
        let invocation = AIAgentInvocation(
            executable: "codex",
            arguments: ["exec", "--sandbox", "read-only", "metadata"],
            environment: [:]
        )

        let launch = RepositoryAITextGenerator().resolvedLaunch(
            invocation: invocation,
            workingDirectory: "/srv/muxy repository",
            context: .ssh(destination)
        )

        #expect(launch.executable == "/usr/bin/ssh")
        #expect(launch.arguments.contains("-T"))
        #expect(launch.arguments.contains("example.com"))
        #expect(launch.arguments.last?.contains("codex") == true)
        #expect(launch.arguments.last?.contains("read-only") == true)
        #expect(launch.arguments.last?.contains("export MUXY_PANE_ID=") == true)
    }

    @Test("provider failure detail favors stderr, strips controls, and caps displayed output")
    func failureDetail() {
        let stderr = "\u{0000}" + String(repeating: "x", count: 1_100) + "\nfinal"
        let detail = RepositoryAITextGenerator.failureDetail(stdout: "stdout", stderr: stderr)

        #expect(!detail.contains("\u{0000}"))
        #expect(detail.count == 1_000)
        #expect(detail.hasSuffix("final"))
    }

    @Test("provider failure detail has a useful fallback")
    func emptyFailureDetail() {
        #expect(RepositoryAITextGenerator.failureDetail(stdout: "", stderr: "") ==
            "The command exited without an error message.")
    }

    @Test("process output limits cap memory and report truncation")
    func outputLimit() async throws {
        let result = try await GitProcessRunner.runResolved(
            ResolvedLaunch(
                executable: "/usr/bin/printf",
                arguments: ["%s", String(repeating: "x", count: 1_024)],
                workingDirectory: nil
            ),
            outputByteLimit: 64
        )

        #expect(result.stdoutData.count == 64)
        #expect(result.stdout == String(repeating: "x", count: 64))
        #expect(result.truncated)
    }

    @Test("byte limits remain active when a line limit is also configured")
    func combinedOutputLimits() async throws {
        let result = try await GitProcessRunner.runResolved(
            ResolvedLaunch(
                executable: "/usr/bin/printf",
                arguments: ["%s", String(repeating: "x", count: 1024)],
                workingDirectory: nil
            ),
            lineLimit: 800,
            outputByteLimit: 64
        )

        #expect(result.stdoutData.count == 64)
        #expect(result.truncated)
    }

    @Test("silent providers are terminated after the metadata timeout")
    func providerTimeout() async {
        let configuration = AIAgentLaunchConfiguration(
            executable: "/bin/sh",
            headlessArguments: ["-c", "while :; do :; done", "--"]
        )

        await #expect(throws: RepositoryAITextGeneratorError.timedOut("Test Provider")) {
            try await RepositoryAITextGenerator(timeout: .milliseconds(20)).generate(
                prompt: "metadata",
                configuration: configuration,
                providerName: "Test Provider",
                workingDirectory: "/tmp",
                context: .local
            )
        }
    }
}
