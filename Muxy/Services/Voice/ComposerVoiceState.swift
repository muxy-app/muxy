import Foundation

@MainActor
@Observable
final class ComposerVoiceState {
    var errorMessage: String?
    private(set) var isStarting = false
    let recorder: VoiceRecorder

    @ObservationIgnored private var startTask: Task<Void, Never>?

    init(recorder: VoiceRecorder = VoiceRecorder()) {
        self.recorder = recorder
        recorder.onFailure = { [weak self] message in
            self?.isStarting = false
            self?.errorMessage = message
        }
    }

    var isActive: Bool {
        isStarting || recorder.isRecording || errorMessage != nil
    }

    func start(languageIdentifier: String) {
        guard !isStarting, !recorder.isRecording else { return }
        errorMessage = nil
        guard let locale = VoiceRecordingSupport.resolveLocale(from: languageIdentifier) else {
            errorMessage = VoiceRecordingSupport.unavailableLanguageMessage
            return
        }
        isStarting = true
        startTask?.cancel()
        startTask = Task { @MainActor [weak self] in
            guard let self else { return }
            let granted = await VoiceRecorder.requestPermissions()
            guard !Task.isCancelled else { return }
            guard granted else {
                isStarting = false
                errorMessage = VoiceRecordingSupport.permissionDeniedMessage
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
