import SwiftUI

struct RichInputSettingsView: View {
    @State private var settings = EditorSettings.shared
    @State private var monoFonts: [String] = []
    @AppStorage(RichInputPreferences.presentationModeKey)
    private var presentationMode = RichInputPreferences.defaultPresentationMode

    var body: some View {
        VStack(spacing: 0) {
            SettingsContainer {
                richInputSection
            }

            HStack {
                Spacer()
                Button(L10n.string("Reset to Defaults")) {
                    settings.resetToDefaults()
                    presentationMode = RichInputPreferences.defaultPresentationMode
                }
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .buttonStyle(.borderless)
                .foregroundStyle(SettingsStyle.mutedForeground)
            }
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.bottom, SettingsMetrics.verticalPadding)
        }
        .task {
            monoFonts = EditorSettings.availableMonospacedFonts
        }
    }

    private var richInputSection: some View {
        SettingsSection(
            "Composer",
            footer: """
            Inline File Path keeps multiple images perfectly ordered with text and Enter. Use Clipboard Paste if your \
            TUI doesn't recognize image paths. SSH panes always upload the image and inline its remote path, because a \
            Mac file path does not resolve on the remote device.
            """,
            showsDivider: false
        ) {
            SettingsRow("Presentation") {
                Picker("", selection: $presentationMode) {
                    ForEach(RichInputPresentationMode.allCases) { mode in
                        Text(L10n.resource(key: mode.displayName)).tag(mode)
                    }
                }
                .labelsHidden()
                .pickerStyle(.segmented)
                .settingsControl(.intrinsic)
            }

            SettingsRow("Image Submission") {
                Picker("", selection: $settings.richInputImageStrategy) {
                    ForEach(RichInputImageStrategy.allCases) { strategy in
                        Text(L10n.resource(key: strategy.displayName)).tag(strategy)
                    }
                }
                .labelsHidden()
                .settingsControl()
            }

            SettingsRow("Font Family") {
                Picker("", selection: $settings.richInputFontFamily) {
                    ForEach(monoFonts, id: \.self) { family in
                        Text(family)
                            .font(.custom(family, size: 12))
                            .tag(family)
                    }
                }
                .labelsHidden()
                .settingsControl()
            }

            SettingsRow("Line Height") {
                HStack(spacing: 8) {
                    Button {
                        settings.richInputLineHeightMultiplier = max(
                            EditorSettings.minLineHeightMultiplier,
                            settings.richInputLineHeightMultiplier - EditorSettings.lineHeightMultiplierStep
                        )
                    } label: {
                        Image(systemName: "minus")
                            .font(.system(size: 10, weight: .medium))
                            .frame(width: 20, height: 20)
                    }
                    .buttonStyle(.borderless)
                    .disabled(
                        settings.richInputLineHeightMultiplier
                            <= EditorSettings.minLineHeightMultiplier + 0.001
                    )

                    Text(String(format: "%.1f×", settings.richInputLineHeightMultiplier))
                        .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                        .frame(width: 44)

                    Button {
                        settings.richInputLineHeightMultiplier = min(
                            EditorSettings.maxLineHeightMultiplier,
                            settings.richInputLineHeightMultiplier + EditorSettings.lineHeightMultiplierStep
                        )
                    } label: {
                        Image(systemName: "plus")
                            .font(.system(size: 10, weight: .medium))
                            .frame(width: 20, height: 20)
                    }
                    .buttonStyle(.borderless)
                    .disabled(
                        settings.richInputLineHeightMultiplier
                            >= EditorSettings.maxLineHeightMultiplier - 0.001
                    )
                }
            }
        }
    }
}
