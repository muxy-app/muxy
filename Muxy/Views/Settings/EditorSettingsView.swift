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
}
