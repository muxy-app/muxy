import SwiftUI

struct EditorSettingsView: View {
    @State private var settings = EditorSettings.shared
    @State private var monoFonts: [String] = []

    var body: some View {
        VStack(spacing: 0) {
            settingRow("Default Editor") {
                Picker("", selection: $settings.defaultEditor) {
                    ForEach(EditorSettings.DefaultEditor.allCases) { editor in
                        Text(editor.displayName).tag(editor)
                    }
                }
                .labelsHidden()
                .frame(width: 210, alignment: .trailing)
            }

            if settings.defaultEditor == .terminalCommand {
                settingRow("Editor Command") {
                    TextField("vim", text: $settings.externalEditorCommand)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: 12, design: .monospaced))
                        .frame(width: 210)
                }
            } else {
                Divider()
                    .padding(.vertical, 4)

                settingRow("Font Family") {
                    Picker("", selection: $settings.fontFamily) {
                        ForEach(monoFonts, id: \.self) { family in
                            Text(family)
                                .font(.custom(family, size: 12))
                                .tag(family)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 210, alignment: .trailing)
                }

                settingRow("Font Size") {
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
                            .font(.system(size: 12, design: .monospaced))
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

                settingRow("Tab Size") {
                    Picker("", selection: $settings.tabSize) {
                        Text("2").tag(2)
                        Text("4").tag(4)
                        Text("8").tag(8)
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                    .frame(width: 140, alignment: .trailing)
                }

                toggleRow("Word Wrap", isOn: $settings.wordWrap)
                toggleRow("Show Line Numbers", isOn: $settings.showLineNumbers)
                toggleRow("Syntax Highlighting", isOn: $settings.syntaxHighlighting)
                toggleRow("Bracket Matching", isOn: $settings.bracketMatching)
                toggleRow("Current Line Highlight", isOn: $settings.currentLineHighlight)
            }

            Spacer()

            HStack {
                Spacer()
                Button("Reset to Defaults") {
                    settings.resetToDefaults()
                }
                .font(.system(size: 11))
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 12)
        }
        .task {
            monoFonts = EditorSettings.availableMonospacedFonts
        }
    }

    private func settingRow(_ label: String, @ViewBuilder content: () -> some View) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 12))
            Spacer()
            content()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private func toggleRow(_ label: String, isOn: Binding<Bool>) -> some View {
        settingRow(label) {
            Toggle("", isOn: isOn)
                .toggleStyle(.switch)
                .controlSize(.small)
        }
    }
}
