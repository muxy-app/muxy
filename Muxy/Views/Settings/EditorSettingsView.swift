import SwiftUI

struct EditorSettingsView: View {
    @State private var settings = EditorSettings.shared
    @State private var monoFonts: [String] = []
    @State private var allowMarkdownRemoteImages = MarkdownPreviewPreferences.allowRemoteImages

    private var showsAppearanceSection: Bool { settings.defaultEditor == .builtIn }

    var body: some View {
        VStack(spacing: 0) {
            SettingsSection("Editor") {
                SettingsRow("Default Editor") {
                    Picker("", selection: $settings.defaultEditor) {
                        ForEach(EditorSettings.DefaultEditor.allCases) { editor in
                            Text(editor.displayName).tag(editor)
                        }
                    }
                    .labelsHidden()
                    .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
                }

                if settings.defaultEditor == .terminalCommand {
                    SettingsRow("Editor Command") {
                        TextField("vim", text: $settings.externalEditorCommand)
                            .textFieldStyle(.roundedBorder)
                            .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                            .frame(width: SettingsMetrics.controlWidth)
                    }
                }
            }

            SettingsSection(
                "Markdown Preview",
                footer: "Remote images are fetched over HTTPS only. Plain HTTP and other schemes are blocked.",
                showsDivider: showsAppearanceSection
            ) {
                SettingsToggleRow(label: "Allow Remote Images", isOn: $allowMarkdownRemoteImages)
                    .onChange(of: allowMarkdownRemoteImages) { _, newValue in
                        MarkdownPreviewPreferences.allowRemoteImages = newValue
                    }
            }

            if showsAppearanceSection {
                SettingsSection("Appearance", showsDivider: false) {
                    SettingsRow("Font Family") {
                        Picker("", selection: $settings.fontFamily) {
                            ForEach(monoFonts, id: \.self) { family in
                                Text(family)
                                    .font(.custom(family, size: 12))
                                    .tag(family)
                            }
                        }
                        .labelsHidden()
                        .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
                    }

                    SettingsRow("Font Size") {
                        HStack(spacing: 8) {
                            Button {
                                guard settings.fontSize > 8 else { return }
                                settings.fontSize -= 1
                            } label: {
                                Image(systemName: "minus")
                                    .font(.system(size: 10, weight: .medium))
                                    .frame(width: 20, height: 20)
                            }
                            .buttonStyle(.borderless)

                            Text("\(Int(settings.fontSize)) pt")
                                .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                                .frame(width: 44)

                            Button {
                                guard settings.fontSize < 36 else { return }
                                settings.fontSize += 1
                            } label: {
                                Image(systemName: "plus")
                                    .font(.system(size: 10, weight: .medium))
                                    .frame(width: 20, height: 20)
                            }
                            .buttonStyle(.borderless)
                        }
                    }
                }
            }

            Spacer(minLength: 0)

            HStack {
                Spacer()
                Button("Reset to Defaults") {
                    settings.resetToDefaults()
                }
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
            }
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.bottom, SettingsMetrics.verticalPadding)
        }
        .task {
            monoFonts = EditorSettings.availableMonospacedFonts
        }
    }
}
