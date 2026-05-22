import Testing

@testable import Muxy

@Suite("NotificationSettings")
struct NotificationSettingsTests {
    @Test("playableSound returns default sound for missing value")
    func playableSoundDefaultsMissingValue() {
        #expect(NotificationSound.playableSound(for: nil) == .funk)
    }

    @Test("playableSound ignores none")
    func playableSoundIgnoresNone() {
        #expect(NotificationSound.playableSound(for: NotificationSound.none.rawValue) == nil)
    }

    @Test("playableSound ignores unknown values")
    func playableSoundIgnoresUnknownValues() {
        #expect(NotificationSound.playableSound(for: "Custom") == nil)
    }

    @Test("playableSound accepts supported values")
    func playableSoundAcceptsSupportedValues() {
        #expect(NotificationSound.playableSound(for: NotificationSound.ping.rawValue) == .ping)
    }
}
