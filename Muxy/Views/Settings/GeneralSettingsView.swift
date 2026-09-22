import AppKit
import SwiftUI

struct GeneralSettingsView: View {
    @AppStorage(QuitConfirmationPreferences.confirmQuitKey)
    private var confirmQuit = true
    @AppStorage(ProfilerService.enabledKey)
    private var profilerEnabled = false
    @State private var sentry = SentryService.shared

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Updates",
                footer: """
                Muxy 2 Beta is a separate app that can live next to this version and you can try it out. Install \
                Muxy Beta.app from the linked release; it will not replace this app or share its settings and sessions.
                """
            ) {
                SettingsRow("Try Muxy 2 Beta") {
                    Link(destination: HelpLinks.v2BetaReleasesURL) {
                        Text(L10n.resource("View beta releases"))
                    }
                    .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
                    .foregroundStyle(SettingsStyle.accent)
                }
            }

            SettingsSection("Quit") {
                SettingsToggleRow(
                    label: L10n.resource("Confirm before quitting Muxy"),
                    isOn: $confirmQuit
                )
            }

            SettingsSection(
                "Diagnostics",
                footer: diagnosticsFooter,
                showsDivider: false
            ) {
                if sentry.hasDSN {
                    SettingsToggleRow(
                        label: L10n.resource("Send anonymous crash reports"),
                        isOn: sentryConsentBinding
                    )
                }
                SettingsToggleRow(
                    label: L10n.resource("Record anonymous performance samples"),
                    isOn: profilerBinding
                )
                SettingsRow("Profiler data") {
                    Button(action: revealProfilerData) {
                        Text(L10n.resource("Reveal in Finder"))
                    }
                    .buttonStyle(.plain)
                    .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
                    .foregroundStyle(SettingsStyle.accent)
                }
            }
        }
    }

    private var diagnosticsFooter: LocalizedStringResource {
        if sentry.hasDSN {
            """
            Crash reports are sent only with your permission. Performance samples record CPU, memory, profiler uptime, \
            app and macOS versions, device architecture, and timestamps once per minute. They stay on this Mac unless \
            you share the file. Project paths, file contents, terminal output, and commands are never recorded.
            """
        } else {
            """
            Performance samples record CPU, memory, profiler uptime, app and macOS versions, device architecture, and \
            timestamps once per minute. They stay on this Mac unless you share the file. Project paths, file contents, \
            terminal output, and commands are never recorded.
            """
        }
    }

    private var sentryConsentBinding: Binding<Bool> {
        Binding(
            get: { sentry.consent == .allowed },
            set: { newValue in sentry.setConsent(newValue ? .allowed : .denied) }
        )
    }

    private var profilerBinding: Binding<Bool> {
        Binding(
            get: { profilerEnabled },
            set: { enabled in
                profilerEnabled = enabled
                ProfilerService.shared.setEnabled(enabled)
            }
        )
    }

    private func revealProfilerData() {
        let fileURL = ProfilerService.shared.fileURL
        let directoryURL = fileURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        let targetURL = FileManager.default.fileExists(atPath: fileURL.path) ? fileURL : directoryURL
        NSWorkspace.shared.activateFileViewerSelecting([targetURL])
    }
}
