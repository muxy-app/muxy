import SwiftUI

struct GeneralSettingsView: View {
    @AppStorage(UpdateChannel.storageKey)
    private var updateChannelRaw = UpdateChannel.stable.rawValue
    @AppStorage(QuitConfirmationPreferences.confirmQuitKey)
    private var confirmQuit = true
    @State private var sentry = SentryService.shared

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Updates",
                footer: "The Beta channel ships every change merged to main and may be unstable. "
                    + "Switch back to Stable to receive only tagged releases."
            ) {
                SettingsRow("Update channel") {
                    Picker("", selection: channelBinding) {
                        ForEach(UpdateChannel.allCases) { channel in
                            Text(channel.displayName).tag(channel)
                        }
                    }
                    .labelsHidden()
                    .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
                }
            }

            SettingsSection("Quit", showsDivider: sentry.hasDSN) {
                SettingsToggleRow(
                    label: "Confirm before quitting Muxy",
                    isOn: $confirmQuit
                )
            }

            if sentry.hasDSN {
                SettingsSection(
                    "Diagnostics",
                    footer: "Anonymous crash reports help us fix bugs. "
                        + "Reports never include project paths, file contents, or personal data.",
                    showsDivider: false
                ) {
                    SettingsToggleRow(
                        label: "Send anonymous crash reports",
                        isOn: sentryConsentBinding
                    )
                }
            }
        }
    }

    private var sentryConsentBinding: Binding<Bool> {
        Binding(
            get: { sentry.consent == .allowed },
            set: { newValue in sentry.setConsent(newValue ? .allowed : .denied) }
        )
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
}
