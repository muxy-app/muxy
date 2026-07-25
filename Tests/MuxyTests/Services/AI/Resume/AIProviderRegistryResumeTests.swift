import Foundation
import Testing

@testable import Muxy

@Suite("AIProviderRegistry resume lookup")
@MainActor
struct AIProviderRegistryResumeTests {
    @Test("claude resolves a resume strategy")
    func claudeResume() {
        let registry = AIProviderRegistry.shared
        let provider = registry.resumeProvider(forProviderID: "claude")
        #expect(provider?.resumeStrategy?.resumeCommand(sessionID: "x", cwd: "/p") == "claude --resume x")
    }

    @Test("agy and hermes are registered")
    func newProvidersRegistered() {
        let registry = AIProviderRegistry.shared
        #expect(registry.resumeProvider(forProviderID: "agy") != nil)
        #expect(registry.resumeProvider(forProviderID: "hermes") != nil)
    }
}
