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
}
