import Foundation
import Testing

@testable import Muxy

@Suite("Composer voice state")
@MainActor
struct ComposerVoiceStateTests {
    @Test func startsAfterPermissionIsGranted() async {
        let recorder = VoiceRecorderStub()
        let state = makeState(recorder: recorder, permissionRequest: { true })

        state.start(languageIdentifier: "en-US")
        await waitUntil { !state.isStarting }

        #expect(recorder.startedLocales == [Locale(identifier: "en-US")])
        #expect(state.isBusy)
        #expect(!state.canSubmit)
    }

    @Test func deniedPermissionLeavesTextSubmissionAvailable() async {
        let recorder = VoiceRecorderStub()
        let state = makeState(recorder: recorder, permissionRequest: { false })

        state.start(languageIdentifier: "en-US")
        await waitUntil { !state.isStarting }

        #expect(recorder.startedLocales.isEmpty)
        #expect(state.errorMessage == VoiceRecordingSupport.permissionDeniedMessage)
        #expect(!state.isBusy)
        #expect(state.canSubmit)
        #expect(state.showsFeedback)
    }

    @Test func cancellationPreventsLatePermissionCompletionFromStarting() async {
        let recorder = VoiceRecorderStub()
        let permission = PermissionGate()
        let state = makeState(
            recorder: recorder,
            permissionRequest: { await permission.request() }
        )

        state.start(languageIdentifier: "en-US")
        state.cancel()
        await permission.resolve(true)
        await waitUntil { !state.isStarting }

        #expect(recorder.startedLocales.isEmpty)
        #expect(!state.isBusy)
        #expect(state.canSubmit)
    }

    @Test func recorderStartFailureCanBeDismissed() async {
        let recorder = VoiceRecorderStub()
        recorder.startError = VoiceRecorderError.recognizerUnavailable
        let state = makeState(recorder: recorder, permissionRequest: { true })

        state.start(languageIdentifier: "en-US")
        await waitUntil { !state.isStarting }

        #expect(state.errorMessage == "Speech recognition is unavailable on this device.")
        #expect(state.canSubmit)

        state.dismissError()

        #expect(state.errorMessage == nil)
        #expect(!state.showsFeedback)
    }

    @Test func finishReturnsTrimmedTranscript() {
        let recorder = VoiceRecorderStub()
        recorder.isRecording = true
        recorder.transcript = "  dictated text \n"
        let state = makeState(recorder: recorder)

        let transcript = state.finish()

        #expect(transcript == "dictated text")
        #expect(state.errorMessage == nil)
    }

    @Test func emptyFinishReportsErrorWithoutBlockingSubmission() {
        let recorder = VoiceRecorderStub()
        recorder.isRecording = true
        recorder.transcript = " \n"
        let state = makeState(recorder: recorder)

        let transcript = state.finish()

        #expect(transcript == nil)
        #expect(state.errorMessage == "No speech detected. Try again or cancel dictation.")
        #expect(state.canSubmit)
    }

    private func makeState(
        recorder: VoiceRecorderStub,
        permissionRequest: @escaping @Sendable () async -> Bool = { true }
    ) -> ComposerVoiceState {
        ComposerVoiceState(
            recorder: recorder,
            permissionRequest: permissionRequest,
            localeResolver: { Locale(identifier: $0) }
        )
    }

    private func waitUntil(_ condition: () -> Bool) async {
        for _ in 0 ..< 100 where !condition() {
            await Task.yield()
        }
    }
}

@MainActor
private final class VoiceRecorderStub: VoiceRecording {
    var isRecording = false
    var isPaused = false
    var elapsed: TimeInterval = 0
    var level: Float = 0
    var transcript = ""
    var onFailure: (@MainActor (String) -> Void)?
    var startError: Error?
    private(set) var startedLocales: [Locale] = []

    func start(locale: Locale) throws {
        if let startError {
            throw startError
        }
        startedLocales.append(locale)
        isRecording = true
    }

    func pause() {
        isPaused = true
    }

    func resume() {
        isPaused = false
    }

    func finish() -> String {
        isRecording = false
        return transcript
    }

    func cancel() {
        isRecording = false
        isPaused = false
    }
}

private actor PermissionGate {
    private var result: Bool?
    private var continuation: CheckedContinuation<Bool, Never>?

    func request() async -> Bool {
        if let result {
            return result
        }
        return await withCheckedContinuation { continuation in
            self.continuation = continuation
        }
    }

    func resolve(_ result: Bool) {
        self.result = result
        continuation?.resume(returning: result)
        continuation = nil
    }
}
