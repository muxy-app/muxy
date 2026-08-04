import Foundation
import MuxySessionProtocol
import Testing

@testable import Muxy

@Suite("PersistentSessionExitHandler decisions")
struct PersistentSessionExitHandlerTests {
    private let descriptor = SessionDescriptor(
        identifier: SessionIdentifier(uuidString: UUID().uuidString)!,
        shellProcessID: 100,
        ttyDevice: 1,
        workingDirectory: "/tmp",
        isAttached: false
    )

    @Test("closes the pane when the session really ended")
    func closesPaneForEndedSession() {
        #expect(PersistentSessionExitHandler.decision(lookup: .absent, attempt: 1, limit: 3) == .closePane)
    }

    @Test("closes the pane for an ended session regardless of earlier retries")
    func closesPaneEvenAfterRetries() {
        #expect(PersistentSessionExitHandler.decision(lookup: .absent, attempt: 9, limit: 3) == .closePane)
    }

    @Test("reattaches while the session is still alive")
    func reattachesLiveSession() {
        for attempt in 1 ... 3 {
            #expect(PersistentSessionExitHandler.decision(
                lookup: .found(descriptor),
                attempt: attempt,
                limit: 3
            ) == .reattach)
        }
    }

    @Test("reattaches when the daemon cannot be reached")
    func reattachesUnreachableDaemon() {
        #expect(PersistentSessionExitHandler.decision(lookup: .unreachable, attempt: 1, limit: 3) == .reattach)
    }

    @Test("reports a failure once the retry budget is spent")
    func reportsFailureAfterBudget() {
        #expect(PersistentSessionExitHandler.decision(
            lookup: .found(descriptor),
            attempt: 4,
            limit: 3
        ) == .reportFailure)
        #expect(PersistentSessionExitHandler.decision(
            lookup: .unreachable,
            attempt: 4,
            limit: 3
        ) == .reportFailure)
    }

    @Test("counts the first exit as the first attempt")
    func countsFirstAttempt() {
        #expect(PersistentSessionExitHandler.attempt(previous: 0, elapsed: nil, resetAfter: 60) == 1)
    }

    @Test("keeps counting attempts inside the reset window")
    func countsRepeatedAttempts() {
        #expect(PersistentSessionExitHandler.attempt(previous: 2, elapsed: 5, resetAfter: 60) == 3)
    }

    @Test("starts over once the session has been healthy for a while")
    func resetsAfterHealthyPeriod() {
        #expect(PersistentSessionExitHandler.attempt(previous: 3, elapsed: 61, resetAfter: 60) == 1)
    }

    @Test("backs off further with every attempt")
    func backsOffProgressively() {
        #expect(PersistentSessionExitHandler.backoff(attempt: 1) == .milliseconds(300))
        #expect(PersistentSessionExitHandler.backoff(attempt: 3) == .milliseconds(900))
        #expect(PersistentSessionExitHandler.backoff(attempt: 0) == .milliseconds(300))
    }
}

@Suite("TerminalPersistentSessionPolicy idleness")
struct TerminalPersistentSessionIdlePolicyTests {
    @Test("treats an idle session shell as idle")
    func idleSessionIsIdle() {
        #expect(TerminalPersistentSessionPolicy.isIdle(
            activity: .idle,
            isShellCommandRunning: false,
            isAlternateScreen: false
        ))
    }

    @Test("keeps a session running a command awake")
    func runningCommandIsBusy() {
        #expect(!TerminalPersistentSessionPolicy.isIdle(
            activity: .running,
            isShellCommandRunning: false,
            isAlternateScreen: false
        ))
    }

    @Test("keeps a session awake while shell integration reports a command")
    func shellIntegrationCommandIsBusy() {
        #expect(!TerminalPersistentSessionPolicy.isIdle(
            activity: .idle,
            isShellCommandRunning: true,
            isAlternateScreen: false
        ))
    }

    @Test("keeps a session showing a full screen program awake")
    func alternateScreenIsBusy() {
        #expect(!TerminalPersistentSessionPolicy.isIdle(
            activity: .idle,
            isShellCommandRunning: false,
            isAlternateScreen: true
        ))
    }

    @Test("never sleeps a session whose state is unknown")
    func unknownActivityStaysAwake() {
        #expect(!TerminalPersistentSessionPolicy.isIdle(
            activity: nil,
            isShellCommandRunning: false,
            isAlternateScreen: false
        ))
    }
}
