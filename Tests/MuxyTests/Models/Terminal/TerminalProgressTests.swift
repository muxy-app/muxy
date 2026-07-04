import Testing

@testable import Muxy

@Suite("TerminalProgress")
struct TerminalProgressTests {
    @Test("tab indicator prefers explicit terminal progress")
    func tabIndicatorPrefersTerminalProgress() {
        let progress = TerminalProgress(kind: .set, percent: 40)

        #expect(TerminalProgress.tabIndicator(progress: progress, agentStatus: .working) == progress)
    }

    @Test("tab indicator maps working agent status to indeterminate progress")
    func tabIndicatorMapsWorkingAgentStatus() {
        #expect(TerminalProgress.tabIndicator(progress: nil, agentStatus: .working) == TerminalProgress(
            kind: .indeterminate,
            percent: nil
        ))
    }

    @Test("tab indicator ignores non-working agent states")
    func tabIndicatorIgnoresNonWorkingAgentStates() {
        #expect(TerminalProgress.tabIndicator(progress: nil, agentStatus: .waiting) == nil)
        #expect(TerminalProgress.tabIndicator(progress: nil, agentStatus: .idle) == nil)
        #expect(TerminalProgress.tabIndicator(progress: nil, agentStatus: nil) == nil)
    }
}
