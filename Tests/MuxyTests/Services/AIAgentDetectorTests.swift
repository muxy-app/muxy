import Foundation
import Testing

@testable import Muxy

@Suite("AIAgentDetector")
struct AIAgentDetectorTests {
    private let executables: [AIAgentExecutable] = [
        AIAgentExecutable(providerID: "claude", executableNames: ["claude"]),
        AIAgentExecutable(providerID: "codex", executableNames: ["codex"]),
        AIAgentExecutable(providerID: "cursor", executableNames: ["cursor-agent", "cursor"]),
        AIAgentExecutable(providerID: "opencode", executableNames: ["opencode"]),
    ]

    @Test("matches a bare executable name to its provider id")
    func matchesBareName() {
        #expect(AIAgentDetector.providerID(forProcessName: "claude", executables: executables) == "claude")
        #expect(AIAgentDetector.providerID(forProcessName: "codex", executables: executables) == "codex")
    }

    @Test("matches an alternate executable name")
    func matchesAlternateName() {
        #expect(AIAgentDetector.providerID(forProcessName: "cursor-agent", executables: executables) == "cursor")
        #expect(AIAgentDetector.providerID(forProcessName: "cursor", executables: executables) == "cursor")
    }

    @Test("strips a full path before matching")
    func stripsPath() {
        #expect(AIAgentDetector.providerID(forProcessName: "/opt/homebrew/bin/codex", executables: executables) == "codex")
        #expect(AIAgentDetector.providerID(forProcessName: "/usr/local/bin/claude", executables: executables) == "claude")
    }

    @Test("ignores trailing command arguments")
    func ignoresArguments() {
        #expect(AIAgentDetector.providerID(forProcessName: "claude --resume", executables: executables) == "claude")
    }

    @Test("matches case-insensitively")
    func matchesCaseInsensitively() {
        #expect(AIAgentDetector.providerID(forProcessName: "Claude", executables: executables) == "claude")
        #expect(AIAgentDetector.providerID(forProcessName: "OPENCODE", executables: executables) == "opencode")
    }

    @Test("returns nil for shells and unrelated programs")
    func returnsNilForNonAgents() {
        #expect(AIAgentDetector.providerID(forProcessName: "zsh", executables: executables) == nil)
        #expect(AIAgentDetector.providerID(forProcessName: "vim", executables: executables) == nil)
        #expect(AIAgentDetector.providerID(forProcessName: "ssh", executables: executables) == nil)
    }

    @Test("returns nil for empty or missing names")
    func returnsNilForEmpty() {
        #expect(AIAgentDetector.providerID(forProcessName: nil, executables: executables) == nil)
        #expect(AIAgentDetector.providerID(forProcessName: "", executables: executables) == nil)
        #expect(AIAgentDetector.providerID(forProcessName: "   ", executables: executables) == nil)
    }

    @Test("matches when any candidate name is an agent")
    func matchesAnyCandidate() {
        #expect(
            AIAgentDetector.providerID(
                forCandidateNames: ["node", "codex"],
                executables: executables
            ) == "codex"
        )
        #expect(
            AIAgentDetector.providerID(
                forCandidateNames: ["/usr/bin/node", "/Users/me/.local/bin/pi"],
                executables: [AIAgentExecutable(providerID: "pi", executableNames: ["pi"])]
            ) == "pi"
        )
    }

    @Test("returns nil when no candidate is an agent")
    func returnsNilForNoCandidate() {
        #expect(AIAgentDetector.providerID(forCandidateNames: ["node", "zsh"], executables: executables) == nil)
        #expect(AIAgentDetector.providerID(forCandidateNames: [], executables: executables) == nil)
    }
}
