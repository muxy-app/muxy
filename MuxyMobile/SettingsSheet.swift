import SwiftUI

struct SettingsSheet: View {
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    NavigationLink {
                        Text("Coming soon")
                            .foregroundStyle(.secondary)
                            .navigationTitle("Interface")
                    } label: {
                        Label("Interface", systemImage: "rectangle.on.rectangle")
                    }
                }

                Section("Connection") {
                    NavigationLink {
                        Text("Coming soon")
                            .foregroundStyle(.secondary)
                            .navigationTitle("Default Port")
                    } label: {
                        Label("Default Port", systemImage: "number")
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
                        Image(systemName: "xmark.circle.fill")
                            .symbolRenderingMode(.hierarchical)
                            .font(.title3)
                    }
                }
            }
        }
    }

    private var aboutView: some View {
        Form {
            Section {
                LabeledContent("Version", value: "0.1.0")
                LabeledContent("Build", value: "1")
            }
        }
        .navigationTitle("About")
    }
}
