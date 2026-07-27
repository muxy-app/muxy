import Foundation

enum VoiceRecordingSupport {
    static let unavailableLanguageMessage =
        "No on-device speech language is installed. Add one in System Settings → Keyboard → Dictation."
    static let permissionDeniedMessage =
        "Microphone or speech recognition is denied. Enable both in System Settings."

    @MainActor
    static func resolveLocale(from identifier: String) -> Locale? {
        if !identifier.isEmpty, let locale = SpeechLanguageCatalog.locale(for: identifier) {
            return locale
        }
        guard let fallback = SpeechLanguageCatalog.defaultIdentifier() else { return nil }
        return Locale(identifier: fallback)
    }

    static func readableMessage(for error: Error) -> String {
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
