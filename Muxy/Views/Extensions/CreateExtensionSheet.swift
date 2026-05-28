import AppKit
import SwiftUI

struct CreateExtensionSheet: View {
    let store: ExtensionStore
    let onFinish: () -> Void

    @State private var name = ""
    @State private var version = "0.1.0"
    @State private var description = ""
    @State private var errorMessage: String?
    @State private var inProgress = false

    private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var trimmedVersion: String { version.trimmingCharacters(in: .whitespacesAndNewlines) }

    private var canCreate: Bool {
        !trimmedName.isEmpty && !trimmedVersion.isEmpty && !inProgress
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("New Extension")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            field(
                label: "Name",
                hint: "letters, digits, dash, underscore, dot",
                placeholder: "my-extension",
                value: $name,
                monospaced: true
            )

            field(
                label: "Version",
                hint: nil,
                placeholder: "0.1.0",
                value: $version,
                monospaced: true
            )

            VStack(alignment: .leading, spacing: 4) {
                Text("Description")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)
                TextField("Optional summary", text: $description, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(2 ... 4)
            }

            if let errorMessage {
                Text(errorMessage)
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack {
                Spacer()
                Button("Cancel") { onFinish() }
                    .keyboardShortcut(.cancelAction)
                Button("Create") { create() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canCreate)
            }
        }
        .padding(20)
        .frame(width: 440)
        .background(MuxyTheme.bg)
    }

    private func field(
        label: String,
        hint: String?,
        placeholder: String,
        value: Binding<String>,
        monospaced: Bool
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Text(label)
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)
                if let hint {
                    Text(hint)
                        .font(.system(size: 10))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
            }
            TextField(placeholder, text: value)
                .font(.system(size: 12, design: monospaced ? .monospaced : .default))
                .textFieldStyle(.roundedBorder)
                .disableAutocorrection(true)
        }
    }

    private func create() {
        errorMessage = nil
        inProgress = true
        let request = ExtensionScaffoldRequest(name: name, version: version, description: description)
        do {
            let directory = try ExtensionScaffoldService.create(request, in: store.rootDirectory)
            store.reload()
            NotificationCenter.default.post(
                name: .openExtensionDirectoryAsProject,
                object: nil,
                userInfo: [OpenExtensionDirectoryUserInfoKey.path: directory.path]
            )
            onFinish()
            NSApp.keyWindow?.close()
        } catch {
            inProgress = false
            errorMessage = error.localizedDescription
        }
    }
}
