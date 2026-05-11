import SwiftUI

struct EditorSettingsView: View {
    @State private var settings = EditorSettings.shared
    @State private var monoFonts: [String] = []
    @State private var markdownFonts: [String] = []
    @State private var allowMarkdownRemoteImages = MarkdownPreviewPreferences.allowRemoteImages
    @AppStorage(RichInputPreferences.floatingKey) private var richInputFloating = RichInputPreferences.defaultFloating
    @AppStorage(RichInputPreferences.positionKey) private var richInputPosition: RichInputPanelPosition = RichInputPreferences
        .defaultPosition

    private var showsAppearanceSection: Bool { settings.defaultEditor == .builtIn }

    var body: some View {
        VStack(spacing: 0) {
            SettingsContainer {
                editorSections
            }

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
            markdownFonts = EditorSettings.availableMarkdownPreviewFonts
        }
    }

    @ViewBuilder
    private var editorSections: some View {
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
            footer: "Remote images are fetched over HTTPS only. Plain HTTP and other schemes are blocked."
        ) {
            SettingsToggleRow(label: "Allow Remote Images", isOn: $allowMarkdownRemoteImages)
                .onChange(of: allowMarkdownRemoteImages) { _, newValue in
                    MarkdownPreviewPreferences.allowRemoteImages = newValue
                }

            SettingsRow("Font Family") {
                Picker("", selection: $settings.markdownPreviewFontFamily) {
                    ForEach(markdownFonts, id: \.self) { family in
                        if family == EditorSettings.systemFontFamilyToken {
                            Text(family).tag(family)
                        } else {
                            Text(family)
                                .font(.custom(family, size: 12))
                                .tag(family)
                        }
                    }
                }
                .labelsHidden()
                .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
            }

            SettingsRow("Zoom") {
                HStack(spacing: 8) {
                    Button {
                        settings.adjustMarkdownPreviewFontScale(by: -EditorSettings.markdownPreviewZoomStep)
                    } label: {
                        Image(systemName: "minus")
                            .font(.system(size: 10, weight: .medium))
                            .frame(width: 20, height: 20)
                    }
                    .buttonStyle(.borderless)

                    Text("\(Int((settings.markdownPreviewFontScale * 100).rounded()))%")
                        .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                        .frame(width: 44)

                    Button {
                        settings.adjustMarkdownPreviewFontScale(by: EditorSettings.markdownPreviewZoomStep)
                    } label: {
                        Image(systemName: "plus")
                            .font(.system(size: 10, weight: .medium))
                            .frame(width: 20, height: 20)
                    }
                    .buttonStyle(.borderless)
                }
            }
        }

        SettingsSection(
            "Rich Input",
            footer: "Inline File Path keeps multiple images perfectly ordered with text and Enter. "
                + "Use Clipboard Paste if your TUI doesn't recognize image paths.",
            showsDivider: showsAppearanceSection
        ) {
            SettingsRow("Image Submission") {
                Picker("", selection: $settings.richInputImageStrategy) {
                    ForEach(RichInputImageStrategy.allCases) { strategy in
                        Text(strategy.displayName).tag(strategy)
                    }
                }
                .labelsHidden()
                .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
            }

            SettingsRow("Position") {
                Picker("", selection: $richInputPosition) {
                    ForEach(RichInputPanelPosition.allCases) { position in
                        Text(position.displayName).tag(position)
                    }
                }
                .labelsHidden()
                .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
            }

            SettingsToggleRow(label: "Floating Panel", isOn: $richInputFloating)
        }

        if showsAppearanceSection {
            SettingsSection("Appearance", showsDivider: false) {
                SettingsToggleRow(label: "Highlight Current Line", isOn: $settings.highlightCurrentLine)

                SettingsToggleRow(label: "Show Line Numbers", isOn: $settings.showLineNumbers)

                SettingsToggleRow(label: "Wrap Lines", isOn: $settings.lineWrapping)

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

                SettingsRow("Line Height") {
                    HStack(spacing: 8) {
                        Button {
                            settings.lineHeightMultiplier = max(
                                EditorSettings.minLineHeightMultiplier,
                                settings.lineHeightMultiplier - EditorSettings.lineHeightMultiplierStep
                            )
                        } label: {
                            Image(systemName: "minus")
                                .font(.system(size: 10, weight: .medium))
                                .frame(width: 20, height: 20)
                        }
                        .buttonStyle(.borderless)
                        .disabled(settings.lineHeightMultiplier <= EditorSettings.minLineHeightMultiplier + 0.001)

                        Text(String(format: "%.1f×", settings.lineHeightMultiplier))
                            .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                            .frame(width: 44)

                        Button {
                            settings.lineHeightMultiplier = min(
                                EditorSettings.maxLineHeightMultiplier,
                                settings.lineHeightMultiplier + EditorSettings.lineHeightMultiplierStep
                            )
                        } label: {
                            Image(systemName: "plus")
                                .font(.system(size: 10, weight: .medium))
                                .frame(width: 20, height: 20)
                        }
                        .buttonStyle(.borderless)
                        .disabled(settings.lineHeightMultiplier >= EditorSettings.maxLineHeightMultiplier - 0.001)
                    }
                }
            }
        }
    }
}
