import SwiftUI

struct KeyboardShortcutsSettingsView: View {
    private enum ListSection: String, CaseIterable, Identifiable {
        case app
        case custom

        var id: String { rawValue }

        var title: String {
            switch self {
            case .app: "App Shortcuts"
            case .custom: "Custom Commands"
            }
        }

        var searchPlaceholder: String {
            switch self {
            case .app: "Search shortcuts"
            case .custom: "Search commands"
            }
        }
    }

    @State private var section: ListSection = .app
    @State private var recordingAction: ShortcutAction?
    @State private var recordingCommandPrefix = false
    @State private var recordingCommandShortcutID: UUID?
    @State private var pendingCommandShortcutID: UUID?
    @State private var searchText = ""
    @State private var conflictWarning: (action: ShortcutAction, existing: ShortcutAction)?
    @State private var commandPrefixConflictWarning: String?
    @State private var commandConflictWarning: (id: UUID, message: String)?
    @State private var deleteAllCommandShortcutsSecondsRemaining = 0
    @State private var deleteAllCommandShortcutsTask: Task<Void, Never>?

    private var store: KeyBindingStore { KeyBindingStore.shared }
    private var commandStore: CommandShortcutStore { CommandShortcutStore.shared }

    var body: some View {
        VStack(spacing: 0) {
            sectionPicker
            Divider()
            header
            Divider()
            switch section {
            case .app: appShortcutsList
            case .custom: customShortcutsList
            }
        }
    }

