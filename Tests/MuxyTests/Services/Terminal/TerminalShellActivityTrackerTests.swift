import Foundation
import Testing

@testable import Muxy

@Suite("TerminalShellActivityTracker")
struct TerminalShellActivityTrackerTests {
    @Test("tracks semantic command start and finish")
    func tracksSemanticCommandLifecycle() {
        let tracker = TerminalShellActivityTracker()
        let session = tracker.beginSession()

        session.recordOutput(Data("\u{1B}]133;C\u{07}".utf8))
        #expect(tracker.isCommandRunning)

        session.recordOutput(Data("\u{1B}]133;D;0\u{07}".utf8))
        #expect(!tracker.isCommandRunning)
    }

    @Test("tracks split semantic sequences")
    func tracksSplitSemanticSequences() {
        let tracker = TerminalShellActivityTracker()
        let session = tracker.beginSession()

        session.recordOutput(Data("\u{1B}]13".utf8))
        session.recordOutput(Data("3;C;cmdline=read".utf8))
        #expect(!tracker.isCommandRunning)

        session.recordOutput(Data("\u{07}".utf8))
        #expect(tracker.isCommandRunning)

        session.recordOutput(Data("\u{1B}]133;D".utf8))
        session.recordOutput(Data([0x1B, 0x5C]))
        #expect(!tracker.isCommandRunning)
    }

    @Test("ignores incomplete and unrelated output")
    func ignoresIncompleteAndUnrelatedOutput() {
        let tracker = TerminalShellActivityTracker()
        let session = tracker.beginSession()

        session.recordOutput(Data("output \u{1B}]133;C".utf8))
        #expect(!tracker.isCommandRunning)

        session.recordOutput(Data("\u{1B}]133;A\u{07}\u{1B}]133;B\u{07}".utf8))
        #expect(!tracker.isCommandRunning)
    }

    @Test("a new session rejects output retained by the detached session")
    func detachedSessionCannotMutateCurrentState() {
        let tracker = TerminalShellActivityTracker()
        let detachedSession = tracker.beginSession()

        detachedSession.recordOutput(Data("\u{1B}]133;C\u{07}".utf8))
        let currentSession = tracker.beginSession()
        currentSession.recordOutput(Data("\u{1B}]133;C\u{07}".utf8))
        detachedSession.recordOutput(Data("\u{1B}]133;D\u{07}".utf8))

        #expect(tracker.isCommandRunning)

        currentSession.invalidate()
        #expect(!tracker.isCommandRunning)
    }

    @Test("tracks semantic sequences with long payloads")
    func tracksLongSemanticSequence() {
        let tracker = TerminalShellActivityTracker()
        let session = tracker.beginSession()
        let payload = String(repeating: "x", count: 16_384)

        session.recordOutput(Data("\u{1B}]133;C;cmdline=\(payload)\u{07}".utf8))

        #expect(tracker.isCommandRunning)
    }
}
