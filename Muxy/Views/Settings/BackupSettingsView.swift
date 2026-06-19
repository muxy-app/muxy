import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct BackupSettingsView: View {
    private static let exportFooter = """
    Saves your settings, projects, remote devices, shortcuts and customizations to a single .muxy file. \
    Credentials such as SSH keys, passwords and paired mobile devices are never included.
    """

    private static let importFooter = """
    Replaces all current Muxy data with the contents of a backup and restarts the app. \
    Your current data is backed up first so it can be recovered.
    """

    @State private var isWorking = false
    @State private var errorMessage: String?
    @State private var pendingImportURL: URL?

    private var importConfirmationBinding: Binding<Bool> {
        Binding(
            get: { pendingImportURL != nil },
            set: { if !$0 { pendingImportURL = nil } }
        )
    }

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Export",
                footer: Self.exportFooter
            ) {
                actionRow(
                    title: "Export Muxy",
                    description: "Create a backup you can import on another Mac.",
                    buttonTitle: "Export…",
                    action: performExport
                )
            }

            SettingsSection(
                "Import",
                footer: Self.importFooter,
                showsDivider: false
            ) {
                actionRow(
                    title: "Import Muxy",
                    description: "Restore a backup created on this or another Mac.",
                    buttonTitle: "Import…",
                    action: chooseImportFile
                )

                if let errorMessage {
                    Text(errorMessage)
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.destructive)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.horizontal, SettingsMetrics.horizontalPadding)
                        .padding(.bottom, SettingsMetrics.rowVerticalPadding)
                }
            }
        }
        .disabled(isWorking)
        .alert("Import backup?", isPresented: importConfirmationBinding) {
            Button("Import & Restart", role: .destructive, action: performImport)
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This replaces all current Muxy data and restarts the app. Your current data is backed up first.")
        }
    }

    private func actionRow(
        title: String,
        description: String,
        buttonTitle: String,
        action: @escaping () -> Void
    ) -> some View {
        SettingsRow(title) {
            Button(buttonTitle, action: action)
                .buttonStyle(.plain)
                .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.accent)
        }
        .help(description)
    }

    private func performExport() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [BackupArchive.contentType]
        panel.nameFieldStringValue = defaultExportName()
        panel.message = "Choose where to save the Muxy backup"
        guard panel.runModal() == .OK, let url = panel.url else { return }

        errorMessage = nil
        isWorking = true
        let version = appVersion
        SettingsJSONStore.syncUserSettingsFileWithCurrentSettings()
        Task {
            do {
                try await BackupService().export(to: url, appVersion: version, createdAt: Date())
                ToastState.shared.show(title: "Export complete", body: url.lastPathComponent)
            } catch {
                errorMessage = error.localizedDescription
            }
            isWorking = false
        }
    }

    private func chooseImportFile() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        let delegate = BackupOpenPanelDelegate()
        panel.delegate = delegate
        panel.message = "Select a Muxy backup to import"
        guard panel.runModal() == .OK, let url = panel.url else { return }

        errorMessage = nil
        DispatchQueue.main.async {
            pendingImportURL = url
        }
    }

    private func performImport() {
        guard let url = pendingImportURL else { return }
        pendingImportURL = nil
        isWorking = true
        let stamp = backupStamp()
        Task {
            do {
                try await BackupService().importBackup(from: url, backupStamp: stamp)
                try SettingsJSONStore.applyUserSettingsFile()
                isWorking = false
                try AppRelaunch.relaunch()
            } catch {
                errorMessage = error.localizedDescription
                isWorking = false
            }
        }
    }

    private func defaultExportName() -> String {
        let host = Host.current().localizedName ?? "Mac"
        let sanitizedHost = host.components(separatedBy: CharacterSet.alphanumerics.inverted)
            .filter { !$0.isEmpty }
            .joined(separator: "-")
        return "Muxy-Backup-\(sanitizedHost).\(BackupArchive.fileExtension)"
    }

    private func backupStamp() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd-HHmmss"
        return formatter.string(from: Date())
    }

    private var appVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
    }
}

private final class BackupOpenPanelDelegate: NSObject, NSOpenSavePanelDelegate {
    func panel(_ sender: Any, shouldEnable url: URL) -> Bool {
        var isDirectory: ObjCBool = false
        if FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory), isDirectory.boolValue {
            return true
        }
        return url.pathExtension.lowercased() == BackupArchive.fileExtension
    }
}