    private var sectionPicker: some View {
        Picker("", selection: $section) {
            ForEach(ListSection.allCases) { section in
                Text(section.title).tag(section)
            }
        }
        .labelsHidden()
        .pickerStyle(.segmented)
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, 10)
        .onChange(of: section) { _, _ in
            discardPendingCommandShortcut()
            searchText = ""
            recordingAction = nil
            recordingCommandPrefix = false
            recordingCommandShortcutID = nil
            conflictWarning = nil
            commandPrefixConflictWarning = nil
            commandConflictWarning = nil
            cancelDeleteAllCommandShortcutsConfirmation()
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                TextField(section.searchPlaceholder, text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.system(size: SettingsMetrics.labelFontSize))
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))

            switch section {
            case .app:
                Button("Reset All") {
                    store.resetToDefaults()
                    recordingAction = nil
                    conflictWarning = nil
                }
                .buttonStyle(.plain)
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(.secondary)
            case .custom:
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
                .help("Add Command Shortcut")
                .accessibilityLabel("Add Command Shortcut")
            }
        }
        .padding(SettingsMetrics.horizontalPadding)
    }

    private var appShortcutsList: some View {
        let visibleCategories = ShortcutAction.categories.filter { !filteredActions(for: $0).isEmpty }
        return ScrollView(.vertical, showsIndicators: true) {
            VStack(spacing: 0) {
                ForEach(visibleCategories, id: \.self) { category in
                    categorySection(
                        title: category,
                        actions: filteredActions(for: category),
                        isLast: category == visibleCategories.last
                    )
                }
            }
        }
    }

    private var customShortcutsList: some View {
        ScrollView(.vertical, showsIndicators: true) {
            commandShortcutsSection
        }
    }

    private func categorySection(title: String, actions: [ShortcutAction], isLast: Bool) -> some View {
        SettingsSection(title, showsDivider: !isLast) {
            ForEach(actions) { action in
                ShortcutRow(
                    action: action,
                    combo: store.combo(for: action),
                    isRecording: recordingAction == action,
                    conflictAction: conflictWarning?.action == action ? conflictWarning?.existing : nil,
                    onStartRecording: {
                        discardPendingCommandShortcut()
                        recordingAction = action
                        recordingCommandPrefix = false
                        recordingCommandShortcutID = nil
                        conflictWarning = nil
                    },
                    onRecord: { combo in handleRecord(action: action, combo: combo) },
                    onCancel: { recordingAction = nil
                        conflictWarning = nil
                    },
                    onReset: { store.resetBinding(action: action)
                        conflictWarning = nil
                    }
                )
            }
        }
    }

    private func filteredActions(for category: String) -> [ShortcutAction] {
        let actions = ShortcutAction.allCases.filter { $0.category == category }
        guard !searchText.isEmpty else { return actions }
        return actions.filter { $0.displayName.localizedCaseInsensitiveContains(searchText) }
    }

    private var filteredCommandShortcuts: [CommandShortcut] {
        guard !searchText.isEmpty else { return commandStore.shortcuts }
        return commandStore.shortcuts.filter {
            $0.displayName.localizedCaseInsensitiveContains(searchText)
                || $0.command.localizedCaseInsensitiveContains(searchText)
        }
    }

    private var commandShortcutsSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Custom Commands")
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, SettingsMetrics.horizontalPadding)
                .padding(.top, SettingsMetrics.sectionHeaderTopPadding)
                .padding(.bottom, 2)

            Text("Press the command layer shortcut, then a command key to open a new terminal tab.")
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, SettingsMetrics.horizontalPadding)
                .padding(.bottom, SettingsMetrics.sectionHeaderBottomPadding)

            CommandPrefixRow(
                combo: commandStore.prefixCombo,
                isRecording: recordingCommandPrefix,
                conflictMessage: commandPrefixConflictWarning,
                onStartRecording: {
                    discardPendingCommandShortcut()
                    recordingAction = nil
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
                    commandStore.resetPrefixCombo()
                    recordingCommandPrefix = false
                    commandPrefixConflictWarning = nil
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
                        recordingAction = nil
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

    private func handleRecord(action: ShortcutAction, combo: KeyCombo) {
        if let existing = store.conflictingAction(for: combo, excluding: action) {
            conflictWarning = (action: action, existing: existing)
            return
        }
        store.updateBinding(action: action, combo: combo)
        recordingAction = nil
        conflictWarning = nil
    }

    private func handleRecord(prefixCombo combo: KeyCombo) {
        commandStore.updatePrefixCombo(combo)
        recordingCommandPrefix = false
        commandPrefixConflictWarning = nil
    }

    private func handleRecord(shortcutID: UUID, combo: KeyCombo) {
        if let existing = commandStore.conflictingShortcut(for: combo, excluding: shortcutID) {
            commandConflictWarning = (id: shortcutID, message: "Conflicts with \"\(existing.displayName)\"")
            return
        }
        guard var shortcut = commandStore.shortcuts.first(where: { $0.id == shortcutID }) else { return }
        shortcut.combo = combo
        commandStore.updateShortcut(shortcut)
        if pendingCommandShortcutID == shortcutID {
            pendingCommandShortcutID = nil
        }
        recordingCommandShortcutID = nil
        commandConflictWarning = nil
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

private struct ShortcutRow: View {
    let action: ShortcutAction
    let combo: KeyCombo
    let isRecording: Bool
    let conflictAction: ShortcutAction?
    let onStartRecording: () -> Void
    let onRecord: (KeyCombo) -> Void
    let onCancel: () -> Void
    let onReset: () -> Void
    @State private var hovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(action.displayName)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .frame(maxWidth: .infinity, alignment: .leading)

                if isRecording {
                    recordingView
                } else {
                    comboDisplay
                }
            }

            if let conflictAction {
                Text("Conflicts with \"\(conflictAction.displayName)\" — press a different shortcut or Esc to cancel")
                    .font(.system(size: 10))
                    .foregroundStyle(.orange)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
        .background(hovered ? Color.primary.opacity(0.04) : .clear)
        .onHover { hovered = $0 }
    }

    private var comboDisplay: some View {
        HStack(spacing: 6) {
            if hovered {
                Button(action: onReset) {
                    Image(systemName: "arrow.counterclockwise")
                        .font(.system(size: 10))
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Reset Shortcut")
            }

            Button(action: onStartRecording) {
                Text(combo.displayString)
                    .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium, design: .rounded))
                    .foregroundStyle(.primary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 5))
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
                .foregroundStyle(.orange)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(.orange.opacity(0.12), in: RoundedRectangle(cornerRadius: 5))
        }
    }
}

private struct CommandPrefixRow: View {
    let combo: KeyCombo
    let isRecording: Bool
    let conflictMessage: String?
    let onStartRecording: () -> Void
    let onRecord: (KeyCombo) -> Void
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
                    .foregroundStyle(.orange)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
        .background(hovered ? Color.primary.opacity(0.04) : .clear)
        .onHover { hovered = $0 }
    }

    private var comboDisplay: some View {
        HStack(spacing: 6) {
            if hovered {
                Button(action: onReset) {
                    Image(systemName: "arrow.counterclockwise")
                        .font(.system(size: 10))
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Reset Shortcut")
            }

            Button(action: onStartRecording) {
                Text(combo.displayString)
                    .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium, design: .rounded))
                    .foregroundStyle(.primary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 5))
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
                .foregroundStyle(.orange)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(.orange.opacity(0.12), in: RoundedRectangle(cornerRadius: 5))
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
    let onRecord: (KeyCombo) -> Void
    let onCancel: () -> Void
    let onDelete: () -> Void
    @State private var hovered = false
    @State private var deleteButtonHovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                TextField("Name", text: $shortcut.name)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .frame(width: 120)

                TextField("Command", text: $shortcut.command)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                    .frame(maxWidth: .infinity)

                if isRecording {
                    recordingView
                } else {
                    comboDisplay
                }
            }

            if let conflictMessage {
                Text("\(conflictMessage) — press a different shortcut or Esc to cancel")
                    .font(.system(size: 10))
                    .foregroundStyle(.orange)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
        .background(hovered ? Color.primary.opacity(0.04) : .clear)
        .onHover { hovered = $0 }
    }

    private var comboDisplay: some View {
        HStack(spacing: 6) {
            Button(action: onStartRecording) {
                Text("\(prefixCombo.displayString) \(shortcut.combo.displayString)")
                    .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium, design: .rounded))
                    .foregroundStyle(.primary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)

            Button(action: onDelete) {
                Image(systemName: "trash")
                    .font(.system(size: 10))
                    .foregroundStyle(
                        deleteButtonHovered ? AnyShapeStyle(MuxyTheme.diffRemoveFg) : AnyShapeStyle(.secondary)
                    )
                    .frame(width: Metrics.deleteButtonSize, height: Metrics.deleteButtonSize)
            }
            .buttonStyle(.plain)
            .background(
                deleteButtonHovered ? AnyShapeStyle(MuxyTheme.diffRemoveBg) : AnyShapeStyle(.quaternary),
                in: RoundedRectangle(cornerRadius: 5)
            )
            .onHover { isHovering in
                deleteButtonHovered = isHovering
            }
            .accessibilityLabel("Delete Command Shortcut")
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
                .foregroundStyle(.orange)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(.orange.opacity(0.12), in: RoundedRectangle(cornerRadius: 5))
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
                    .foregroundStyle(isConfirming ? MuxyTheme.diffRemoveFg : .secondary)
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
        isConfirming ? AnyShapeStyle(MuxyTheme.diffRemoveBg) : AnyShapeStyle(.quaternary)
    }
}
