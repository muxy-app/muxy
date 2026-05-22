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

    @Test("average power handles float samples")
    func averagePowerHandlesFloatSamples() {
        let samples: [Float] = [0.5, -0.5, 0.5, -0.5]
        let power = samples.withUnsafeBufferPointer { VoiceRecorder.averagePower(in: $0) }

        #expect(power > -7 && power < -5)
    }

    @Test("Transcript merge keeps earlier text when recognition restarts")
    func transcriptMergeKeepsEarlierTextWhenRecognitionRestarts() {
        let first = VoiceRecorder.mergeTranscript(committed: "", partial: "", incoming: "Open the file")
        let pause = VoiceRecorder.mergeTranscript(
            committed: first.committed,
            partial: first.partial,
            incoming: ""
        )
        let second = VoiceRecorder.mergeTranscript(
            committed: pause.committed,
            partial: pause.partial,
            incoming: "and add tests"
        )

        #expect(second.transcript == "Open the file and add tests")
    }

    @Test("Transcript merge appends a new phrase after partial reset")
    func transcriptMergeAppendsNewPhraseAfterPartialReset() {
        let first = VoiceRecorder.mergeTranscript(committed: "", partial: "", incoming: "Open the file")
        let second = VoiceRecorder.mergeTranscript(
            committed: first.committed,
            partial: first.partial,
            incoming: "and add tests"
        )

        #expect(second.transcript == "Open the file and add tests")
    }

    @Test("Transcript merge appends reset phrase with same first word")
    func transcriptMergeAppendsResetPhraseWithSameFirstWord() {
        let first = VoiceRecorder.mergeTranscript(committed: "", partial: "", incoming: "Open the file")
        let second = VoiceRecorder.mergeTranscript(
            committed: first.committed,
            partial: first.partial,
            incoming: "Open settings"
        )

        #expect(second.transcript == "Open the file Open settings")
    }

    @Test("Transcript merge replaces early corrected partials")
    func transcriptMergeReplacesEarlyCorrectedPartials() {
        let first = VoiceRecorder.mergeTranscript(committed: "", partial: "", incoming: "The more")
        let second = VoiceRecorder.mergeTranscript(
            committed: first.committed,
            partial: first.partial,
            incoming: "The more I continue"
        )
        let third = VoiceRecorder.mergeTranscript(
            committed: second.committed,
            partial: second.partial,
            incoming: "The more I continue the stable sounds like"
        )

        #expect(third.transcript == "The more I continue the stable sounds like")
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
