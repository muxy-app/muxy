import SwiftUI

struct TerminalSettingsView: View {
    @AppStorage(GeneralSettingsKeys.autoCopyTerminalSelection)
    private var autoCopyTerminalSelection = false
    @AppStorage(GeneralSettingsKeys.terminalFileOpenBehavior)
    private var terminalFileOpenBehavior = TerminalFileOpenBehavior.defaultBehavior.rawValue
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

            SettingsSection(
                "Files",
                footer: "Choose what happens when you cmd-click a file path in the terminal. "
                    + "Open in external editor keeps the current behavior, trying a registered opener first "
                    + "and then launching your external editor. Open with in-app opener only uses a registered "
                    + "extension opener and shows a notice instead of launching an external editor when none is registered."
            ) {
                SettingsPickerRow<TerminalFileOpenBehavior>(
                    label: "Cmd-click a file path",
                    selection: $terminalFileOpenBehavior,
                    width: 220
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
