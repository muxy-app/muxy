import Foundation

@MainActor
@Observable
final class ComposerVoiceState {
    var errorMessage: String?
    private(set) var isStarting = false
    let recorder: any VoiceRecording

    @ObservationIgnored private var startTask: Task<Void, Never>?
    @ObservationIgnored private let permissionRequest: @Sendable () async -> Bool
    @ObservationIgnored private let localeResolver: @MainActor (String) -> Locale?

    init(
        recorder: any VoiceRecording = VoiceRecorder(),
        permissionRequest: @escaping @Sendable () async -> Bool = VoiceRecorder.requestPermissions,
        localeResolver: @escaping @MainActor (String) -> Locale? = VoiceRecordingSupport.resolveLocale
    ) {
        self.recorder = recorder
        self.permissionRequest = permissionRequest
        self.localeResolver = localeResolver
        recorder.onFailure = { [weak self] message in
            self?.isStarting = false
            self?.errorMessage = message
        }
    }

    var isBusy: Bool {
        isStarting || recorder.isRecording
    }

    var canSubmit: Bool {
        !isBusy
    }

    var showsFeedback: Bool {
        isBusy || errorMessage != nil
    }

    func start(languageIdentifier: String) {
        guard !isStarting, !recorder.isRecording else { return }
        errorMessage = nil
        guard let locale = localeResolver(languageIdentifier) else {
            errorMessage = VoiceRecordingSupport.unavailableLanguageMessage
            return
        }
        isStarting = true
        startTask?.cancel()
        startTask = Task { @MainActor [weak self] in
            guard let self else { return }
            let granted = await permissionRequest()
            guard !Task.isCancelled else { return }
            guard granted else {
                isStarting = false
                errorMessage = VoiceRecordingSupport.permissionDeniedMessage
                startTask = nil
                return
            }
            do {
                try recorder.start(locale: locale)
            } catch {
                errorMessage = VoiceRecordingSupport.readableMessage(for: error)
            }
            isStarting = false
            startTask = nil
        }
    }

    func finish() -> String? {
        guard recorder.isRecording else { return nil }
        let transcript = recorder.finish().trimmingCharacters(in: .whitespacesAndNewlines)
        guard !transcript.isEmpty else {
            errorMessage = "No speech detected. Try again or cancel dictation."
            return nil
        }
        errorMessage = nil
        return transcript
    }

    func dismissError() {
        errorMessage = nil
    }

    func cancel() {
        startTask?.cancel()
        startTask = nil
        isStarting = false
        recorder.cancel()
        errorMessage = nil
    }

    func togglePause() {
        guard recorder.isRecording else { return }
        if recorder.isPaused {
            recorder.resume()
        } else {
            recorder.pause()
        }
    }
}
