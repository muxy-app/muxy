import Foundation
import Testing

@testable import Muxy

@Suite("AI metadata providers")
struct AIAgentLaunchProviderTests {
    @Test("built-in providers use read-only non-interactive commands")
    func builtInProviderCommands() {
        let prompt = "generate metadata"
        let providers: [(any AIAgentLaunchProvider, [String])] = [
            (
                ClaudeCodeProvider(),
                [
                    "--print",
                    "--output-format",
                    "text",
                    "--permission-mode",
                    "dontAsk",
                    "--no-session-persistence",
                    "--tools=",
                    prompt,
                ]
            ),
            (OpenCodeProvider(), ["run", "--pure", prompt]),
            (CodexProvider(), ["exec", "--ephemeral", "--sandbox", "read-only", "--color", "never", prompt]),
            (CursorProvider(), ["--print", "--output-format", "text", prompt]),
            (DroidProvider(), ["exec", "--output-format", "text", prompt]),
            (PiProvider(), ["--print", "--no-session", "--no-tools", prompt]),
            (
                GrokProvider(),
                [
                    "--no-auto-update",
                    "--sandbox",
                    "workspace",
                    "--permission-mode",
                    "dontAsk",
                    "--no-subagents",
                    "--disable-web-search",
                    "--output-format",
                    "text",
                    "-p",
                    prompt,
                ]
            ),
        ]

        for (provider, expectedArguments) in providers {
            let invocation = provider.agentLaunchConfiguration.invocation(prompt: prompt)
            #expect(invocation?.arguments == expectedArguments)
        }
    }

    @Test("models and prompts remain individual process arguments")
    func structuredArguments() throws {
        let prompt = "Review $(touch /tmp/muxy) `whoami`; echo 'done' | cat\nthen respond"
        let model = "provider/model latest"
        let configuration = AIAgentLaunchConfiguration(
            executable: "codex",
            headlessArguments: ["exec", "--sandbox", "read-only"]
        )
        let invocation = try #require(configuration.invocation(prompt: prompt, model: model))

        #expect(invocation.executable == "codex")
        #expect(invocation.arguments == ["exec", "--sandbox", "read-only", "--model", model, prompt])
    }

    @Test("blank prompts do not produce invocations")
    func blankPromptIsRejected() {
        let configuration = AIAgentLaunchConfiguration(executable: "codex", headlessArguments: ["exec"])
        #expect(configuration.invocation(prompt: " \n ") == nil)
    }

    @Test("leading option prompts cannot become provider options")
    func positionalPromptCannotBecomeOption() {
        let configuration = AIAgentLaunchConfiguration(executable: "claude", headlessArguments: ["--print"])
        let invocation = configuration.invocation(prompt: "--dangerously-skip-permissions")

        #expect(invocation?.arguments == ["--print", " --dangerously-skip-permissions"])
    }

    @Test("OpenCode denies every tool for metadata generation")
    func openCodeDeniesTools() {
        let invocation = OpenCodeProvider().agentLaunchConfiguration.invocation(prompt: "metadata")
        #expect(invocation?.environment == ["OPENCODE_PERMISSION": #"{"*":"deny"}"#])
    }

    @Test("agent tabs launch installed local executables safely")
    func localAgentTabCommand() {
        let provider = AgentTabLaunchTestProvider(executablePath: "/tmp/Agent Tools/codex")

        #expect(AgentTabLaunchCommand.local(provider: provider) == "'/tmp/Agent Tools/codex'")
    }

    @Test("agent tabs omit unavailable local providers")
    func unavailableLocalAgentTabCommand() {
        let provider = AgentTabLaunchTestProvider(executablePath: nil)

        #expect(AgentTabLaunchCommand.local(provider: provider) == nil)
    }

    @Test("agent tabs escape remote executable names")
    func remoteAgentTabCommand() {
        let provider = AgentTabLaunchTestProvider(executablePath: nil)

        #expect(AgentTabLaunchCommand.remote(provider: provider) == "test-agent")
    }

    @Test("agent launch options resolve each local executable once")
    func launchOptionsSnapshotExecutableResolution() {
        let provider = CountingAgentTabLaunchTestProvider()

        let options = AgentTabLaunchOption.resolveLocal(providers: [provider])

        #expect(provider.resolutionCount == 1)
        #expect(options.first?.command == "/tmp/test-agent")
        #expect(options.first?.title == "Test Agent")
    }

