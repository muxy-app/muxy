import SwiftUI

enum GeneralSettingsKeys {
    static let autoExpandWorktreesOnProjectSwitch = "muxy.general.autoExpandWorktreesOnProjectSwitch"
}

struct GeneralSettingsView: View {
    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch)
    private var autoExpandWorktrees = false
    @AppStorage(TabCloseConfirmationPreferences.confirmRunningProcessKey)
    private var confirmRunningProcess = true
    @AppStorage(ProjectLifecyclePreferences.keepOpenWhenNoTabsKey)
    private var keepProjectsOpenWhenNoTabs = false
    @AppStorage(UpdateChannel.storageKey)
    private var updateChannelRaw = UpdateChannel.stable.rawValue
    @AppStorage(QuitConfirmationPreferences.confirmQuitKey)
    private var confirmQuit = true

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

            SettingsSection(
                "Sidebar",
                footer: "Automatically reveal worktrees when you switch to a project."
            ) {
                SettingsToggleRow(
                    label: "Auto-expand worktrees on project switch",
                    isOn: $autoExpandWorktrees
                )
            }

            SettingsSection(
                "Projects",
                footer: "Keep projects in the sidebar after closing their last tab. "
                    + "To remove a project afterward, use the right-click menu."
            ) {
                SettingsToggleRow(
                    label: "Keep projects open after closing the last tab",
                    isOn: $keepProjectsOpenWhenNoTabs
                )
            }

            SettingsSection("Tabs") {
                SettingsToggleRow(
                    label: "Confirm before closing a tab with a running process",
                    isOn: $confirmRunningProcess
                )
            }

            SettingsSection("Quit", showsDivider: false) {
                SettingsToggleRow(
                    label: "Confirm before quitting Muxy",
                    isOn: $confirmQuit
                )
            }
        }
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
