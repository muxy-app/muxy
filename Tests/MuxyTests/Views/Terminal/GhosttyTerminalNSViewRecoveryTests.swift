import Foundation
import Testing

@testable import Muxy

@Suite("Ghostty terminal remote recovery")
@MainActor
struct GhosttyTerminalNSViewRecoveryTests {
    @Test("Validated recovery titles reset state with a rotated token")
    func validatedRecoveryTitleResetsState() {
        let recoveryToken = UUID()
        let view = GhosttyTerminalNSView(
            workingDirectory: "~",
            workspaceContext: .ssh(SSHDestination(host: "prod")),
            remoteRecoveryToken: recoveryToken
        )
        var events: [String] = []
        var recreatedSurface = false
        view.onSearchEnd = { events.append("search-ended") }
        view.onRemoteSessionRecoveryFailed = { events.append("recovery-\($0)") }

        let forgedTitle = TerminalLaunchCommand.remoteReconnectRequiredTitle(recoveryToken: UUID())
        let validTitle = TerminalLaunchCommand.remoteReconnectRequiredTitle(recoveryToken: recoveryToken)

        #expect(!view.handleRemoteSessionRecoveryTitle(forgedTitle))
        #expect(!view.processExitHandled)
        #expect(view.handleRemoteSessionRecoveryTitle(validTitle))
        #expect(events == ["search-ended", "recovery-true"])
        #expect(view.isRemoteSessionRecoveryFailed)
        #expect(view.processExitHandled)

        view.retryRemoteSession {
            recreatedSurface = true
            return true
        }

        #expect(recreatedSurface)
        #expect(events == ["search-ended", "recovery-true", "recovery-false"])
        #expect(!view.isRemoteSessionRecoveryFailed)
        #expect(!view.processExitHandled)
        #expect(!view.handleRemoteSessionRecoveryTitle(validTitle))
    }
}
