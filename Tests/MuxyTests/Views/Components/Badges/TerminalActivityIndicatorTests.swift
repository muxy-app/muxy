import Testing

@testable import Muxy

@Suite("TerminalActivityIndicator")
@MainActor
struct TerminalActivityIndicatorTests {
    @Test("tooltips explain every activity state")
    func tooltipsExplainActivity() {
        let progress = TerminalProgress(kind: .indeterminate, percent: nil)

        #expect(TerminalActivityIndicator.tooltip(for: .working(progress)) == "Work is in progress.")
        #expect(TerminalActivityIndicator.tooltip(for: .waiting) == "An agent is waiting for your attention.")
        #expect(TerminalActivityIndicator.tooltip(for: .unread(1)) == "1 unread notification")
        #expect(TerminalActivityIndicator.tooltip(for: .unread(3)) == "3 unread notifications")
        #expect(TerminalActivityIndicator.tooltip(for: .finished) == "Work finished and is ready to review.")
    }
}
