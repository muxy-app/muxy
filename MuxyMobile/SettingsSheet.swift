import SwiftUI

struct SettingsSheet: View {
    @Environment(\.dismiss) private var dismiss
    @State private var useNerdFont = TerminalFont.useNerdFont
    @State private var fontSize = TerminalFont.fontSize
    @State private var cursorStyle = TerminalCursorStyle.current

    var body: some View {
        NavigationStack {
            Form {
                Section("Terminal") {
                    Toggle("Use NerdFont", isOn: $useNerdFont)
                        .onChange(of: useNerdFont) { _, newValue in
                            TerminalFont.useNerdFont = newValue
                        }

                    HStack {
                        Text("Font Size")
                        Spacer()
                        Text("\(Int(fontSize))")
                            .foregroundStyle(.secondary)
                        Stepper("", value: $fontSize, in: 8 ... 24, step: 1)
                            .labelsHidden()
                            .onChange(of: fontSize) { _, newValue in
                                TerminalFont.fontSize = newValue
                            }
                    }

                    Text("The quick brown fox")
                        .font(TerminalFont.current)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 4)

                    Picker("Cursor", selection: $cursorStyle) {
                        ForEach(TerminalCursorStyle.allCases) { style in
                            Text(style.displayName).tag(style)
                        }
                    }
                    .onChange(of: cursorStyle) { _, newValue in
                        TerminalCursorStyle.current = newValue
                    }
                }

                Section {
                    NavigationLink {
                        aboutView
                    } label: {
                        Label("About", systemImage: "info.circle")
                    }
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        dismiss()
                    } label: {
                        Image(systemName: "xmark")
                    }
                }
            }
        }
    }

    private var aboutView: some View {
        Form {
            Section {
                LabeledContent("Version", value: appVersion)
                LabeledContent("Build", value: appBuild)
            }
        }
        .navigationTitle("About")
    }

    private var appVersion: String {
        (Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String) ?? "-"
    }

    private var appBuild: String {
        (Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String) ?? "-"
    }
}
