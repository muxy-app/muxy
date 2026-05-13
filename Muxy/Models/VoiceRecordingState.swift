import AppKit
import AVFoundation
import Foundation
import Speech

@MainActor
@Observable
final class VoiceRecordingState {
    static let shared = VoiceRecordingState()

    var isPanelVisible = false
    var errorMessage: String?
    let recorder = VoiceRecorder()

    @ObservationIgnored private var capturedResponder: NSResponder?

    private init() {}

    func present(languageIdentifier: String) {
        guard !isPanelVisible else { return }
        errorMessage = nil
        capturedResponder = NSApp.keyWindow?.firstResponder
        isPanelVisible = true
        let resolvedLocale = Self.resolveLocale(from: languageIdentifier)
        guard let resolvedLocale else {
            errorMessage = "No on-device speech language is installed. Add one in System Settings → Keyboard → Dictation."
            return
        }
        Task { @MainActor in
            let granted = await VoiceRecorder.requestPermissions()
            guard granted else {
                errorMessage = "Microphone or speech recognition is denied. Enable both in System Settings."
                return
            }
            do {
                try recorder.start(locale: resolvedLocale)
            } catch {
                errorMessage = readableMessage(for: error)
            }
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

    private static func resolveLocale(from identifier: String) -> Locale? {
        if !identifier.isEmpty, let locale = SpeechLanguageCatalog.locale(for: identifier) {
            return locale
        }
        if let fallback = SpeechLanguageCatalog.defaultIdentifier() {
            return Locale(identifier: fallback)
        }
        return nil
    }

    private func cleanup() {
        isPanelVisible = false
        errorMessage = nil
        capturedResponder = nil
    }

    private func readableMessage(for error: Error) -> String {
        switch error {
        case VoiceRecorderError.recognizerUnavailable:
            "Speech recognition is unavailable on this device."
        case let VoiceRecorderError.engineFailure(message):
            "Recording failed: \(message)"
        default:
            error.localizedDescription
        }
    }
}
