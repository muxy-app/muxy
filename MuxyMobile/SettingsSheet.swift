import SwiftUI

struct SettingsSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(ConnectionManager.self) private var connection
    @State private var useNerdFont = TerminalFont.useNerdFont
    @State private var fontSize = TerminalFont.fontSize
    @State private var demoMode = false

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
                        .font(.system(size: fontSize, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 4)
                }

                Section {
                    Toggle("Demo Mode", isOn: $demoMode)
                        .onChange(of: demoMode) { _, newValue in
                            guard newValue != connection.isDemoMode else { return }
                            connection.setDemoMode(newValue)
                        }
                } footer: {
                    Text("Loads sample data so you can try the app without a Mac. Switching it off restores your real devices.")
                }

                Section {
                    NavigationLink {
                        aboutView
                    } label: {
                        Label("About", systemImage: "info.circle")
                    }
                }
            }
            .onAppear { demoMode = connection.isDemoMode }
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
