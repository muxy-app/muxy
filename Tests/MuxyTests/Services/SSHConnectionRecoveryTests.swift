import Foundation
import NIOCore
import Testing

@testable import MuxySSH

@Suite("SSH connection recovery")
struct SSHConnectionRecoveryTests {
    @Test("interactive channel EOF is retryable")
    func interactiveChannelEOFIsRetryable() {
        let disposition = SSHConnectionRecoveryDecision.disposition(
            host: "prod",
            sessionMode: .interactive,
            remoteExit: nil,
            error: ChannelError.eof
        )

        #expect(disposition == .retryable(.disconnected("Connection to prod was lost")))
    }

    @Test("authentication failure is fatal")
    func authenticationFailureIsFatal() {
        let disposition = SSHConnectionRecoveryDecision.disposition(
            host: "prod",
            sessionMode: .interactive,
            remoteExit: nil,
            error: SSHConnectionFailure.unknownHostKey
        )

        #expect(disposition == .failed(error: .unknownHostKey("prod"), retryable: false))
    }

    @Test("interactive remote shell exit does not close the tab")
    func interactiveRemoteShellExitDoesNotCloseTheTab() {
        let disposition = SSHConnectionRecoveryDecision.disposition(
            host: "prod",
            sessionMode: .interactive,
            remoteExit: .status(0),
            error: nil
        )

        #expect(disposition == .failed(error: .sessionEnded("The remote shell exited with status 0."), retryable: false))
    }

    @Test("exec remote exit closes the tab")
    func execRemoteExitClosesTheTab() {
        let disposition = SSHConnectionRecoveryDecision.disposition(
            host: "prod",
            sessionMode: .exec,
            remoteExit: .status(0),
            error: nil
        )

        #expect(disposition == .close)
    }

    @Test("timeout is retryable for interactive sessions")
    func timeoutIsRetryableForInteractiveSessions() {
        let disposition = SSHConnectionRecoveryDecision.disposition(
            host: "prod",
            sessionMode: .interactive,
            remoteExit: nil,
            error: POSIXError(.ETIMEDOUT)
        )

        #expect(disposition == .retryable(.timeout("prod")))
    }

    @Test("refused is fatal for interactive sessions")
    func refusedIsFatalForInteractiveSessions() {
        let disposition = SSHConnectionRecoveryDecision.disposition(
            host: "prod",
            sessionMode: .interactive,
            remoteExit: nil,
            error: POSIXError(.ECONNREFUSED)
        )

        #expect(disposition == .failed(error: .refused("prod"), retryable: false))
    }

    @Test("reconnect backoff uses the fixed retry schedule")
    func reconnectBackoffUsesTheFixedRetrySchedule() {
        #expect(SSHReconnectPolicy.delay(forAttempt: 1) == 1)
        #expect(SSHReconnectPolicy.delay(forAttempt: 2) == 2)
        #expect(SSHReconnectPolicy.delay(forAttempt: 3) == 5)
        #expect(SSHReconnectPolicy.delay(forAttempt: 4) == 10)
        #expect(SSHReconnectPolicy.delay(forAttempt: 5) == 20)
        #expect(SSHReconnectPolicy.delay(forAttempt: 6) == nil)
    }
}
