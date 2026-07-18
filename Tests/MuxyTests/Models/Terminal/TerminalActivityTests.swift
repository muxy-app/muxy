import Testing

@testable import Muxy

@Suite("TerminalActivity")
struct TerminalActivityTests {
    @Test("explicit progress has highest priority")
    func explicitProgressWins() {
        let progress = TerminalProgress(kind: .set, percent: 40)
        let activity = TerminalActivity.resolve(
            progress: progress,
            agentStatus: .waiting,
            unreadCount: 2,
            completionPending: true
        )
        #expect(activity == .working(progress))
    }

    @Test("agent working takes priority over waiting indicators")
    func agentWorkingWins() {
        let activity = TerminalActivity.resolve(
            progress: nil,
            agentStatus: .working,
            unreadCount: 2,
            completionPending: true
        )
        #expect(activity == .working(TerminalProgress(kind: .indeterminate, percent: nil)))
    }

    @Test("waiting takes priority over unread and finished")
    func waitingWins() {
        let activity = TerminalActivity.resolve(
            progress: nil,
            agentStatus: .waiting,
            unreadCount: 2,
            completionPending: true
        )
        #expect(activity == .waiting)
    }

    @Test("unread takes priority over finished")
    func unreadWins() {
        let activity = TerminalActivity.resolve(
            progress: nil,
            agentStatus: .idle,
            unreadCount: 2,
            completionPending: true
        )
        #expect(activity == .unread(2))
    }

    @Test("finished appears without higher-priority activity")
    func finishedAppears() {
        let activity = TerminalActivity.resolve(
            progress: nil,
            agentStatus: .idle,
            unreadCount: 0,
            completionPending: true
        )
        #expect(activity == .finished)
    }

    @Test("idle state without pending completion has no indicator")
    func idleHasNoIndicator() {
        let activity = TerminalActivity.resolve(
            progress: nil,
            agentStatus: .idle,
            unreadCount: 0,
            completionPending: false
        )
        #expect(activity == nil)
    }
}
