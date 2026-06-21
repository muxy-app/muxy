import SwiftUI

struct BrowserImportSheet: View {
    let targetProfile: BrowserProfile
    let onDismiss: () -> Void

    @State private var profiles: [ImportableProfile] = []
    @State private var loadError: String?
    @State private var isImporting = false

    private let source: BrowserImportSource = .chrome

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text("Import to “\(targetProfile.name)”")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            Text("Choose a \(source.displayName) profile. macOS may ask for Keychain permission to read its cookies.")
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .fixedSize(horizontal: false, vertical: true)

            content

            HStack(spacing: UIMetrics.spacing3) {
                Spacer()
                Button("Close", action: onDismiss)
                    .keyboardShortcut(.cancelAction)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(420))
        .task { load() }
    }

    @ViewBuilder
    private var content: some View {
        if let loadError {
            Text(loadError)
                .font(.system(size: SettingsMetrics.labelFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
        } else if profiles.isEmpty {
            Text("No \(source.displayName) profiles with cookies were found.")
                .font(.system(size: SettingsMetrics.labelFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
        } else {
            VStack(spacing: 4) {
                ForEach(profiles) { profile in
                    importRow(profile)
                }
            }
        }
    }

    private func importRow(_ profile: ImportableProfile) -> some View {
        HStack(spacing: 10) {
            Text(profile.name)
                .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.foreground)
            Spacer()
            Button("Import") { runImport(profile) }
                .disabled(isImporting)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private func load() {
        let importer = CookieImportService.importer(for: source)
        guard importer.isInstalled() else {
            loadError = "\(source.displayName) is not installed."
            return
        }
        do {
            profiles = try importer.availableProfiles()
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
