import AppKit
import SwiftUI

struct TerminalSettingsView: View {
    @AppStorage(GeneralSettingsKeys.autoCopyTerminalSelection)
    private var autoCopyTerminalSelection = false
    @AppStorage(TabCloseConfirmationPreferences.confirmRunningProcessKey)
    private var confirmRunningProcess = true
    @AppStorage(TerminalPersistentSessionPreferences.enabledKey)
    private var backgroundSessionsEnabled = TerminalPersistentSessionPreferences.defaultIsEnabled
    @AppStorage(TerminalOfflinePreferences.enabledKey)
    private var freeIdleTerminalsEnabled = TerminalOfflinePreferences.defaultIsEnabled
    @AppStorage(TerminalOfflinePreferences.idleThresholdKey)
    private var idleThresholdSeconds = TerminalOfflinePreferences.defaultIdleThreshold

    private func confirmStoppingBackgroundSessions() {
        Task { @MainActor in
            let sessions = await PersistentSessionService.shared.liveSessions()
            guard !sessions.isEmpty else { return }

            let alert = NSAlert()
            alert.messageText = L10n.string("Stop background terminals?")
            alert.informativeText = L10n.string(
                "Terminals are still running in the background. Turning this off stops them and ends whatever they are running."
            )
            alert.alertStyle = .warning
            alert.addButton(withTitle: L10n.string("Stop"))
            alert.addButton(withTitle: L10n.string("Cancel"))
            alert.buttons[1].keyEquivalent = "\u{1b}"

            guard alert.runModal() == .alertFirstButtonReturn else {
                backgroundSessionsEnabled = true
                return
            }
            await PersistentSessionService.shared.endAllSessions()
        }
    }

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
                footer: L10n.resource("When enabled, releasing the mouse after selecting text in the terminal copies it to the clipboard.")
            ) {
                SettingsToggleRow(
                    label: L10n.resource("Auto-copy selected text"),
                    isOn: $autoCopyTerminalSelection
                )
            }

            SettingsSection("Tabs") {
                SettingsToggleRow(
                    label: L10n.resource("Confirm before closing a tab with a running process"),
                    isOn: $confirmRunningProcess
                )
            }

            SettingsSection(
                "Background sessions",
                footer: """
                Runs each new terminal in a separate background process, the way tmux does. Quitting Muxy leaves \
                those terminals running and reopening reconnects them with their recent output. Closing a tab still \
                ends its session. Use Send to Background from a terminal's context menu to close an eligible tab \
                without stopping its processes. Terminals that are already open keep their current behaviour.
                """
            ) {
                SettingsToggleRow(
                    label: L10n.resource("Run new terminals in the background"),
                    isOn: $backgroundSessionsEnabled
                )
                .disabled(!PersistentSessionService.shared.isAvailable)
                .onChange(of: backgroundSessionsEnabled) { _, isEnabled in
                    guard !isEnabled else { return }
                    confirmStoppingBackgroundSessions()
                }
            }

            SettingsSection(
                "Memory",
                footer: """
                Frees an idle terminal you are not actively using to reclaim memory, including visible split panes \
                that are not focused. It reopens in the same folder when you return. Tabs running a process or a \
                full-screen app are never touched.
                """
            ) {
                SettingsToggleRow(
                    label: L10n.resource("Free idle inactive terminals"),
                    isOn: $freeIdleTerminalsEnabled
                )
                .onChange(of: freeIdleTerminalsEnabled) { _, _ in
                    TerminalOfflineService.shared.reload()
                }
                SettingsPickerRow<TerminalOfflineTimeout>(
                    label: L10n.resource("Free after idle for"),
                    selection: idleTimeoutSelection,
                    width: 140
                )
                .disabled(!freeIdleTerminalsEnabled)
            }
        }
    }
}
