import Testing

@testable import Muxy

@Suite("Terminal backend")
struct TerminalBackendTests {
    @Test("defaults missing and unknown persisted values to Ghostty")
    func defaultsInvalidPersistedValues() {
        #expect(TerminalBackend(persisted: nil) == .ghostty)
        #expect(TerminalBackend(persisted: "unknown") == .ghostty)
    }

    @Test("restores a known persisted backend")
    func restoresKnownPersistedBackend() {
        #expect(TerminalBackend(persisted: "ghostty") == .ghostty)
    }

    @Test("Ghostty exposes every capability used by current Muxy integrations")
    func ghosttyCapabilities() {
        let capabilities = TerminalBackend.ghostty.capabilities

        #expect(capabilities.contains(.rawOutput))
        #expect(capabilities.contains(.gridSnapshot))
        #expect(capabilities.contains(.clientTheme))
        #expect(capabilities.contains(.offlineLifecycle))
        #expect(capabilities.contains(.search))
        #expect(capabilities.contains(.imagePaste))
    }

    @MainActor
    @Test("creates a Ghostty surface through the backend factory")
    func createsGhosttySurface() {
        let surface = TerminalBackend.ghostty.makeSurface(launch: TerminalLaunchRequest(
            workingDirectory: "/tmp",
            command: nil,
            commandInteractive: false,
            closesOnCommandExit: true,
            workspaceContext: .local
        ))

        #expect(surface.backend == .ghostty)
        #expect(surface.capabilities == .ghostty)
        surface.tearDown()
    }
}
