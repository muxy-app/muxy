import SwiftUI

struct RecordingSettingsView: View {
    @AppStorage(RecordingPreferences.autoSendKey) private var autoSend = RecordingPreferences.defaultAutoSend
    @AppStorage(RecordingPreferences.languageKey) private var languageIdentifier = RecordingPreferences.defaultLanguage

    @State private var languages: [SpeechLanguage] = []

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Voice Recording",
                footer: "Press the Voice Recording shortcut to dictate. "
                    + "Muxy transcribes your speech on-device and inserts it wherever your cursor was before "
                    + "you opened the recorder. If that target is gone, the transcript lands on your clipboard."
            ) {
                SettingsToggleRow(
                    label: "Press Return after inserting",
                    isOn: $autoSend
                )
            }

            SettingsSection(
                "Language",
                footer: languageFooter,
                showsDivider: false
            ) {
                languagePicker
            }
        }
        .onAppear(perform: loadLanguages)
    }

    private var languageFooter: String {
        if languages.isEmpty {
            return "No on-device speech models are installed. "
                + "Add a dictation language in System Settings → Keyboard → Dictation, then return here."
        }
        return "Only languages with an on-device model are listed. Transcription never leaves your Mac."
    }

    @ViewBuilder
    private var languagePicker: some View {
        if languages.isEmpty {
            SettingsRow("Language") {
                Text("None available")
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .foregroundStyle(.secondary)
            }
        } else {
            SettingsRow("Language") {
                Picker("", selection: resolvedSelection) {
                    ForEach(languages) { language in
                        Text(language.displayName).tag(language.identifier)
                    }
                }
                .labelsHidden()
                .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
            }
        }
    }

    private var resolvedSelection: Binding<String> {
        Binding(
            get: {
                if languages.contains(where: { $0.identifier == languageIdentifier }) {
                    return languageIdentifier
                }
                return SpeechLanguageCatalog.defaultIdentifier() ?? languages.first?.identifier ?? ""
            },
            set: { languageIdentifier = $0 }
        )
    }

    private func loadLanguages() {
        languages = SpeechLanguageCatalog.onDeviceLanguages()
        if languageIdentifier.isEmpty, let defaultIdentifier = SpeechLanguageCatalog.defaultIdentifier() {
            languageIdentifier = defaultIdentifier
        }
    }
}
