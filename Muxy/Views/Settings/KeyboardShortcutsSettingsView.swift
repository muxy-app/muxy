import SwiftUI

struct KeyboardShortcutsSettingsView: View {
    @Environment(\.settingsSearchQuery) private var settingsSearchQuery
    @State private var recordingAction: ShortcutAction?
    @State private var searchText = ""
    @State private var conflictWarning: (action: ShortcutAction, message: String)?
    @State private var recordingExtensionShortcutID: String?
    @State private var extensionConflictWarning: (id: String, message: String)?
    @State private var isRecordingQuickTerminalShortcut = false
    @State private var quickTerminalShortcutError: String?
    @AppStorage(QuickTerminalSizePreferences.widthKey)
    private var quickTerminalWidth = QuickTerminalSizePreferences.defaultWidth
    @AppStorage(QuickTerminalSizePreferences.heightKey)
    private var quickTerminalHeight = QuickTerminalSizePreferences.defaultHeight
    @AppStorage(QuickTerminalAppearancePreferences.transparencyKey)
    private var quickTerminalTransparency = QuickTerminalAppearancePreferences.defaultTransparency
    @AppStorage(QuickTerminalAppearancePreferences.blurIntensityKey)
    private var quickTerminalBlurIntensity = QuickTerminalAppearancePreferences.defaultBlurIntensity

    private var store: KeyBindingStore { KeyBindingStore.shared }
    private var extensionStore: ExtensionShortcutStore { ExtensionShortcutStore.shared }
    private var quickTerminalShortcutService: QuickTerminalShortcutService { QuickTerminalShortcutService.shared }

