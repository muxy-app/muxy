import AppKit
import SwiftUI

struct BrowserSettingsView: View {
    @AppStorage(BrowserPreferences.persistDataKey)
    private var persistData = BrowserPreferences.defaultPersistData
    @AppStorage(BrowserPreferences.autoOpenDevServerKey)
    private var autoOpenDevServer = BrowserPreferences.defaultAutoOpenDevServer
    @AppStorage(BrowserPreferences.inspectableKey)
    private var inspectable = BrowserPreferences.defaultInspectable
    @AppStorage(BrowserPreferences.homeURLKey)
    private var homeURL = ""

    @State private var homeURLDraft: String = ""
    @State private var homeURLError: String?
    @State private var isClearingData = false
    @State private var clearDataMessage: String?

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Home",
                footer: "Used when opening a new browser tab without a URL. Leave blank to use \(BrowserPreferences.defaultHomeURL)."
            ) {
                SettingsRow("Home page") {
                    TextField(BrowserPreferences.defaultHomeURL, text: $homeURLDraft, onCommit: commitHomeURL)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: SettingsMetrics.controlWidth)
                        .onSubmit(commitHomeURL)
                }
                if let homeURLError {
                    SettingsFootnote(
                        text: homeURLError,
                        color: .red
                    )
                }
            }

            SettingsSection(
                "Sessions",
                footer: persistFooter
            ) {
                SettingsToggleRow(
                    label: "Persist cookies and site data between launches",
                    isOn: persistDataBinding
                )
                SettingsRow("Stored data") {
                    Button(isClearingData ? "Clearing…" : "Clear Browsing Data") {
                        clearBrowsingData()
                    }
                    .disabled(isClearingData)
                }
                if let clearDataMessage {
                    SettingsFootnote(text: clearDataMessage)
                }
            }

            SettingsSection(
                "Dev Servers",
                footer: "When enabled, Muxy probes common ports after recognised dev-server commands "
                    + "(npm/pnpm/yarn dev, vite, next, uvicorn, etc.) and opens the URL in the worktree that started the server."
            ) {
                SettingsToggleRow(
                    label: "Open dev server URL in a browser tab",
                    isOn: $autoOpenDevServer
                )
            }

            SettingsSection(
                "Developer",
                footer: "When enabled, browser tabs expose Safari's Web Inspector via Right-Click → Inspect Element.",
                showsDivider: false
            ) {
                SettingsToggleRow(
                    label: "Enable Web Inspector",
                    isOn: $inspectable
                )
            }
        }
        .onAppear { homeURLDraft = homeURL }
        .onChange(of: homeURL) { _, newValue in homeURLDraft = newValue }
    }

    private var persistFooter: String {
        "When disabled, the in-app browser uses an ephemeral profile that is cleared between launches. "
            + "Persistence changes only apply to new browser tabs; existing tabs keep their current session."
    }

    private var persistDataBinding: Binding<Bool> {
        Binding(
            get: { persistData },
            set: { newValue in
                let wasPersistent = persistData
                persistData = newValue
                if wasPersistent, !newValue {
                    confirmAndClearPersistentStore()
                }
            }
        )
    }

    private func commitHomeURL() {
        let trimmed = homeURLDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            homeURL = ""
            homeURLError = nil
            return
        }
        guard let url = BrowserURLNormalizer.normalize(trimmed),
              BrowserURLNormalizer.isAllowedNavigationURL(url)
        else {
            homeURLError = "Enter a valid URL (e.g. https://example.com)."
            return
        }
        homeURL = url.absoluteString
        homeURLDraft = url.absoluteString
        homeURLError = nil
    }

    private func confirmAndClearPersistentStore() {
        let alert = NSAlert()
        alert.messageText = "Clear stored cookies and data?"
        alert.informativeText = "Persistence is now off. You can keep any cookies and data Muxy already saved, or remove them now."
        alert.alertStyle = .informational
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Clear Now")
        alert.addButton(withTitle: "Keep")
        let response = alert.runModal()
        guard response == .alertFirstButtonReturn else { return }
        Task { @MainActor in
            await BrowserDataStoreFactory.clearPersistentData()
            clearDataMessage = "Cleared previously stored browsing data."
        }
    }

    private func clearBrowsingData() {
        isClearingData = true
        clearDataMessage = nil
        Task { @MainActor in
            await BrowserDataStoreFactory.clearAllBrowsingData()
            isClearingData = false
            clearDataMessage = "Browsing data cleared for current and stored sessions."
        }
    }
}

private struct SettingsFootnote: View {
    let text: String
    var color: Color = .secondary

    var body: some View {
        Text(text)
            .font(.system(size: SettingsMetrics.footnoteFontSize))
            .foregroundStyle(color)
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.bottom, SettingsMetrics.rowVerticalPadding)
    }
}
