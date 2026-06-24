import SwiftUI

struct BrowserImportSheet: View {
    let targetProfile: BrowserProfile
    let onDismiss: () -> Void

    @State private var profiles: [ImportableProfile] = []
    @State private var loadError: String?
    @State private var isImporting = false

    private let source: BrowserImportSource = .chrome

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing6) {
            header
            content
            footer
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(440))
        .task { await load() }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Text("Import to “\(targetProfile.name)”")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
            Text("Choose a \(source.displayName) profile to copy cookies from. macOS may ask for Keychain permission.")
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    @ViewBuilder
    private var content: some View {
        if let loadError {
            messageBox(loadError)
        } else if profiles.isEmpty {
            messageBox("No \(source.displayName) profiles with cookies were found.")
        } else {
            VStack(spacing: 0) {
                ForEach(profiles) { profile in
                    importRow(profile)
                    if profile.id != profiles.last?.id {
                        Divider()
                    }
                }
            }
            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .overlay(
                RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                    .stroke(SettingsStyle.border, lineWidth: 1)
            )
        }
    }

    private func importRow(_ profile: ImportableProfile) -> some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "person.crop.circle")
                .font(.system(size: SettingsMetrics.labelFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
            Text(profile.name)
                .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.foreground)
            Spacer()
            Button("Import") { runImport(profile) }
                .disabled(isImporting)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, UIMetrics.spacing3)
    }

    private func messageBox(_ message: String) -> some View {
        Text(message)
            .font(.system(size: SettingsMetrics.labelFontSize))
            .foregroundStyle(SettingsStyle.mutedForeground)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(SettingsMetrics.horizontalPadding)
            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
    }

    private var footer: some View {
        HStack {
            if isImporting {
                ProgressView().controlSize(.small)
            }
            Spacer()
            Button("Close", action: onDismiss)
                .keyboardShortcut(.cancelAction)
        }
    }

    private func load() async {
        do {
            profiles = try await Task.detached(priority: .userInitiated) {
                let importer = CookieImportService.importer(for: source)
                guard importer.isInstalled() else {
                    throw BrowserImportError.sourceNotInstalled
                }
                return try importer.availableProfiles()
            }.value
        } catch {
            loadError = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
        }
    }

    private func runImport(_ profile: ImportableProfile) {
        isImporting = true
        Task {
            do {
                let result = try await CookieImportService.importCookies(
                    from: source,
                    profile: profile,
                    into: targetProfile.id
                )
                ToastState.shared.show("Imported \(result.imported) cookies into “\(targetProfile.name)”")
                onDismiss()
            } catch {
                let message = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
                ToastState.shared.show(message)
            }
            isImporting = false
        }
    }
}
