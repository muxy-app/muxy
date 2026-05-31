import Foundation
import Testing

@testable import Muxy

@Suite("AIProviderRegistry")
@MainActor
struct AIProviderRegistryTests {
    @Test("notificationSource resolves built-in socket type keys")
    func notificationSourceResolvesBuiltIn() {
        let source = AIProviderRegistry.shared.notificationSource(for: "claude_hook")
        #expect(source == .aiProvider("claude"))
    }

    @Test("notificationSource falls back to socket for unknown types")
    func notificationSourceFallsBackToSocket() {
        let source = AIProviderRegistry.shared.notificationSource(for: "not-a-known-type")
        #expect(source == .socket)
    }

    @Test("iconName resolves a built-in provider icon")
    func iconNameResolvesBuiltIn() {
        #expect(AIProviderRegistry.shared.iconName(for: .aiProvider("claude")) == "claude")
    }

    @Test("iconName falls back to sparkles for an extension source")
    func iconNameFallsBackForExtension() {
        #expect(AIProviderRegistry.shared.iconName(for: .aiProvider("some-extension")) == "sparkles")
    }

    @Test("iconName resolves osc and socket sources")
    func iconNameResolvesStaticSources() {
        #expect(AIProviderRegistry.shared.iconName(for: .osc) == "terminal")
        #expect(AIProviderRegistry.shared.iconName(for: .socket) == "network")
    }
}
