import AppKit
import SwiftUI

struct GeneralSettingsView: View {
    @AppStorage(UpdateChannel.storageKey)
    private var updateChannelRaw = UpdateChannel.stable.rawValue
    @AppStorage(QuitConfirmationPreferences.confirmQuitKey)
    private var confirmQuit = true
    @AppStorage(ProfilerService.enabledKey)
    private var profilerEnabled = false
    @State private var sentry = SentryService.shared
    @State private var updateService = UpdateService.shared

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Updates",
                footer: """
                The Beta channel ships every change merged to main and may be unstable. Switch back to Stable to \
                receive only tagged releases.
                """
            ) {
                SettingsRow("Update channel") {
                    Picker("", selection: channelBinding) {
                        ForEach(UpdateChannel.allCases) { channel in
                            Text(L10n.resource(key: channel.displayName)).tag(channel)
                        }
                    }
                    .labelsHidden()
                    .settingsControl()
                }
                SettingsToggleRow(
                    label: L10n.resource("Install Downloaded Updates on Quit"),
                    isOn: automaticUpdatesBinding
                )
                .disabled(!updateService.allowsAutomaticUpdates)
                .help(L10n.string("Automatic updates are unavailable for this configuration."))
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

    private var channelBinding: Binding<UpdateChannel> {
        Binding(
            get: { UpdateChannel(rawValue: updateChannelRaw) ?? .stable },
            set: { newValue in
                updateChannelRaw = newValue.rawValue
                UpdateService.shared.channel = newValue
            }
        )
    }

    private var automaticUpdatesBinding: Binding<Bool> {
        Binding(
            get: { updateService.automaticallyDownloadsUpdates },
            set: { updateService.setAutomaticallyDownloadsUpdates($0) }
        )
    }
}
