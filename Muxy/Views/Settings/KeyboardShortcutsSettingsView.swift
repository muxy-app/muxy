import SwiftUI

struct KeyboardShortcutsSettingsView: View {
    @Environment(\.settingsSearchQuery) private var settingsSearchQuery
    @State private var recordingAction: ShortcutAction?
    @State private var searchText = ""
    @State private var conflictWarning: (action: ShortcutAction, message: String)?
    @State private var recordingExtensionShortcutID: String?
    @State private var extensionConflictWarning: (id: String, message: String)?

    private var store: KeyBindingStore { KeyBindingStore.shared }
    private var extensionStore: ExtensionShortcutStore { ExtensionShortcutStore.shared }

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
                store.resetToDefaults()
                recordingAction = nil
                recordingExtensionShortcutID = nil
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
