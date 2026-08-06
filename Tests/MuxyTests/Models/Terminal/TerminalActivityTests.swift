import Testing

@testable import Muxy

@Suite("TerminalActivity")
struct TerminalActivityTests {
    @Test("waiting takes priority over explicit progress")
    func waitingWinsOverExplicitProgress() {
        let progress = TerminalProgress(kind: .set, percent: 40)
        let activity = TerminalActivity.resolve(
            progress: progress,
            agentStatus: .waiting,
            unreadCount: 2,
            completionPending: true
        )
        #expect(activity == .waiting)
    }

    @Test("finished takes priority over active progress and unread")
    func finishedWinsOverProgressAndUnread() {
        let progress = TerminalProgress(kind: .set, percent: 40)
        let activity = TerminalActivity.resolve(
            progress: progress,
            agentStatus: .working,
            unreadCount: 2,
            completionPending: true
        )
        #expect(activity == .finished)
    }

    @Test("agent working takes priority over unread")
    func agentWorkingWins() {
        let activity = TerminalActivity.resolve(
            progress: nil,
            agentStatus: .working,
            unreadCount: 2,
            completionPending: false
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

    @Test("finished takes priority over unread")
    func finishedWinsOverUnread() {
        let activity = TerminalActivity.resolve(
            progress: nil,
            agentStatus: .idle,
            unreadCount: 2,
            completionPending: true
        )
        #expect(activity == .finished)
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
