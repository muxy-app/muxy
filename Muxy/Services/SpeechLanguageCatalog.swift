import Foundation
import Speech

struct SpeechLanguage: Identifiable, Hashable {
    let identifier: String
    let displayName: String

    var id: String { identifier }
}

@MainActor
enum SpeechLanguageCatalog {
    private static var cachedLanguages: [SpeechLanguage]?

    static func onDeviceLanguages() -> [SpeechLanguage] {
        if let cachedLanguages { return cachedLanguages }
        let computed = computeOnDeviceLanguages()
        cachedLanguages = computed
        return computed
    }

    static func defaultIdentifier() -> String? {
        let supported = onDeviceLanguages()
        let current = Locale.current.identifier
        if supported.contains(where: { $0.identifier == current }) { return current }
        let language = Locale.current.language.languageCode?.identifier
        if let language, let match = supported.first(where: { $0.identifier.hasPrefix(language) }) {
            return match.identifier
        }
        return supported.first?.identifier
    }

    static func locale(for identifier: String) -> Locale? {
        guard !identifier.isEmpty else { return nil }
        return Locale(identifier: identifier)
    }

    private static func computeOnDeviceLanguages() -> [SpeechLanguage] {
        let displayLocale = Locale.current
        var seen = Set<String>()
        let languages = SFSpeechRecognizer.supportedLocales()
            .compactMap { locale -> SpeechLanguage? in
                guard let recognizer = SFSpeechRecognizer(locale: locale),
                      recognizer.supportsOnDeviceRecognition
                else { return nil }
                let identifier = locale.identifier
                guard seen.insert(identifier).inserted else { return nil }
                let name = displayLocale.localizedString(forIdentifier: identifier) ?? identifier
                return SpeechLanguage(identifier: identifier, displayName: name)
            }
        return languages.sorted { $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending }
    }
}
