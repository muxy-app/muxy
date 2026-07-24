import SwiftUI

struct QuickTerminalSettingsView: View {
    @State private var isRecordingShortcut = false
    @State private var shortcutError: String?
    @AppStorage(QuickTerminalPreferences.enabledKey)
    private var quickTerminalEnabled = QuickTerminalPreferences.defaultIsEnabled
    @AppStorage(QuickTerminalSizePreferences.widthKey)
    private var width = QuickTerminalSizePreferences.defaultWidth
    @AppStorage(QuickTerminalSizePreferences.heightKey)
    private var height = QuickTerminalSizePreferences.defaultHeight
    @AppStorage(QuickTerminalAppearancePreferences.transparencyKey)
    private var transparency = QuickTerminalAppearancePreferences.defaultTransparency
    @AppStorage(QuickTerminalAppearancePreferences.blurIntensityKey)
    private var blurIntensity = QuickTerminalAppearancePreferences.defaultBlurIntensity

    private var shortcutService: QuickTerminalShortcutService { QuickTerminalShortcutService.shared }

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "General",
                footer: quickTerminalEnabled ? nil : Self.disabledFooter
            ) {
                SettingsToggleRow(
                    label: "Enable Quick Terminal",
                    isOn: enabledBinding
                )
            }

            SettingsSection("Shortcut") {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Open Quick Terminal")
                                .font(.system(size: SettingsMetrics.labelFontSize))
                            Text(statusText)
                                .font(.system(size: SettingsMetrics.footnoteFontSize))
                                .foregroundStyle(statusColor)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)

                        Button {
                            _ = updateShortcut(.unassigned)
                        } label: {
                            Label(
                                "No Shortcut",
                                systemImage: shortcutService.shortcut == .unassigned
                                    ? "checkmark.circle.fill"
                                    : "circle"
                            )
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .accessibilityAddTraits(isNoShortcutSelected ? .isSelected : [])
                        .accessibilityValue(isNoShortcutSelected ? "Selected" : "Not selected")

                        Button {
                            _ = updateShortcut(.doubleShift)
                        } label: {
                            Label(
                                "Double Shift",
                                systemImage: shortcutService.shortcut == .doubleShift
                                    ? "checkmark.circle.fill"
                                    : "circle"
                            )
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .accessibilityAddTraits(isDoubleShiftSelected ? .isSelected : [])
                        .accessibilityValue(isDoubleShiftSelected ? "Selected" : "Not selected")

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
                            Button(isRecordingShortcut ? "Press shortcut…" : customShortcutTitle) {
                                isRecordingShortcut = true
                                shortcutError = nil
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                            .accessibilityAddTraits(isCustomShortcutSelected ? .isSelected : [])
                            .accessibilityValue(isCustomShortcutSelected ? "Selected" : "Not selected")
                        }
                    }

                    if shortcutService.needsInputMonitoringAccess {
                        HStack(spacing: 8) {
                            Text("Double Shift needs Input Monitoring outside Muxy.")
                                .font(.system(size: SettingsMetrics.footnoteFontSize))
                                .foregroundStyle(SettingsStyle.mutedForeground)
                            Spacer()
                            Button("Enable Input Monitoring") {
                                _ = shortcutService.requestInputMonitoringAccess()
                            }
                            .buttonStyle(.borderedProminent)
                            .controlSize(.small)
                        }
                    }

                    if let errorMessage = shortcutError ?? shortcutService.errorMessage {
                        Text(errorMessage)
                            .font(.system(size: 10))
                            .foregroundStyle(SettingsStyle.warning)
                    }
                }
                .padding(.horizontal, SettingsMetrics.horizontalPadding)
                .padding(.vertical, SettingsMetrics.rowVerticalPadding)
            }

            SettingsSection("Size") {
                HStack(spacing: 8) {
                    Text("Terminal size")
                        .font(.system(size: SettingsMetrics.labelFontSize))
                    Spacer()
                    Text("Width")
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    QuickTerminalDimensionField(
                        label: "Width",
                        value: $width,
                        range: QuickTerminalSizePreferences.widthRange
                    )
                    Text("×")
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    Text("Height")
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    QuickTerminalDimensionField(
                        label: "Height",
                        value: $height,
                        range: QuickTerminalSizePreferences.heightRange
                    )
                    Button("Reset") {
                        width = QuickTerminalSizePreferences.defaultWidth
                        height = QuickTerminalSizePreferences.defaultHeight
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
                .padding(.horizontal, SettingsMetrics.horizontalPadding)
                .padding(.vertical, SettingsMetrics.rowVerticalPadding)
            }

            SettingsSection("Appearance", showsDivider: false) {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Text("Terminal transparency")
                            .font(.system(size: SettingsMetrics.labelFontSize))
                        Spacer()
                        Slider(
                            value: transparencyBinding,
                            in: Double(QuickTerminalAppearancePreferences.transparencyRange.lowerBound)
                                ... Double(QuickTerminalAppearancePreferences.transparencyRange.upperBound),
                            step: 1
                        )
                        .frame(width: 220)
                        .accessibilityLabel("Terminal transparency")
                        Text("\(displayedTransparency)%")
                            .font(.system(size: SettingsMetrics.footnoteFontSize).monospacedDigit())
                            .foregroundStyle(SettingsStyle.mutedForeground)
                            .frame(width: 34, alignment: .trailing)
                    }

                    HStack(spacing: 8) {
                        Text("Background vibrancy")
                            .font(.system(size: SettingsMetrics.labelFontSize))
                        Spacer()
                        Slider(
                            value: blurIntensityBinding,
                            in: Double(QuickTerminalAppearancePreferences.blurIntensityRange.lowerBound)
                                ... Double(QuickTerminalAppearancePreferences.blurIntensityRange.upperBound),
                            step: 1
                        )
                        .frame(width: 220)
                        .accessibilityLabel("Background vibrancy")
                        Text("\(displayedBlurIntensity)%")
                            .font(.system(size: SettingsMetrics.footnoteFontSize).monospacedDigit())
                            .foregroundStyle(SettingsStyle.mutedForeground)
                            .frame(width: 34, alignment: .trailing)
                        Button("Reset") {
                            transparency = QuickTerminalAppearancePreferences.defaultTransparency
                            blurIntensity = QuickTerminalAppearancePreferences.defaultBlurIntensity
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    }
                }
                .padding(.horizontal, SettingsMetrics.horizontalPadding)
                .padding(.vertical, SettingsMetrics.rowVerticalPadding)
            }
        }
    }

    private static let disabledFooter = """
    The Quick Terminal shortcut listener and shell are off. Your shortcut, size, and appearance \
    settings are preserved.
    """

    private var enabledBinding: Binding<Bool> {
        Binding(
            get: { quickTerminalEnabled },
            set: { QuickTerminalPreferences.setEnabled($0) }
        )
    }

    private var customShortcutTitle: String {
        guard case .keyCombo = shortcutService.shortcut else { return "Record Custom…" }
        return shortcutService.shortcut.displayString
    }

    private var isDoubleShiftSelected: Bool {
        shortcutService.shortcut == .doubleShift
    }

    private var isNoShortcutSelected: Bool {
        shortcutService.shortcut == .unassigned
    }

    private var isCustomShortcutSelected: Bool {
        guard case .keyCombo = shortcutService.shortcut else { return false }
        return true
    }

    private var transparencyBinding: Binding<Double> {
        Binding(
            get: { Double(displayedTransparency) },
            set: { transparency = Int($0.rounded()) }
        )
    }

    private var displayedTransparency: Int {
        min(
            max(transparency, QuickTerminalAppearancePreferences.transparencyRange.lowerBound),
            QuickTerminalAppearancePreferences.transparencyRange.upperBound
        )
    }

    private var blurIntensityBinding: Binding<Double> {
        Binding(
            get: { Double(displayedBlurIntensity) },
            set: { blurIntensity = Int($0.rounded()) }
        )
    }

    private var displayedBlurIntensity: Int {
        min(
            max(blurIntensity, QuickTerminalAppearancePreferences.blurIntensityRange.lowerBound),
            QuickTerminalAppearancePreferences.blurIntensityRange.upperBound
        )
    }

    private var statusText: String {
        guard quickTerminalEnabled else { return "Disabled" }
        guard shortcutService.shortcut != .unassigned else { return "No shortcut assigned" }
        return switch shortcutService.monitoringState {
        case .systemWide,
             .carbonHotKey:
            "Active system-wide"
        case .localOnly:
            "Active while Muxy is focused"
        case .stopped:
            "Inactive"
        }
    }

    private var statusColor: Color {
        guard quickTerminalEnabled else { return SettingsStyle.mutedForeground }
        guard shortcutService.shortcut != .unassigned else { return SettingsStyle.mutedForeground }
        return switch shortcutService.monitoringState {
        case .systemWide,
             .carbonHotKey:
            SettingsStyle.accent
        case .localOnly,
             .stopped:
            SettingsStyle.warning
        }
    }

    private func recordShortcut(_ combo: KeyCombo, virtualKeyCode: UInt16) -> Bool {
        updateShortcut(.keyCombo(combo, virtualKeyCode: virtualKeyCode))
    }

    private func updateShortcut(_ shortcut: QuickTerminalShortcut) -> Bool {
        if case let .keyCombo(combo, _) = shortcut,
           let conflict = QuickTerminalShortcutConflictResolver.conflictMessage(for: combo)
        {
            shortcutError = conflict
            return false
        }
        do {
            try shortcutService.updateShortcut(shortcut)
            shortcutError = nil
            isRecordingShortcut = false
            return true
        } catch {
            shortcutError = error.localizedDescription
            return false
        }
    }
}

