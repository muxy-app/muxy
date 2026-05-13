import Foundation
import Testing

@testable import Muxy

@Suite("VoiceRecorder helpers")
struct VoiceRecorderHelperTests {
    @Test("normalize clamps below floor to zero")
    func normalizeBelowFloor() {
        #expect(VoiceRecorder.normalize(power: -120) == 0)
    }

    @Test("normalize clamps at zero dB to one")
    func normalizeAtCeiling() {
        #expect(VoiceRecorder.normalize(power: 0) == 1)
    }

    @Test("normalize maps mid-range power smoothly")
    func normalizeMidrange() {
        let value = VoiceRecorder.normalize(power: -25)
        #expect(value > 0.45 && value < 0.55)
    }

    @Test("normalize handles non-finite values")
    func normalizeNonFinite() {
        #expect(VoiceRecorder.normalize(power: .nan) == 0)
        #expect(VoiceRecorder.normalize(power: -.infinity) == 0)
    }
}

@Suite("VoiceRecordingPanel formatting")
struct VoiceRecordingPanelTests {
    @Test("Zero formats as 00:00")
    func formatsZero() {
        #expect(VoiceRecordingPanel.formatElapsed(0) == "00:00")
    }

    @Test("Sub-minute formats with leading zero")
    func formatsSeconds() {
        #expect(VoiceRecordingPanel.formatElapsed(7) == "00:07")
    }

    @Test("Multi-minute formats correctly")
    func formatsMinutes() {
        #expect(VoiceRecordingPanel.formatElapsed(125) == "02:05")
    }

    @Test("Negative input clamps to zero")
    func clampsNegative() {
        #expect(VoiceRecordingPanel.formatElapsed(-5) == "00:00")
    }
}
