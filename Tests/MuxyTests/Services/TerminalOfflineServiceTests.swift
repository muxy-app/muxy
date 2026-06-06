import Foundation
import Testing

@testable import Muxy

@Suite("TerminalOfflineService")
struct TerminalOfflineServiceTests {
    @Test("scan interval follows the idle threshold, clamped to 5...30 seconds")
    func scanIntervalFollowsThreshold() {
        #expect(TerminalOfflineService.scanInterval(for: 10) == 10)
        #expect(TerminalOfflineService.scanInterval(for: 30) == 30)
    }

    @Test("scan interval is clamped for very small and very large thresholds")
    func scanIntervalClamped() {
        #expect(TerminalOfflineService.scanInterval(for: 3) == 5)
        #expect(TerminalOfflineService.scanInterval(for: 300) == 30)
        #expect(TerminalOfflineService.scanInterval(for: 1800) == 30)
    }
}
