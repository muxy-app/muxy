import SwiftUI

struct CommandsSettingsView: View {
    @State private var recordingCommandPrefix = false
    @State private var recordingCommandShortcutID: UUID?
    @State private var pendingCommandShortcutID: UUID?
    @State private var searchText = ""
    @State private var commandPrefixConflictWarning: String?
    @State private var commandConflictWarning: (id: UUID, message: String)?
    @State private var deleteAllCommandShortcutsSecondsRemaining = 0
    @State private var deleteAllCommandShortcutsTask: Task<Void, Never>?

    private var commandStore: CommandShortcutStore { CommandShortcutStore.shared }

    var body: some View {
        VStack(spacing: 0) {
            header
            SettingsDivider()
            list
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                TextField("Search commands", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .foregroundStyle(SettingsStyle.foreground)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 6))

            Button {
                searchText = ""
                discardPendingCommandShortcut()
                let shortcut = commandStore.addShortcut()
                pendingCommandShortcutID = shortcut.id
                recordingCommandPrefix = false
                recordingCommandShortcutID = shortcut.id
            } label: {
                Image(systemName: "plus")
                    .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .semibold))
            }
            .buttonStyle(.plain)
            .help("Add Command")
            .accessibilityLabel("Add Command")
        }
        .padding(SettingsMetrics.horizontalPadding)
    }

    private var list: some View {
        ScrollView(.vertical, showsIndicators: true) {
            commandShortcutsSection
        }
    }

    private var commandShortcutsSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Press the command layer shortcut, then a command key to open a new terminal tab.")
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, SettingsMetrics.horizontalPadding)
                .padding(.top, SettingsMetrics.sectionHeaderTopPadding)
                .padding(.bottom, SettingsMetrics.sectionHeaderBottomPadding)

            CommandPrefixRow(
                combo: commandStore.prefixCombo,
                isRecording: recordingCommandPrefix,
                conflictMessage: commandPrefixConflictWarning,
                onStartRecording: {
                    discardPendingCommandShortcut()
                    recordingCommandPrefix = true
                    recordingCommandShortcutID = nil
                    commandPrefixConflictWarning = nil
                    commandConflictWarning = nil
                },
                onRecord: handleRecord(prefixCombo:),
                onCancel: {
                    recordingCommandPrefix = false
                    commandPrefixConflictWarning = nil
                },
                onReset: {
                    resetCommandPrefix()
                }
            )

            ForEach(filteredCommandShortcuts) { shortcut in
                CommandShortcutRow(
                    shortcut: binding(for: shortcut),
                    prefixCombo: commandStore.prefixCombo,
                    isRecording: recordingCommandShortcutID == shortcut.id,
                    conflictMessage: commandConflictWarning?.id == shortcut.id ? commandConflictWarning?.message : nil,
                    onStartRecording: {
                        if pendingCommandShortcutID != shortcut.id {
                            discardPendingCommandShortcut()
                        }
                        recordingCommandPrefix = false
                        recordingCommandShortcutID = shortcut.id
                        commandConflictWarning = nil
                    },
                    onRecord: { combo in handleRecord(shortcutID: shortcut.id, combo: combo) },
                    onCancel: {
                        cancelCommandShortcutRecording(shortcutID: shortcut.id)
                    },
                    onDelete: {
                        commandStore.deleteShortcut(id: shortcut.id)
                        if recordingCommandShortcutID == shortcut.id {
                            recordingCommandShortcutID = nil
                        }
                        if pendingCommandShortcutID == shortcut.id {
                            pendingCommandShortcutID = nil
                        }
                        if commandConflictWarning?.id == shortcut.id {
                            commandConflictWarning = nil
                        }
                    }
                )
            }

            if !commandStore.shortcuts.isEmpty {
                DeleteAllCommandShortcutsRow(
                    secondsRemaining: deleteAllCommandShortcutsSecondsRemaining,
                    action: handleDeleteAllCommandShortcuts
                )
            }
        }
        .onDisappear {
            discardPendingCommandShortcut()
            cancelDeleteAllCommandShortcutsConfirmation()
        }
    }

    private var filteredCommandShortcuts: [CommandShortcut] {
        guard !searchText.isEmpty else { return commandStore.shortcuts }
        return commandStore.shortcuts.filter {
            $0.displayName.localizedCaseInsensitiveContains(searchText)
                || $0.command.localizedCaseInsensitiveContains(searchText)
        }
    }

    private func handleRecord(prefixCombo combo: KeyCombo) -> Bool {
        if let message = QuickTerminalShortcutConflictResolver.quickTerminalConflictMessage(for: combo) {
            commandPrefixConflictWarning = message
            return false
        }
        commandStore.updatePrefixCombo(combo)
        recordingCommandPrefix = false
        commandPrefixConflictWarning = nil
        return true
    }

    private func resetCommandPrefix() {
        if let message = QuickTerminalShortcutConflictResolver.commandPrefixResetConflictMessage() {
            commandPrefixConflictWarning = message
            return
        }
        commandStore.resetPrefixCombo()
        recordingCommandPrefix = false
        commandPrefixConflictWarning = nil
    }

    private func handleRecord(shortcutID: UUID, combo: KeyCombo) -> Bool {
        if let message = QuickTerminalShortcutConflictResolver.quickTerminalConflictMessage(for: combo) {
            commandConflictWarning = (id: shortcutID, message: message)
            return false
        }
        if let existing = commandStore.conflictingShortcut(for: combo, excluding: shortcutID) {
            commandConflictWarning = (id: shortcutID, message: "Conflicts with \"\(existing.displayName)\"")
            return false
        }
        guard var shortcut = commandStore.shortcuts.first(where: { $0.id == shortcutID }) else { return false }
        shortcut.combo = combo
        commandStore.updateShortcut(shortcut)
        if pendingCommandShortcutID == shortcutID {
            pendingCommandShortcutID = nil
        }
        recordingCommandShortcutID = nil
        commandConflictWarning = nil
        return true
    }

    private func cancelCommandShortcutRecording(shortcutID: UUID) {
        if pendingCommandShortcutID == shortcutID {
            commandStore.deleteShortcut(id: shortcutID)
            pendingCommandShortcutID = nil
        }
        recordingCommandShortcutID = nil
        commandConflictWarning = nil
    }

    private func discardPendingCommandShortcut() {
        guard let shortcutID = pendingCommandShortcutID else { return }
        commandStore.deleteShortcut(id: shortcutID)
        pendingCommandShortcutID = nil
        if recordingCommandShortcutID == shortcutID {
            recordingCommandShortcutID = nil
        }
        if commandConflictWarning?.id == shortcutID {
            commandConflictWarning = nil
        }
    }

    private func binding(for shortcut: CommandShortcut) -> Binding<CommandShortcut> {
        Binding {
            commandStore.shortcuts.first { $0.id == shortcut.id } ?? shortcut
        } set: { updated in
            commandStore.updateShortcut(updated)
        }
    }

    private func handleDeleteAllCommandShortcuts() {
        guard deleteAllCommandShortcutsSecondsRemaining == 0 else {
            commandStore.deleteAllShortcuts()
            pendingCommandShortcutID = nil
            recordingCommandShortcutID = nil
            commandConflictWarning = nil
            cancelDeleteAllCommandShortcutsConfirmation()
            return
        }

        startDeleteAllCommandShortcutsConfirmation()
    }

    private func startDeleteAllCommandShortcutsConfirmation() {
        deleteAllCommandShortcutsTask?.cancel()
        deleteAllCommandShortcutsTask = Task { @MainActor in
            for seconds in stride(from: 5, through: 1, by: -1) {
                deleteAllCommandShortcutsSecondsRemaining = seconds
                do {
                    try await Task.sleep(for: .seconds(1))
                } catch {
                    return
                }
            }
            deleteAllCommandShortcutsSecondsRemaining = 0
            deleteAllCommandShortcutsTask = nil
        }
    }

    private func cancelDeleteAllCommandShortcutsConfirmation() {
        deleteAllCommandShortcutsTask?.cancel()
        deleteAllCommandShortcutsTask = nil
        deleteAllCommandShortcutsSecondsRemaining = 0
    }
}

