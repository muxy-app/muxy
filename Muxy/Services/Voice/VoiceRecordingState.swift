import AppKit
import Foundation

@MainActor
@Observable
final class VoiceRecordingState {
    static let shared = VoiceRecordingState()

    var isPanelVisible = false
    var errorMessage: String?
    let recorder = VoiceRecorder()

    @ObservationIgnored private var capturedResponder: NSResponder?
    @ObservationIgnored private var startTask: Task<Void, Never>?

    private init() {
        recorder.onFailure = { [weak self] message in
            self?.errorMessage = message
        }
    }

    func present(languageIdentifier: String) {
        guard !isPanelVisible else { return }
        errorMessage = nil
        capturedResponder = NSApp.keyWindow?.firstResponder
        isPanelVisible = true
        let resolvedLocale = VoiceRecordingSupport.resolveLocale(from: languageIdentifier)
        guard let resolvedLocale else {
            errorMessage = VoiceRecordingSupport.unavailableLanguageMessage
            return
        }
        startTask = Task { @MainActor [weak self] in
            guard let self else { return }
            let granted = await VoiceRecorder.requestPermissions()
            guard !Task.isCancelled, isPanelVisible else { return }
            guard granted else {
                errorMessage = VoiceRecordingSupport.permissionDeniedMessage
                startTask = nil
                return
            }
            do {
                try recorder.start(locale: resolvedLocale)
            } catch {
                errorMessage = VoiceRecordingSupport.readableMessage(for: error)
            }
            startTask = nil
        }
    }

    func finish(autoSend: Bool) {
        guard isPanelVisible else { return }
        let final = recorder.finish()
        guard !final.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            errorMessage = "No speech detected. Try again or press Esc to close."
            return
        }
        let responder = capturedResponder
        cleanup()
        TranscriptInserter.insert(text: final, into: responder, appendReturn: autoSend)
    }

    func cancel() {
        guard isPanelVisible else { return }
        startTask?.cancel()
        startTask = nil
        recorder.cancel()
        cleanup()
    }

    func togglePause() {
        guard recorder.isRecording else { return }
        if recorder.isPaused {
            recorder.resume()
        } else {
            recorder.pause()
        }
    }

    private func cleanup() {
        isPanelVisible = false
        errorMessage = nil
        capturedResponder = nil
    }
}