    @Test("remote launch options disable providers missing from the remote PATH")
    func remoteLaunchOptionsReflectAvailability() {
        let provider = AgentTabLaunchTestProvider(executablePath: nil)

        let available = AgentTabLaunchOption.resolveRemote(
            providers: [provider],
            availableProviderIDs: ["test"]
        )
        let unavailable = AgentTabLaunchOption.resolveRemote(
            providers: [provider],
            availableProviderIDs: []
        )

        #expect(available.first?.command == "test-agent")
        #expect(available.first?.title == "Test Agent")
        #expect(unavailable.first?.command == nil)
        #expect(unavailable.first?.title == "Test Agent · Not installed")
    }

    @Test("remote provider availability uses a login shell and parses marked output")
    @MainActor
    func remoteProviderAvailability() async throws {
        let provider = AgentTabLaunchTestProvider(executablePath: nil)
        let destination = SSHDestination(host: "example.com")
        var capturedCommand = ""

        let providerIDs = try await RemoteAgentLaunchAvailability.resolve(
            providers: [provider],
            destination: destination
        ) { receivedDestination, command in
            capturedCommand = command
            #expect(receivedDestination == destination)
            return GitProcessResult(
                status: 0,
                stdout: """
                shell noise
                __MUXY_AGENT_PROVIDERS_START__
                test
                unsupported
                __MUXY_AGENT_PROVIDERS_END__
                """,
                stdoutData: Data(),
                stderr: "",
                truncated: false
            )
        }

        #expect(providerIDs == ["test"])
        #expect(capturedCommand.contains(#""${SHELL:-/bin/sh}" -l -i -c"#))
        #expect(capturedCommand.contains("command -v test-agent"))
    }

    @Test("remote provider availability surfaces command failures")
    @MainActor
    func remoteProviderAvailabilityFailure() async {
        let provider = AgentTabLaunchTestProvider(executablePath: nil)

        await #expect(throws: RemoteAgentLaunchAvailabilityError.self) {
            try await RemoteAgentLaunchAvailability.resolve(
                providers: [provider],
                destination: SSHDestination(host: "example.com")
            ) { _, _ in
                GitProcessResult(
                    status: 255,
                    stdout: "",
                    stdoutData: Data(),
                    stderr: "Connection failed",
                    truncated: false
                )
            }
        }
    }

    @Test("cancelled option loading cannot return a stale remote result")
    @MainActor
    func cancelledOptionLoadingRejectsResult() async {
        let provider = AgentTabLaunchTestProvider(executablePath: nil)
        var rejectedResult = false
        let task = Task { @MainActor in
            do {
                _ = try await AgentTabLaunchOptionsLoader.resolve(
                    context: .ssh(SSHDestination(host: "first.example.com")),
                    localResolver: { [] },
                    remoteResolver: { _ in
                        try? await Task.sleep(for: .milliseconds(100))
                        return [AgentTabLaunchOption(provider: provider, command: "test-agent")]
                    }
                )
            } catch is CancellationError {
                rejectedResult = true
            } catch {
                Issue.record(error)
            }
        }

        await Task.yield()
        task.cancel()
        await task.value

        #expect(rejectedResult)
    }

    @Test("option loading resolves against the supplied workspace context")
    @MainActor
    func optionLoadingUsesWorkspaceContext() async throws {
        let destination = SSHDestination(host: "target.example.com")
        var resolvedDestination: SSHDestination?

        _ = try await AgentTabLaunchOptionsLoader.resolve(
            context: .ssh(destination),
            localResolver: { [] },
            remoteResolver: { receivedDestination in
                resolvedDestination = receivedDestination
                return []
            }
        )

        #expect(resolvedDestination == destination)
    }
}

private struct AgentTabLaunchTestProvider: AIAgentLaunchProvider {
    let id = "test"
    let displayName = "Test Agent"
    let iconName = "sparkles"
    let executablePath: String?
    let agentLaunchConfiguration = AIAgentLaunchConfiguration(
        executable: "test-agent",
        headlessArguments: []
    )

    func agentCLIExecutablePath() -> String? {
        executablePath
    }

    func isAgentCLIInstalled() -> Bool {
        executablePath != nil
    }
}

private final class CountingAgentTabLaunchTestProvider: AIAgentLaunchProvider {
    let id = "counting-test"
    let displayName = "Test Agent"
    let iconName = "sparkles"
    let agentLaunchConfiguration = AIAgentLaunchConfiguration(
        executable: "test-agent",
        headlessArguments: []
    )
    private(set) var resolutionCount = 0

    func agentCLIExecutablePath() -> String? {
        resolutionCount += 1
        return "/tmp/test-agent"
    }

    func isAgentCLIInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }
}