private struct QuickTerminalDimensionField: View {
    let label: String
    @Binding var value: Int
    let range: ClosedRange<Int>
    @State private var input = QuickTerminalDimensionInput()
    @FocusState private var isFocused: Bool

    var body: some View {
        HStack(spacing: 4) {
            TextField("", text: $input.text)
                .textFieldStyle(.plain)
                .font(.system(size: SettingsMetrics.labelFontSize).monospacedDigit())
                .multilineTextAlignment(.trailing)
                .frame(width: 48)
                .padding(.horizontal, 6)
                .padding(.vertical, 4)
                .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 5))
                .focused($isFocused)
                .onSubmit(commit)
                .accessibilityLabel(label)
            Text("pt")
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
        }
        .onAppear {
            input.synchronize(with: value)
        }
        .onChange(of: isFocused) { wasFocused, focused in
            guard wasFocused, !focused else { return }
            commit()
        }
        .onChange(of: value) { _, newValue in
            input.synchronize(with: newValue)
        }
    }

    private func commit() {
        value = input.commit(currentValue: value, range: range)
    }
}

struct QuickTerminalDimensionInput: Equatable {
    var text = ""

    mutating func synchronize(with value: Int) {
        text = String(value)
    }

    mutating func commit(currentValue: Int, range: ClosedRange<Int>) -> Int {
        guard let parsed = Int(text.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            synchronize(with: currentValue)
            return currentValue
        }
        let value = min(max(parsed, range.lowerBound), range.upperBound)
        synchronize(with: value)
        return value
    }
}
