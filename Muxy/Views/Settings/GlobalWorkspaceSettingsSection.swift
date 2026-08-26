import SwiftUI

struct GlobalWorkspaceSettingsSection: View {
    @State private var isRecordingShortcut = false
    @State private var shortcutError: String?
    @AppStorage(GlobalWorkspacePreferences.enabledKey)
    private var isEnabled = GlobalWorkspacePreferences.defaultEnabled
    @AppStorage(GlobalWorkspacePreferences.triggerKey)
    private var triggerRaw = GlobalWorkspacePreferences.defaultTrigger.rawValue
    @AppStorage(GlobalWorkspacePreferences.doubleTapIntervalMillisecondsKey)
    private var doubleTapIntervalMilliseconds = GlobalWorkspacePreferences.defaultDoubleTapIntervalMilliseconds
    @AppStorage(GlobalWorkspacePreferences.toggleToHideKey)
    private var toggleToHide = GlobalWorkspacePreferences.defaultToggleToHide

    private var shortcutService: GlobalWorkspaceShortcutService { .shared }

    private var selectedTrigger: GlobalWorkspaceTrigger {
        GlobalWorkspaceTrigger(rawValue: triggerRaw) ?? GlobalWorkspacePreferences.defaultTrigger
    }

    private var intervalBinding: Binding<Double> {
        Binding(
            get: {
                GlobalWorkspacePreferences.clampedDoubleTapIntervalMilliseconds(doubleTapIntervalMilliseconds)
            },
            set: { newValue in
                let clamped = GlobalWorkspacePreferences.clampedDoubleTapIntervalMilliseconds(newValue)
                let step = GlobalWorkspacePreferences.doubleTapIntervalStepMilliseconds
                doubleTapIntervalMilliseconds = (clamped / step).rounded() * step
            }
        )
    }

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "General",
                footer: "Double-tap the selected modifier to show a full Muxy workspace from any app."
            ) {
                SettingsToggleRow(label: "Enable Global Workspace", isOn: $isEnabled)
            }

            SettingsSection("Shortcut") {
                SettingsRow("Trigger") {
                    HStack(spacing: UIMetrics.spacing2) {
                        Menu {
                            ForEach(GlobalWorkspaceTrigger.allCases.filter { $0 != .custom }) { trigger in
                                Button {
                                    selectTrigger(trigger)
                                } label: {
                                    if trigger == selectedTrigger {
                                        Label(trigger.title, systemImage: "checkmark")
                                    } else {
                                        Text(trigger.title)
                                    }
                                }
                            }
                        } label: {
                            HStack(spacing: UIMetrics.spacing2) {
                                Text(doubleModifierLabel)
                                    .foregroundStyle(SettingsStyle.foreground)
                                Spacer(minLength: 0)
                                Image(systemName: "chevron.up.chevron.down")
                                    .font(.system(size: 9, weight: .semibold))
                                    .foregroundStyle(SettingsStyle.mutedForeground)
                            }
                            .font(.system(size: SettingsMetrics.labelFontSize))
                            .padding(.horizontal, 8)
                            .frame(width: 138, height: 26)
                            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 6))
                            .overlay(
                                RoundedRectangle(cornerRadius: 6)
                                    .stroke(SettingsStyle.border, lineWidth: 1)
                            )
                            .contentShape(Rectangle())
                        }
                        .menuStyle(.borderlessButton)
                        .fixedSize()

                        ZStack {
                            if isRecordingShortcut {
                                ShortcutRecorderView(
                                    onRecord: { _ in false },
                                    onCancel: { isRecordingShortcut = false },
                                    onRecordWithKeyCode: recordShortcut
                                )
                                .frame(width: 0, height: 0)
                                .opacity(0)
                            }
                            Button {
                                isRecordingShortcut = true
                                shortcutError = nil
                            } label: {
                                if isRecordingShortcut {
                                    Text(L10n.resource("Press shortcut…"))
                                } else if selectedTrigger == .custom,
                                          let shortcut = shortcutService.configuration.customShortcut
                                {
                                    Text(verbatim: shortcut.displayString)
                                } else {
                                    Text(L10n.resource("Record Custom…"))
                                }
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                            .accessibilityAddTraits(selectedTrigger == .custom ? .isSelected : [])
                        }
                    }
                    .disabled(!isEnabled)
                }

                SettingsRow("Double Tap Interval") {
                    HStack(spacing: UIMetrics.spacing3) {
                        Slider(
                            value: intervalBinding,
                            in: GlobalWorkspacePreferences.minimumDoubleTapIntervalMilliseconds
                                ... GlobalWorkspacePreferences.maximumDoubleTapIntervalMilliseconds
                        )
                        Text("\(Int(intervalBinding.wrappedValue.rounded())) ms")
                            .font(.system(size: SettingsMetrics.labelFontSize).monospacedDigit())
                            .foregroundStyle(SettingsStyle.mutedForeground)
                            .frame(width: 58, alignment: .trailing)
                    }
                    .frame(width: SettingsMetrics.controlWidth)
                    .disabled(!isEnabled || selectedTrigger == .custom)
                }

                SettingsToggleRow(label: "Toggle to Hide", isOn: $toggleToHide)
                    .disabled(!isEnabled)

                if shortcutService.needsInputMonitoringAccess {
                    SettingsRow("System-wide Access") {
                        Button("Enable Input Monitoring") {
                            _ = shortcutService.requestInputMonitoringAccess()
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                    }
                }

                if let errorMessage = shortcutError ?? shortcutService.errorMessage {
                    SettingsRow("Shortcut Error") {
                        Text(errorMessage)
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.warning)
                    }
                }

                SettingsRow("Status") {
                    Text(statusText)
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(statusColor)
                }
            }
        }
    }

    private var doubleModifierLabel: String {
        selectedTrigger.title
    }

    private func selectTrigger(_ trigger: GlobalWorkspaceTrigger) {
        do {
            try GlobalWorkspacePreferences.setShortcutConfiguration(.init(trigger: trigger))
            shortcutService.refresh()
            shortcutError = nil
        } catch {
            shortcutError = error.localizedDescription
        }
    }

    private func recordShortcut(_ combo: KeyCombo, virtualKeyCode: UInt16) -> Bool {
        if let conflict = globalWorkspaceConflictMessage(for: combo) {
            shortcutError = conflict
            return false
        }
        do {
            try shortcutService.updateCustomShortcut(.keyCombo(combo, virtualKeyCode: virtualKeyCode))
            shortcutError = nil
            isRecordingShortcut = false
            return true
        } catch {
            shortcutError = error.localizedDescription
            return false
        }
    }

    private func globalWorkspaceConflictMessage(for combo: KeyCombo) -> String? {
        QuickTerminalShortcutConflictResolver.conflictMessage(for: combo)
            ?? QuickTerminalShortcutConflictResolver.quickTerminalConflictMessage(for: combo)
    }

    private var statusText: String {
        guard isEnabled else { return "Disabled" }
        guard selectedTrigger != .custom || shortcutService.configuration.customShortcut != nil else {
            return "No shortcut assigned"
        }
        return switch shortcutService.monitoringState {
        case .systemWide:
            "Active system-wide"
        case .localOnly:
            "Active while Muxy is focused"
        case .carbonHotKey:
            "Active system-wide"
        case .stopped:
            "Inactive"
        }
    }

    private var statusColor: Color {
        guard isEnabled else { return SettingsStyle.mutedForeground }
        return switch shortcutService.monitoringState {
        case .systemWide,
             .carbonHotKey:
            SettingsStyle.accent
        case .localOnly,
             .stopped:
            SettingsStyle.warning
        }
    }
}
