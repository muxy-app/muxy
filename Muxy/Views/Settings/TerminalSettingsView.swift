import SwiftUI

struct TerminalSettingsView: View {
    @AppStorage(GeneralSettingsKeys.autoCopyTerminalSelection)
    private var autoCopyTerminalSelection = false
    @AppStorage(TabCloseConfirmationPreferences.confirmRunningProcessKey)
    private var confirmRunningProcess = true
    @AppStorage(TerminalOfflinePreferences.enabledKey)
    private var freeIdleTerminalsEnabled = TerminalOfflinePreferences.defaultIsEnabled
    @AppStorage(TerminalOfflinePreferences.idleThresholdKey)
    private var idleThresholdSeconds = TerminalOfflinePreferences.defaultIdleThreshold

    private var idleTimeoutSelection: Binding<String> {
        Binding(
            get: { TerminalOfflineTimeout.closest(to: idleThresholdSeconds).rawValue },
            set: { rawValue in
                guard let option = TerminalOfflineTimeout(rawValue: rawValue) else { return }
                idleThresholdSeconds = option.seconds
                TerminalOfflineService.shared.reload()
            }
        )
    }

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Selection",
                footer: "When enabled, releasing the mouse after selecting text in the terminal copies it to the clipboard."
            ) {
                SettingsToggleRow(
                    label: "Auto-copy selected text",
                    isOn: $autoCopyTerminalSelection
                )
            }

            SettingsSection("Tabs") {
                SettingsToggleRow(
                    label: "Confirm before closing a tab with a running process",
                    isOn: $confirmRunningProcess
                )
            }

            SettingsSection(
                "Memory",
                footer: "Frees an idle terminal you are not actively using to reclaim memory, including "
                    + "visible split panes that are not focused. It reopens in the same folder when you return. "
                    + "Tabs running a process or a full-screen app are never touched."
            ) {
                SettingsToggleRow(
                    label: "Free idle inactive terminals",
                    isOn: $freeIdleTerminalsEnabled
                )
                .onChange(of: freeIdleTerminalsEnabled) { _, _ in
                    TerminalOfflineService.shared.reload()
                }
                SettingsPickerRow<TerminalOfflineTimeout>(
                    label: "Free after idle for",
                    selection: idleTimeoutSelection,
                    width: 140
                )
                .disabled(!freeIdleTerminalsEnabled)
            }
        }
    }
}
