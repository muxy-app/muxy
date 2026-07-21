import SwiftUI

struct KeyboardShortcutsSettingsView: View {
    @Environment(\.settingsSearchQuery) private var settingsSearchQuery
    @State private var recordingAction: ShortcutAction?
    @State private var searchText = ""
    @State private var conflictWarning: (action: ShortcutAction, message: String)?
    @State private var recordingExtensionShortcutID: String?
    @State private var extensionConflictWarning: (id: String, message: String)?
    @State private var isRecordingNotchTerminalShortcut = false
    @State private var notchTerminalShortcutError: String?
    @AppStorage(NotchTerminalSizePreferences.widthKey)
    private var notchTerminalWidth = NotchTerminalSizePreferences.defaultWidth
    @AppStorage(NotchTerminalSizePreferences.heightKey)
    private var notchTerminalHeight = NotchTerminalSizePreferences.defaultHeight
    @AppStorage(NotchTerminalAppearancePreferences.transparencyKey)
    private var notchTerminalTransparency = NotchTerminalAppearancePreferences.defaultTransparency
    @AppStorage(NotchTerminalAppearancePreferences.blurIntensityKey)
    private var notchTerminalBlurIntensity = NotchTerminalAppearancePreferences.defaultBlurIntensity

    private var store: KeyBindingStore { KeyBindingStore.shared }
    private var extensionStore: ExtensionShortcutStore { ExtensionShortcutStore.shared }
    private var notchShortcutService: NotchTerminalShortcutService { NotchTerminalShortcutService.shared }

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
                    try notchShortcutService.resetShortcut()
                    store.resetToDefaults()
                    notchTerminalShortcutError = nil
                } catch {
                    notchTerminalShortcutError = error.localizedDescription
                }
                recordingAction = nil
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
                if notchTerminalMatchesSearch {
                    notchTerminalSection(showsDivider: !visibleCategories.isEmpty || !extensionGroups.isEmpty)
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

    private var notchTerminalMatchesSearch: Bool {
        searchText.isEmpty || SettingsCatalog.matchingItems(query: searchText).contains {
            $0.section == "Notch Terminal"
        }
    }

    private func notchTerminalSection(showsDivider: Bool) -> some View {
        SettingsSection("Notch Terminal", showsDivider: showsDivider) {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Open Notch Terminal")
                            .font(.system(size: SettingsMetrics.labelFontSize))
                        Text(notchTerminalStatusText)
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(notchTerminalStatusColor)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)

                    Button {
                        updateNotchTerminalShortcut(.doubleShift)
                    } label: {
                        Label(
                            "Double Shift",
                            systemImage: notchShortcutService.shortcut == .doubleShift
                                ? "checkmark.circle.fill"
                                : "circle"
                        )
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)

                    ZStack {
                        if isRecordingNotchTerminalShortcut {
                            ShortcutRecorderView(
                                onRecord: { _ in },
                                onCancel: { isRecordingNotchTerminalShortcut = false },
                                onRecordWithKeyCode: recordNotchTerminalShortcut
                            )
                            .frame(width: 0, height: 0)
                            .opacity(0)
                        }
                        Button(isRecordingNotchTerminalShortcut ? "Press shortcut…" : customShortcutTitle) {
                            isRecordingNotchTerminalShortcut = true
                            notchTerminalShortcutError = nil
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    }
                }

                if notchShortcutService.needsInputMonitoringAccess {
                    HStack(spacing: 8) {
                        Text("Double Shift needs Input Monitoring outside Muxy.")
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.mutedForeground)
                        Spacer()
                        Button("Enable Input Monitoring") {
                            _ = notchShortcutService.requestInputMonitoringAccess()
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
                    NotchTerminalDimensionField(
                        label: "Width",
                        value: $notchTerminalWidth,
                        range: NotchTerminalSizePreferences.widthRange
                    )
                    Text("×")
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    Text("Height")
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    NotchTerminalDimensionField(
                        label: "Height",
                        value: $notchTerminalHeight,
                        range: NotchTerminalSizePreferences.heightRange
                    )
                    Button("Reset") {
                        notchTerminalWidth = NotchTerminalSizePreferences.defaultWidth
                        notchTerminalHeight = NotchTerminalSizePreferences.defaultHeight
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }

                HStack(spacing: 8) {
                    Text("Terminal transparency")
                        .font(.system(size: SettingsMetrics.labelFontSize))
                    Spacer()
                    Slider(
                        value: notchTerminalTransparencyBinding,
                        in: Double(NotchTerminalAppearancePreferences.transparencyRange.lowerBound)
                            ... Double(NotchTerminalAppearancePreferences.transparencyRange.upperBound),
                        step: 1
                    )
                    .frame(width: 220)
                    .accessibilityLabel("Terminal transparency")
                    Text("\(displayedNotchTerminalTransparency)%")
                        .font(.system(size: SettingsMetrics.footnoteFontSize).monospacedDigit())
                        .foregroundStyle(SettingsStyle.mutedForeground)
                        .frame(width: 34, alignment: .trailing)
                }

                HStack(spacing: 8) {
                    Text("Background vibrancy")
                        .font(.system(size: SettingsMetrics.labelFontSize))
                    Spacer()
                    Slider(
                        value: notchTerminalBlurIntensityBinding,
                        in: Double(NotchTerminalAppearancePreferences.blurIntensityRange.lowerBound)
                            ... Double(NotchTerminalAppearancePreferences.blurIntensityRange.upperBound),
                        step: 1
                    )
                    .frame(width: 220)
                    .accessibilityLabel("Background vibrancy")
                    Text("\(displayedNotchTerminalBlurIntensity)%")
                        .font(.system(size: SettingsMetrics.footnoteFontSize).monospacedDigit())
                        .foregroundStyle(SettingsStyle.mutedForeground)
                        .frame(width: 34, alignment: .trailing)
                    Button("Reset") {
                        notchTerminalTransparency = NotchTerminalAppearancePreferences.defaultTransparency
                        notchTerminalBlurIntensity = NotchTerminalAppearancePreferences.defaultBlurIntensity
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }

                if let errorMessage = notchTerminalShortcutError ?? notchShortcutService.errorMessage {
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
        guard case let .keyCombo(combo, _) = notchShortcutService.shortcut else { return "Record Custom…" }
        return combo.displayString
    }

    private var notchTerminalTransparencyBinding: Binding<Double> {
        Binding(
            get: { Double(displayedNotchTerminalTransparency) },
            set: { notchTerminalTransparency = Int($0.rounded()) }
        )
    }

    private var displayedNotchTerminalTransparency: Int {
        min(
            max(notchTerminalTransparency, NotchTerminalAppearancePreferences.transparencyRange.lowerBound),
            NotchTerminalAppearancePreferences.transparencyRange.upperBound
        )
    }

    private var notchTerminalBlurIntensityBinding: Binding<Double> {
        Binding(
            get: { Double(displayedNotchTerminalBlurIntensity) },
            set: { notchTerminalBlurIntensity = Int($0.rounded()) }
        )
    }

    private var displayedNotchTerminalBlurIntensity: Int {
        min(
            max(notchTerminalBlurIntensity, NotchTerminalAppearancePreferences.blurIntensityRange.lowerBound),
            NotchTerminalAppearancePreferences.blurIntensityRange.upperBound
        )
    }

    private var notchTerminalStatusText: String {
        switch notchShortcutService.monitoringState {
        case .systemWide,
             .carbonHotKey:
            "Active system-wide"
        case .localOnly:
            "Active while Muxy is focused"
        case .stopped:
            "Inactive"
        }
    }

    private var notchTerminalStatusColor: Color {
        switch notchShortcutService.monitoringState {
        case .systemWide,
             .carbonHotKey:
            SettingsStyle.accent
        case .localOnly,
             .stopped:
            SettingsStyle.warning
        }
    }

    private func recordNotchTerminalShortcut(_ combo: KeyCombo, virtualKeyCode: UInt16) {
        updateNotchTerminalShortcut(.keyCombo(combo, virtualKeyCode: virtualKeyCode))
        if notchTerminalShortcutError == nil {
            isRecordingNotchTerminalShortcut = false
        }
    }

    private func updateNotchTerminalShortcut(_ shortcut: NotchTerminalShortcut) {
        if case let .keyCombo(combo, _) = shortcut,
           let conflict = NotchTerminalShortcutConflictResolver.conflictMessage(for: combo)
        {
            notchTerminalShortcutError = conflict
            return
        }
        do {
            try notchShortcutService.updateShortcut(shortcut)
            notchTerminalShortcutError = nil
            isRecordingNotchTerminalShortcut = false
        } catch {
            notchTerminalShortcutError = error.localizedDescription
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

    private func handleRecord(extensionEntry entry: ExtensionShortcutEntry, combo: KeyCombo) {
        if let message = extensionStore.conflictMessage(
            for: combo,
            extensionID: entry.extensionID,
            commandID: entry.commandID
        ) {
            extensionConflictWarning = (id: entry.id, message: "\(message) — press a different shortcut or Esc to cancel")
            return
        }
        extensionStore.updateCombo(extensionID: entry.extensionID, commandID: entry.commandID, combo: combo)
        recordingExtensionShortcutID = nil
        extensionConflictWarning = nil
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

    private func handleRecord(action: ShortcutAction, combo: KeyCombo) {
        if let message = NotchTerminalShortcutConflictResolver.notchTerminalConflictMessage(for: combo) {
            conflictWarning = (action: action, message: "\(message) Press a different shortcut or Esc to cancel.")
            return
        }
        if let existing = store.conflictingAction(for: combo, excluding: action) {
            conflictWarning = (
                action: action,
                message: "Conflicts with \"\(existing.displayName)\". Press a different shortcut or Esc to cancel."
            )
            return
        }
        store.updateBinding(action: action, combo: combo)
        recordingAction = nil
        conflictWarning = nil
    }

    private func resetBinding(action: ShortcutAction) {
        if let message = NotchTerminalShortcutConflictResolver.appShortcutResetConflictMessage(for: action) {
            conflictWarning = (action: action, message: message)
            return
        }
        store.resetBinding(action: action)
        conflictWarning = nil
    }
}

private struct NotchTerminalDimensionField: View {
    let label: String
    @Binding var value: Int
    let range: ClosedRange<Int>
    @State private var input = NotchTerminalDimensionInput()
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

struct NotchTerminalDimensionInput: Equatable {
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
    let onRecord: (KeyCombo) -> Void
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