private struct CommandPrefixRow: View {
    let combo: KeyCombo
    let isRecording: Bool
    let conflictMessage: String?
    let onStartRecording: () -> Void
    let onRecord: (KeyCombo) -> Bool
    let onCancel: () -> Void
    let onReset: () -> Void
    @State private var hovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text("Command Layer")
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .frame(maxWidth: .infinity, alignment: .leading)

                if isRecording {
                    recordingView
                } else {
                    comboDisplay
                }
            }

            if let conflictMessage {
                Text("\(conflictMessage) — press a different shortcut or Esc to cancel")
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
                Button(action: onReset) {
                    Image(systemName: "arrow.counterclockwise")
                        .font(.system(size: 10))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Reset Shortcut")
            }

            Button(action: onStartRecording) {
                Text(combo.displayString)
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

private struct CommandShortcutRow: View {
    private enum Metrics {
        static let deleteButtonSize: CGFloat = 18
        static let shortcutControlWidth: CGFloat = 130
    }

    @Binding var shortcut: CommandShortcut
    let prefixCombo: KeyCombo
    let isRecording: Bool
    let conflictMessage: String?
    let onStartRecording: () -> Void
    let onRecord: (KeyCombo) -> Bool
    let onCancel: () -> Void
    let onDelete: () -> Void
    @State private var hovered = false
    @State private var deleteButtonHovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                TextField("Name", text: $shortcut.name)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .settingsTextInput(width: 120)

                TextField("Command", text: $shortcut.command)
                    .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                    .settingsTextInput(maxWidth: .infinity)

                if isRecording {
                    recordingView
                } else {
                    comboDisplay
                }
            }

            if let conflictMessage {
                Text("\(conflictMessage) — press a different shortcut or Esc to cancel")
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
            Button(action: onStartRecording) {
                Text("\(prefixCombo.displayString) \(shortcut.combo.displayString)")
                    .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium, design: .rounded))
                    .foregroundStyle(SettingsStyle.foreground)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)

            Button(action: onDelete) {
                Image(systemName: "trash")
                    .font(.system(size: 10))
                    .foregroundStyle(
                        deleteButtonHovered ? AnyShapeStyle(SettingsStyle.destructive) : AnyShapeStyle(SettingsStyle.mutedForeground)
                    )
                    .frame(width: Metrics.deleteButtonSize, height: Metrics.deleteButtonSize)
            }
            .buttonStyle(.plain)
            .background(
                deleteButtonHovered ? AnyShapeStyle(SettingsStyle.destructiveSoft) : AnyShapeStyle(SettingsStyle.surface),
                in: RoundedRectangle(cornerRadius: 5)
            )
            .onHover { isHovering in
                deleteButtonHovered = isHovering
            }
            .accessibilityLabel("Delete Command")
        }
        .frame(alignment: .trailing)
    }

    private var recordingView: some View {
        ZStack {
            ShortcutRecorderView(onRecord: onRecord, onCancel: onCancel, requiresModifier: false)
                .frame(width: 0, height: 0)
                .opacity(0)

            Text("Press key…")
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.warning)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(SettingsStyle.warning.opacity(0.12), in: RoundedRectangle(cornerRadius: 5))
        }
    }
}

private struct DeleteAllCommandShortcutsRow: View {
    let secondsRemaining: Int
    let action: () -> Void

    private var isConfirming: Bool {
        secondsRemaining > 0
    }

    var body: some View {
        HStack {
            Spacer()

            Button(action: action) {
                Text(title)
                    .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                    .foregroundStyle(isConfirming ? SettingsStyle.destructive : SettingsStyle.mutedForeground)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(backgroundStyle, in: RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(title)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private var title: String {
        if isConfirming {
            return "Confirm Delete All (\(secondsRemaining))"
        }
        return "Delete All"
    }

    private var backgroundStyle: AnyShapeStyle {
        isConfirming ? AnyShapeStyle(SettingsStyle.destructiveSoft) : AnyShapeStyle(SettingsStyle.surface)
    }
}