    var body: some View {
        VStack(spacing: 0) {
            header
            SettingsDivider()
            appShortcutsList
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                TextField("Search shortcuts", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .foregroundStyle(SettingsStyle.foreground)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 6))

            Button("Reset All") {
                do {
                    try quickTerminalShortcutService.resetShortcut()
                    store.resetToDefaults()
                    quickTerminalShortcutError = nil
                } catch {
                    quickTerminalShortcutError = error.localizedDescription
                }
                recordingAction = nil
                recordingExtensionShortcutID = nil
                isRecordingQuickTerminalShortcut = false
                conflictWarning = nil
            }
            .buttonStyle(.plain)
            .font(.system(size: SettingsMetrics.footnoteFontSize))
            .foregroundStyle(SettingsStyle.mutedForeground)
        }
        .padding(SettingsMetrics.horizontalPadding)
    }

    private var appShortcutsList: some View {
        let visibleCategories = ShortcutAction.categories.filter { !filteredActions(for: $0).isEmpty }
        let extensionGroups = filteredExtensionGroups
        return ScrollView(.vertical, showsIndicators: true) {
            VStack(spacing: 0) {
                if quickTerminalMatchesSearch {
                    quickTerminalSection(showsDivider: !visibleCategories.isEmpty || !extensionGroups.isEmpty)
                }
                ForEach(visibleCategories, id: \.self) { category in
                    categorySection(
                        title: category,
                        actions: filteredActions(for: category),
                        isLast: category == visibleCategories.last && extensionGroups.isEmpty
                    )
                }
                ForEach(extensionGroups) { group in
                    extensionSection(group: group, isLast: group.id == extensionGroups.last?.id)
                }
            }
        }
        .onAppear {
            searchText = settingsSearchQuery
        }
        .onChange(of: settingsSearchQuery) { _, query in
            searchText = query
        }
    }

    private var quickTerminalMatchesSearch: Bool {
        searchText.isEmpty || SettingsCatalog.matchingItems(query: searchText).contains {
            $0.section == "Quick Terminal"
        }
    }

    private func quickTerminalSection(showsDivider: Bool) -> some View {
        SettingsSection("Quick Terminal", showsDivider: showsDivider) {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Open Quick Terminal")
                            .font(.system(size: SettingsMetrics.labelFontSize))
                        Text(quickTerminalStatusText)
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(quickTerminalStatusColor)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)

                    Button {
                        _ = updateQuickTerminalShortcut(.doubleShift)
                    } label: {
                        Label(
                            "Double Shift",
                            systemImage: quickTerminalShortcutService.shortcut == .doubleShift
                                ? "checkmark.circle.fill"
                                : "circle"
                        )
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .accessibilityAddTraits(isDoubleShiftShortcutSelected ? .isSelected : [])
                    .accessibilityValue(isDoubleShiftShortcutSelected ? "Selected" : "Not selected")

                    ZStack {
                        if isRecordingQuickTerminalShortcut {
                            ShortcutRecorderView(
                                onRecord: { _ in false },
                                onCancel: { isRecordingQuickTerminalShortcut = false },
                                onRecordWithKeyCode: recordQuickTerminalShortcut
                            )
                            .frame(width: 0, height: 0)
                            .opacity(0)
                        }
                        Button(isRecordingQuickTerminalShortcut ? "Press shortcut…" : customShortcutTitle) {
                            recordingAction = nil
                            recordingExtensionShortcutID = nil
                            isRecordingQuickTerminalShortcut = true
                            quickTerminalShortcutError = nil
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .accessibilityAddTraits(isCustomShortcutSelected ? .isSelected : [])
                        .accessibilityValue(isCustomShortcutSelected ? "Selected" : "Not selected")
                    }
                }

                if quickTerminalShortcutService.needsInputMonitoringAccess {
                    HStack(spacing: 8) {
                        Text("Double Shift needs Input Monitoring outside Muxy.")
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.mutedForeground)
                        Spacer()
                        Button("Enable Input Monitoring") {
                            _ = quickTerminalShortcutService.requestInputMonitoringAccess()
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                    }
                }

                HStack(spacing: 8) {
                    Text("Terminal size")
                        .font(.system(size: SettingsMetrics.labelFontSize))
                    Spacer()
                    Text("Width")
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    QuickTerminalDimensionField(
                        label: "Width",
                        value: $quickTerminalWidth,
                        range: QuickTerminalSizePreferences.widthRange
                    )
                    Text("×")
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    Text("Height")
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    QuickTerminalDimensionField(
                        label: "Height",
                        value: $quickTerminalHeight,
                        range: QuickTerminalSizePreferences.heightRange
                    )
                    Button("Reset") {
                        quickTerminalWidth = QuickTerminalSizePreferences.defaultWidth
                        quickTerminalHeight = QuickTerminalSizePreferences.defaultHeight
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }

                HStack(spacing: 8) {
                    Text("Terminal transparency")
                        .font(.system(size: SettingsMetrics.labelFontSize))
                    Spacer()
                    Slider(
                        value: quickTerminalTransparencyBinding,
                        in: Double(QuickTerminalAppearancePreferences.transparencyRange.lowerBound)
                            ... Double(QuickTerminalAppearancePreferences.transparencyRange.upperBound),
                        step: 1
                    )
                    .frame(width: 220)
                    .accessibilityLabel("Terminal transparency")
                    Text("\(displayedQuickTerminalTransparency)%")
                        .font(.system(size: SettingsMetrics.footnoteFontSize).monospacedDigit())
                        .foregroundStyle(SettingsStyle.mutedForeground)
                        .frame(width: 34, alignment: .trailing)
                }

                HStack(spacing: 8) {
                    Text("Background vibrancy")
                        .font(.system(size: SettingsMetrics.labelFontSize))
                    Spacer()
                    Slider(
                        value: quickTerminalBlurIntensityBinding,
                        in: Double(QuickTerminalAppearancePreferences.blurIntensityRange.lowerBound)
                            ... Double(QuickTerminalAppearancePreferences.blurIntensityRange.upperBound),
                        step: 1
                    )
                    .frame(width: 220)
                    .accessibilityLabel("Background vibrancy")
                    Text("\(displayedQuickTerminalBlurIntensity)%")
                        .font(.system(size: SettingsMetrics.footnoteFontSize).monospacedDigit())
                        .foregroundStyle(SettingsStyle.mutedForeground)
                        .frame(width: 34, alignment: .trailing)
                    Button("Reset") {
                        quickTerminalTransparency = QuickTerminalAppearancePreferences.defaultTransparency
                        quickTerminalBlurIntensity = QuickTerminalAppearancePreferences.defaultBlurIntensity
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }

                if let errorMessage = quickTerminalShortcutError ?? quickTerminalShortcutService.errorMessage {
                    Text(errorMessage)
                        .font(.system(size: 10))
                        .foregroundStyle(SettingsStyle.warning)
                }
            }
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.vertical, SettingsMetrics.rowVerticalPadding)
        }
        .environment(\.settingsSearchQuery, "")
    }

    private var customShortcutTitle: String {
        guard case .keyCombo = quickTerminalShortcutService.shortcut else { return "Record Custom…" }
        return quickTerminalShortcutService.shortcut.displayString
    }

    private var isDoubleShiftShortcutSelected: Bool {
        quickTerminalShortcutService.shortcut == .doubleShift
    }

    private var isCustomShortcutSelected: Bool {
        guard case .keyCombo = quickTerminalShortcutService.shortcut else { return false }
        return true
    }

    private var quickTerminalTransparencyBinding: Binding<Double> {
        Binding(
            get: { Double(displayedQuickTerminalTransparency) },
            set: { quickTerminalTransparency = Int($0.rounded()) }
        )
    }

    private var displayedQuickTerminalTransparency: Int {
        min(
            max(quickTerminalTransparency, QuickTerminalAppearancePreferences.transparencyRange.lowerBound),
            QuickTerminalAppearancePreferences.transparencyRange.upperBound
        )
    }

    private var quickTerminalBlurIntensityBinding: Binding<Double> {
        Binding(
            get: { Double(displayedQuickTerminalBlurIntensity) },
            set: { quickTerminalBlurIntensity = Int($0.rounded()) }
        )
    }

    private var displayedQuickTerminalBlurIntensity: Int {
        min(
            max(quickTerminalBlurIntensity, QuickTerminalAppearancePreferences.blurIntensityRange.lowerBound),
            QuickTerminalAppearancePreferences.blurIntensityRange.upperBound
        )
    }

    private var quickTerminalStatusText: String {
        switch quickTerminalShortcutService.monitoringState {
        case .systemWide,
             .carbonHotKey:
            "Active system-wide"
        case .localOnly:
            "Active while Muxy is focused"
        case .stopped:
            "Inactive"
        }
    }

    private var quickTerminalStatusColor: Color {
        switch quickTerminalShortcutService.monitoringState {
        case .systemWide,
             .carbonHotKey:
            SettingsStyle.accent
        case .localOnly,
             .stopped:
            SettingsStyle.warning
        }
    }

    private func recordQuickTerminalShortcut(_ combo: KeyCombo, virtualKeyCode: UInt16) -> Bool {
        updateQuickTerminalShortcut(.keyCombo(combo, virtualKeyCode: virtualKeyCode))
    }

    private func updateQuickTerminalShortcut(_ shortcut: QuickTerminalShortcut) -> Bool {
        if case let .keyCombo(combo, _) = shortcut,
           let conflict = QuickTerminalShortcutConflictResolver.conflictMessage(for: combo)
        {
            quickTerminalShortcutError = conflict
            return false
        }
        do {
            try quickTerminalShortcutService.updateShortcut(shortcut)
            quickTerminalShortcutError = nil
            isRecordingQuickTerminalShortcut = false
            return true
        } catch {
            quickTerminalShortcutError = error.localizedDescription
            return false
        }
    }

    private func extensionSection(group: ExtensionShortcutGroup, isLast: Bool) -> some View {
        SettingsSection(group.extensionName, showsDivider: !isLast) {
            ForEach(group.entries) { entry in
                ShortcutRow(
                    title: entry.commandTitle,
                    combo: entry.combo,
                    isRecording: recordingExtensionShortcutID == entry.id,
                    conflictMessage: extensionConflictWarning?.id == entry.id ? extensionConflictWarning?.message : nil,
                    onStartRecording: {
                        recordingAction = nil
                        isRecordingQuickTerminalShortcut = false
                        recordingExtensionShortcutID = entry.id
                        extensionConflictWarning = nil
                    },
                    onRecord: { combo in handleRecord(extensionEntry: entry, combo: combo) },
                    onCancel: {
                        recordingExtensionShortcutID = nil
                        extensionConflictWarning = nil
                    },
                    onReset: {
                        extensionStore.resetCombo(
                            extensionID: entry.extensionID,
                            commandID: entry.commandID,
                            defaultCombo: entry.defaultCombo
                        )
                        recordingExtensionShortcutID = nil
                        extensionConflictWarning = nil
                    },
                    onUnassign: {
                        extensionStore.unassign(extensionID: entry.extensionID, commandID: entry.commandID)
                        recordingExtensionShortcutID = nil
                        extensionConflictWarning = nil
                    }
                )
            }
        }
        .environment(\.settingsSearchQuery, "")
    }

    private func handleRecord(extensionEntry entry: ExtensionShortcutEntry, combo: KeyCombo) -> Bool {
        if let message = extensionStore.conflictMessage(
            for: combo,
            extensionID: entry.extensionID,
            commandID: entry.commandID
        ) {
            extensionConflictWarning = (id: entry.id, message: "\(message) — press a different shortcut or Esc to cancel")
            return false
        }
        extensionStore.updateCombo(extensionID: entry.extensionID, commandID: entry.commandID, combo: combo)
        recordingExtensionShortcutID = nil
        extensionConflictWarning = nil
        return true
    }

    private var filteredExtensionGroups: [ExtensionShortcutGroup] {
        let groups = ExtensionShortcutGroup.build(
            shortcuts: extensionStore.shortcuts,
            statuses: ExtensionStore.shared.statuses
        )
        guard !searchText.isEmpty else { return groups }
        return groups.compactMap { group in
            let entries = group.entries.filter {
                $0.commandTitle.localizedCaseInsensitiveContains(searchText)
                    || group.extensionName.localizedCaseInsensitiveContains(searchText)
            }
            guard !entries.isEmpty else { return nil }
            return ExtensionShortcutGroup(extensionID: group.extensionID, extensionName: group.extensionName, entries: entries)
        }
    }

    private func categorySection(title: String, actions: [ShortcutAction], isLast: Bool) -> some View {
        SettingsSection(title, showsDivider: !isLast) {
            ForEach(actions) { action in
                ShortcutRow(
                    title: action.displayName,
                    combo: store.combo(for: action),
                    isRecording: recordingAction == action,
                    conflictMessage: conflictWarning?.action == action
                        ? conflictWarning?.message
                        : nil,
                    onStartRecording: {
                        recordingExtensionShortcutID = nil
                        isRecordingQuickTerminalShortcut = false
                        recordingAction = action
                        conflictWarning = nil
                    },
                    onRecord: { combo in handleRecord(action: action, combo: combo) },
                    onCancel: { recordingAction = nil
                        conflictWarning = nil
                    },
                    onReset: { resetBinding(action: action) },
                    onUnassign: {
                        store.updateBinding(action: action, combo: KeyCombo(key: "", modifiers: 0))
                        recordingAction = nil
                        conflictWarning = nil
                    }
                )
            }
        }
        .environment(\.settingsSearchQuery, "")
    }

    private func filteredActions(for category: String) -> [ShortcutAction] {
        let actions = ShortcutAction.allCases.filter { $0.category == category }
        guard !searchText.isEmpty else { return actions }
        return actions.filter { $0.displayName.localizedCaseInsensitiveContains(searchText) }
    }

    private func handleRecord(action: ShortcutAction, combo: KeyCombo) -> Bool {
        if let message = QuickTerminalShortcutConflictResolver.quickTerminalConflictMessage(for: combo) {
            conflictWarning = (action: action, message: "\(message) Press a different shortcut or Esc to cancel.")
            return false
        }
        if let existing = store.conflictingAction(for: combo, excluding: action) {
            conflictWarning = (
                action: action,
                message: "Conflicts with \"\(existing.displayName)\". Press a different shortcut or Esc to cancel."
            )
            return false
        }
        store.updateBinding(action: action, combo: combo)
        recordingAction = nil
        conflictWarning = nil
        return true
    }

    private func resetBinding(action: ShortcutAction) {
        if let message = QuickTerminalShortcutConflictResolver.appShortcutResetConflictMessage(for: action) {
            conflictWarning = (action: action, message: message)
            return
        }
        store.resetBinding(action: action)
        conflictWarning = nil
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

private struct ShortcutRow: View {
    let title: String
    let combo: KeyCombo
    let isRecording: Bool
    let conflictMessage: String?
    let onStartRecording: () -> Void
    let onRecord: (KeyCombo) -> Bool
    let onCancel: () -> Void
    let onReset: () -> Void
    let onUnassign: () -> Void
    @State private var hovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(title)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .frame(maxWidth: .infinity, alignment: .leading)

                if isRecording {
                    recordingView
                } else {
                    comboDisplay
                }
            }

            if let conflictMessage {
                Text(conflictMessage)
                    .font(.system(size: 10))
                    .foregroundStyle(SettingsStyle.warning)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
        .background(hovered ? SettingsStyle.hover : .clear)
        .onHover { hovered = $0 }
    }

    private var comboDisplay: some View {
        HStack(spacing: 6) {
            if hovered {
                Button(action: onUnassign) {
                    Image(systemName: "xmark")
                        .font(.system(size: 10))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                }
                .buttonStyle(.plain)
                .disabled(!combo.isAssigned)
                .accessibilityLabel("Unassign Shortcut")

                Button(action: onReset) {
                    Image(systemName: "arrow.counterclockwise")
                        .font(.system(size: 10))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Reset Shortcut")
            }

            Button(action: onStartRecording) {
                Text(combo.isAssigned ? combo.displayString : "Unassigned")
                    .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium, design: .rounded))
                    .foregroundStyle(SettingsStyle.foreground)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)
        }
    }

    private var recordingView: some View {
        ZStack {
            ShortcutRecorderView(onRecord: onRecord, onCancel: onCancel)
                .frame(width: 0, height: 0)
                .opacity(0)

            Text("Press shortcut…")
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.warning)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(SettingsStyle.warning.opacity(0.12), in: RoundedRectangle(cornerRadius: 5))
        }
    }
}
