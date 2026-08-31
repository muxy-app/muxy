import Foundation
import Testing
@testable import Muxy

@Suite("Terminal process exit policy")
struct TerminalProcessExitPolicyTests {
    @Test("recovers remote connections instead of closing their panes")
    func recoversRemoteConnection() {
        #expect(TerminalProcessExitPolicy.disposition(
            isRemote: true,
            persistentSessionID: nil
        ) == .recoverRemoteConnection)
    }

    @Test("remote recovery takes precedence over persistent session recovery")
    func prioritizesRemoteRecovery() {
        #expect(TerminalProcessExitPolicy.disposition(
            isRemote: true,
            persistentSessionID: UUID()
        ) == .recoverRemoteConnection)
    }

    @Test("recovers persistent local sessions")
    func recoversPersistentLocalSession() {
        let sessionID = UUID()
        #expect(TerminalProcessExitPolicy.disposition(
            isRemote: false,
            persistentSessionID: sessionID
        ) == .recoverPersistentSession(sessionID))
    }

    @Test("closes non-persistent local panes")
    func closesNonPersistentLocalPane() {
        #expect(TerminalProcessExitPolicy.disposition(
            isRemote: false,
            persistentSessionID: nil
        ) == .closePane)
    }
}
