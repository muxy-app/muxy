import Foundation
import Testing

@testable import Muxy

@Suite("TerminalOfflineTimeout")
struct TerminalOfflineTimeoutTests {
    @Test("maps each option to its duration in seconds")
    func mapsSeconds() {
        #expect(TerminalOfflineTimeout.tenSeconds.seconds == 10)
        #expect(TerminalOfflineTimeout.fiveMinutes.seconds == 300)
        #expect(TerminalOfflineTimeout.thirtyMinutes.seconds == 1800)
    }

    @Test("closest picks the nearest option to an arbitrary duration")
    func closestPicksNearest() {
        #expect(TerminalOfflineTimeout.closest(to: 9) == .tenSeconds)
        #expect(TerminalOfflineTimeout.closest(to: 55) == .oneMinute)
        #expect(TerminalOfflineTimeout.closest(to: 280) == .fiveMinutes)
        #expect(TerminalOfflineTimeout.closest(to: 100_000) == .thirtyMinutes)
    }

    @Test("default idle threshold maps to a real option")
    func defaultMapsToOption() {
        #expect(TerminalOfflineTimeout.closest(to: TerminalOfflinePreferences.defaultIdleThreshold) == .fiveMinutes)
    }
}
