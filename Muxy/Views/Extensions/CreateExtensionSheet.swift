import AppKit
import SwiftUI

struct CreateExtensionSheet: View {
    let store: ExtensionStore
    let onFinish: () -> Void

    @State private var name = ""
    @State private var version = "0.1.0"
    @State private var description = ""
    @State private var location: URL
    @State private var errorMessage: String?

    init(store: ExtensionStore, onFinish: @escaping () -> Void) {
        self.store = store
        self.onFinish = onFinish
        _location = State(initialValue: store.rootDirectory)
    }

    private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var trimmedVersion: String { version.trimmingCharacters(in: .whitespacesAndNewlines) }

    private var isDefaultLocation: Bool {
        location.standardizedFileURL.path == store.rootDirectory.standardizedFileURL.path
    }

    private var canCreate: Bool {
        !trimmedName.isEmpty && !trimmedVersion.isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("New Extension")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            guideBlock

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

            VStack(alignment: .leading, spacing: 4) {
                Text("Location")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)
                HStack(spacing: 8) {
                    Text(location.path)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer(minLength: 8)
                    Button("Choose…") { chooseLocation() }
                        .font(.system(size: 11))
                }
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

    private var guideBlock: some View {
        VStack(alignment: .leading, spacing: 6) {
            guideRow(
                symbol: "folder",
                text: "Creates the extension in \(location.path)/<name>"
            )
            guideRow(
                symbol: "shippingbox",
                text: "Copies the vanilla JS + Tailwind starter kit (a working panel, topbar item, and command)"
            )
            guideRow(
                symbol: "doc.text",
                text: "Adds CLAUDE.md and AGENTS.md and bundles the muxy-extension skill"
            )
            guideRow(
                symbol: "sidebar.left",
                text: "Adds the directory to the Muxy sidebar as a project"
            )
            guideRow(
                symbol: "wand.and.stars",
                text: "Open your agentic coding tool there and ask it to write the extension"
            )
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
    }

    private func guideRow(symbol: String, text: String) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: symbol)
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.accent)
                .frame(width: 14, alignment: .center)
            Text(text)
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
        }
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

    private func chooseLocation() {
        guard let directory = ExtensionFolderPicker.pick(
            title: "Extension Location",
            message: "Choose the parent folder for the new extension.",
            directory: location
        )
        else { return }
        location = directory
    }

    private func create() {
        errorMessage = nil
        let request = ExtensionScaffoldRequest(name: name, version: version, description: description)
        do {
            let directory = try ExtensionScaffoldService.create(request, in: location)
            if isDefaultLocation {
                store.reload()
            } else {
                store.addDevPath(directory.path)
            }
            onFinish()
            NSApp.keyWindow?.close()
            NotificationCenter.default.post(
                name: .openExtensionDirectoryAsProject,
                object: nil,
                userInfo: [OpenExtensionDirectoryUserInfoKey.path: directory.path]
            )
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
