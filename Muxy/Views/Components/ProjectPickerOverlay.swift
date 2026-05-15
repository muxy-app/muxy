import AppKit
import Carbon.HIToolbox
import SwiftUI

struct ProjectPickerOverlay: View {
    let projectPaths: [String]
    let onConfirm: (String, Bool) -> ProjectOpenConfirmationResult
    let onChooseFinder: () -> Void
    let onDismiss: () -> Void

    @AppStorage(ProjectPickerDefaultDirectory.storageKey) private var projectPickerDefaultDirectoryPath = ""
    @State private var input = ""
    @State private var rows: [String] = []
    @State private var highlightedIndex: Int?
    @State private var directoryLoadState = ProjectPickerDirectoryLoadState.loading(showsMessage: false)
    @State private var directoryLoadID = UUID()
    @State private var reloadTask: Task<Void, Never>?
    @State private var loadingMessageTask: Task<Void, Never>?

    private var navigator: ProjectPickerNavigator {
        ProjectPickerNavigator(input: input, homeDirectory: NSHomeDirectory())
    }

    private var highlightedRow: String? {
        guard let highlightedIndex, highlightedIndex < rows.count else { return nil }
        return rows[highlightedIndex]
    }

    private var standardizedTypedPath: String {
        URL(fileURLWithPath: navigator.confirmPath).standardizedFileURL.path
    }

    private var typedPathState: ProjectPickerTypedPathState {
        var isDirectory = ObjCBool(false)
        guard FileManager.default.fileExists(atPath: standardizedTypedPath, isDirectory: &isDirectory) else {
            return .missing
        }
        return isDirectory.boolValue ? .directory : .notDirectory
    }

    private var isExistingProject: Bool {
        projectPaths.contains(standardizedTypedPath)
    }

    private var actionTitle: String {
        if isExistingProject { return "Open" }
        return typedPathState == .missing ? "Create & Add" : "Add"
    }

    private var ghostText: String {
        guard let highlightedRow, !isParentDirectoryRow(highlightedRow) else { return "" }
        let completedPath = navigator.completedPath(highlightedRow: highlightedRow)
        guard completedPath.hasPrefix(input) else { return "" }
        return String(completedPath.dropFirst(input.count))
    }

    var body: some View {
        ZStack {
            Color.black.opacity(0.3)
                .ignoresSafeArea()
                .onTapGesture { onDismiss() }

            VStack(spacing: 0) {
                pathBar
                Divider().overlay(MuxyTheme.border)
                directoryContent
                Divider().overlay(MuxyTheme.border)
                footer
            }
            .frame(width: UIMetrics.scaled(640), height: UIMetrics.scaled(460))
            .background(MuxyTheme.bg)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusXL))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusXL).stroke(MuxyTheme.border, lineWidth: 1))
            .shadow(color: .black.opacity(0.4), radius: UIMetrics.scaled(20), y: UIMetrics.scaled(8))
            .padding(.top, UIMetrics.scaled(60))
            .frame(maxHeight: .infinity, alignment: .top)
            .accessibilityAddTraits(.isModal)
        }
        .onAppear { resetInputToDefaultDirectory() }
        .onChange(of: input) { scheduleDirectoryReload() }
        .onChange(of: projectPickerDefaultDirectoryPath) { resetInputToDefaultDirectory() }
        .onDisappear { cancelDirectoryReload() }
    }

    private var pathBar: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)

            ZStack(alignment: .leading) {
                ghostTextPreview
                ProjectPickerPathField(
                    text: $input,
                    onSubmit: { confirmDefault() },
                    onCommandSubmit: { confirmTypedPath() },
                    onEscape: { onDismiss() },
                    onArrowUp: { moveHighlight(-1) },
                    onArrowDown: { moveHighlight(1) },
                    onTab: { completeHighlighted() },
                    onGoUp: { goUp() }
                )
            }

            Button(action: confirmTypedPath) {
                Text(actionTitle)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
            }
            .buttonStyle(.plain)
            .foregroundStyle(MuxyTheme.accent)
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing5)
    }

    private var ghostTextPreview: some View {
        HStack(spacing: 0) {
            Text(input)
                .foregroundStyle(.clear)
            Text(ghostText)
                .foregroundStyle(MuxyTheme.fgDim.opacity(0.65))
        }
        .font(.system(size: UIMetrics.fontEmphasis, design: .monospaced))
        .lineLimit(1)
        .allowsHitTesting(false)
    }

    private var directoryContent: some View {
        Group {
            if directoryLoadState.isLoading {
                loadingProjectContent
            } else if showsUnavailableProjectState {
                unavailableProjectContent
            } else {
                directoryRows
            }
        }
        .frame(maxHeight: .infinity)
    }

    private var showsUnavailableProjectState: Bool {
        directoryLoadState.readFailed || projectRows.isEmpty
    }

    private var projectRows: [String] {
        rows.filter { !isParentDirectoryRow($0) }
    }

    private var hasParentRow: Bool {
        rows.contains { isParentDirectoryRow($0) }
    }

    private var loadingProjectContent: some View {
        VStack {
            Spacer()
            if directoryLoadState.showsMessage {
                Text("Loading…")
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var unavailableProjectContent: some View {
        VStack(spacing: 0) {
            if hasParentRow {
                parentDirectoryRow
            }
            unavailableProjectMessage
        }
    }

    private var parentDirectoryRow: some View {
        ProjectPickerDirectoryRow(
            row: ProjectPickerNavigator.parentDirectoryRow,
            isParent: true,
            isHighlighted: highlightedIndex == 0
        )
        .onTapGesture { descend(ProjectPickerNavigator.parentDirectoryRow) }
    }

    private var directoryRows: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: true) {
                LazyVStack(spacing: 0) {
                    ForEach(Array(rows.enumerated()), id: \.element) { index, row in
                        ProjectPickerDirectoryRow(
                            row: row,
                            isParent: isParentDirectoryRow(row),
                            isHighlighted: index == highlightedIndex
                        )
                        .onTapGesture {
                            highlightedIndex = index
                            descend(row)
                        }
                        .id(row)
                    }
                }
            }
            .onChange(of: highlightedIndex) { _, newIndex in
                guard let newIndex, newIndex < rows.count else { return }
                proxy.scrollTo(rows[newIndex], anchor: nil)
            }
        }
    }

    private func isParentDirectoryRow(_ row: String) -> Bool {
        row == ProjectPickerNavigator.parentDirectoryRow
    }

    private var unavailableProjectMessage: some View {
        VStack(spacing: UIMetrics.spacing4) {
            Text(unavailableProjectTitle)
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text(unavailableProjectDescription)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgDim)
                .multilineTextAlignment(.center)
                .frame(maxWidth: UIMetrics.scaled(420))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var footer: some View {
        HStack(spacing: UIMetrics.spacing5) {
            HStack(spacing: UIMetrics.spacing4) {
                ProjectPickerShortcutHint(keycap: .navigate, label: "Navigate")
                ProjectPickerShortcutHint(keycap: .returnKey, label: "Open")
                ProjectPickerShortcutHint(keycap: .commandReturn, label: actionTitle)
                ProjectPickerShortcutHint(keycap: .escape, label: "Close")
            }
            Spacer(minLength: UIMetrics.spacing6)
            Button("Choose with Finder…") {
                onDismiss()
                DispatchQueue.main.async { onChooseFinder() }
            }
            .buttonStyle(.plain)
            .foregroundStyle(MuxyTheme.accent)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing4)
    }

    private func resetInputToDefaultDirectory() {
        input = ProjectPickerDefaultDirectory.displayPath(storedCustomPath: projectPickerDefaultDirectoryPath)
        scheduleDirectoryReload()
    }

    private func scheduleDirectoryReload() {
        let navigator = navigator
        let loadID = UUID()
        directoryLoadID = loadID
        cancelDirectoryReload()
        directoryLoadState = .loading(showsMessage: false)
        loadingMessageTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard directoryLoadID == loadID, directoryLoadState.isLoading else { return }
                directoryLoadState = .loading(showsMessage: true)
            }
        }
        reloadTask = Task {
            try? await Task.sleep(for: .milliseconds(100))
            guard !Task.isCancelled else { return }
            let snapshot = await Task.detached(priority: .userInitiated) {
                ProjectPickerDirectorySnapshot.load(navigator: navigator)
            }.value
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard directoryLoadID == loadID else { return }
                applyDirectorySnapshot(snapshot)
            }
        }
    }

    private func cancelDirectoryReload() {
        reloadTask?.cancel()
        loadingMessageTask?.cancel()
    }

    private func applyDirectorySnapshot(_ snapshot: ProjectPickerDirectorySnapshot) {
        loadingMessageTask?.cancel()
        directoryLoadState = snapshot.readFailed ? .failed : .loaded
        rows = snapshot.rows
        highlightedIndex = initialHighlightedIndex(for: snapshot.rows)
    }

    private func initialHighlightedIndex(for rows: [String]) -> Int? {
        guard !rows.isEmpty else { return nil }
        guard rows.first.map(isParentDirectoryRow) == true, rows.count > 1 else { return 0 }
        return 1
    }

    private func moveHighlight(_ delta: Int) {
        guard !rows.isEmpty else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : rows.count - 1
            return
        }
        highlightedIndex = max(0, min(rows.count - 1, current + delta))
    }

    private func completeHighlighted() {
        guard let highlightedRow else { return }
        input = navigator.completedPath(highlightedRow: highlightedRow)
    }

    private func confirmDefault() {
        guard let highlightedRow else {
            confirmTypedPath()
            return
        }
        descend(highlightedRow)
    }

    private func confirmTypedPath() {
        let shouldCreate = typedPathState == .missing
        guard !shouldCreate || confirmCreateDirectory() else { return }
        let result = onConfirm(standardizedTypedPath, shouldCreate)
        guard !result.didConfirm else {
            onDismiss()
            return
        }
        showConfirmationFailureAlert(result)
    }

    private func descend(_ row: String) {
        if isParentDirectoryRow(row) {
            goUp()
            return
        }
        input = navigator.completedPath(highlightedRow: row)
    }

    private func goUp() {
        let parentPath = navigator.parentDisplayPath
        guard parentPath != input else { return }
        input = parentPath
    }

    private func confirmCreateDirectory() -> Bool {
        let alert = NSAlert()
        alert.messageText = "Create Project Folder?"
        alert.informativeText = "Muxy will create \"\(standardizedTypedPath)\" and add it as a project."
        alert.addButton(withTitle: "Create & Add")
        alert.addButton(withTitle: "Cancel")
        return alert.runModal() == .alertFirstButtonReturn
    }

    private func showConfirmationFailureAlert(_ result: ProjectOpenConfirmationResult) {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = failureTitle(for: result)
        alert.informativeText = failureMessage(for: result)
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    private func failureTitle(for result: ProjectOpenConfirmationResult) -> String {
        switch result {
        case .notDirectory:
            "Path Is Not a Folder"
        case .createFailed:
            "Could Not Create Project Folder"
        default:
            "Could Not Add Project"
        }
    }

    private func failureMessage(for result: ProjectOpenConfirmationResult) -> String {
        switch result {
        case .notDirectory:
            "Muxy can only add folders as projects. Choose a folder or type a new folder path."
        case .missingDirectory:
            "Muxy couldn't find \"\(standardizedTypedPath)\". Check the path and try again."
        case .createFailed:
            "Muxy couldn't create and add \"\(standardizedTypedPath)\". "
                + "Check that you have permission to use this location."
        default:
            "Muxy couldn't add \"\(standardizedTypedPath)\". "
                + "Check that the folder exists and you have permission to use it."
        }
    }

    private var unavailableProjectTitle: String {
        "No project folders found"
    }

    private var unavailableProjectDescription: String {
        "Use the action above to open or create this project, go up, or choose with Finder."
    }
}

private enum ProjectPickerTypedPathState {
    case missing
    case directory
    case notDirectory
}

private enum ProjectPickerDirectoryLoadState: Equatable {
    case loading(showsMessage: Bool)
    case loaded
    case failed

    var isLoading: Bool {
        if case .loading = self { return true }
        return false
    }

    var showsMessage: Bool {
        if case let .loading(showsMessage) = self { return showsMessage }
        return false
    }

    var readFailed: Bool {
        self == .failed
    }
}

struct ProjectPickerDirectorySnapshot {
    let rows: [String]
    let readFailed: Bool

    static func load(navigator: ProjectPickerNavigator) -> ProjectPickerDirectorySnapshot {
        do {
            let urls = try FileManager.default.contentsOfDirectory(
                at: URL(fileURLWithPath: navigator.directoryPath),
                includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey],
                options: []
            )
            let names = urls.compactMap { url -> String? in
                guard isDirectoryOrDirectorySymlink(url) else { return nil }
                return url.lastPathComponent
            }
            return ProjectPickerDirectorySnapshot(rows: navigator.directoryRows(from: names), readFailed: false)
        } catch {
            let rows = navigator.directoryPath == "/" ? [] : [ProjectPickerNavigator.parentDirectoryRow]
            return ProjectPickerDirectorySnapshot(rows: rows, readFailed: true)
        }
    }

    private static func isDirectoryOrDirectorySymlink(_ url: URL) -> Bool {
        let values = try? url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
        if values?.isDirectory == true { return true }
        guard values?.isSymbolicLink == true else { return false }
        var isDirectory = ObjCBool(false)
        return FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory) && isDirectory.boolValue
    }
}

private struct ProjectPickerDirectoryRow: View {
    let row: String
    let isParent: Bool
    let isHighlighted: Bool
    @State private var hovered = false

    private var iconName: String {
        isParent ? "arrow.turn.up.left" : "folder"
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: iconName)
                .foregroundStyle(MuxyTheme.fgMuted)
            Text(row)
                .font(.system(size: UIMetrics.fontBody, design: .monospaced))
            Spacer()
            if isParent {
                ProjectPickerShortcutHint(keycap: .optionDelete, label: "Go back")
            }
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing3)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
    }
}

private struct ProjectPickerShortcutHint: View {
    let keycap: ProjectPickerShortcutKeycap
    let label: String

    var body: some View {
        HStack(spacing: UIMetrics.scaled(4)) {
            HStack(spacing: UIMetrics.scaled(3)) {
                ForEach(Array(keycap.parts.enumerated()), id: \.offset) { _, part in
                    keycapPart(part)
                }
            }
            .lineLimit(1)
            .fixedSize(horizontal: true, vertical: false)
            .padding(.horizontal, UIMetrics.scaled(4))
            .padding(.vertical, UIMetrics.scaled(2))
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
            Text(label)
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgDim)
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
        }
        .fixedSize(horizontal: true, vertical: false)
    }

    @ViewBuilder
    private func keycapPart(_ part: ProjectPickerShortcutKeycapPart) -> some View {
        switch part {
        case let .symbol(name):
            Image(systemName: name)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
        case let .text(text):
            Text(text)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }
}

private struct ProjectPickerShortcutKeycap: Hashable {
    let parts: [ProjectPickerShortcutKeycapPart]

    static let navigate = ProjectPickerShortcutKeycap(parts: [.symbol("arrow.up"), .symbol("arrow.down")])
    static let returnKey = ProjectPickerShortcutKeycap(parts: [.text("Return")])
    static let commandReturn = ProjectPickerShortcutKeycap(parts: [.symbol("command"), .text("Return")])
    static let escape = ProjectPickerShortcutKeycap(parts: [.text("Esc")])
    static let optionDelete = ProjectPickerShortcutKeycap(parts: [.symbol("option"), .symbol("delete.left")])
}

private enum ProjectPickerShortcutKeycapPart: Hashable {
    case symbol(String)
    case text(String)
}

private struct ProjectPickerPathField: NSViewRepresentable {
    @Binding var text: String
    let onSubmit: () -> Void
    let onCommandSubmit: () -> Void
    let onEscape: () -> Void
    let onArrowUp: () -> Void
    let onArrowDown: () -> Void
    let onTab: () -> Void
    let onGoUp: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeNSView(context: Context) -> NSTextField {
        let field = ProjectPickerNSTextField()
        field.delegate = context.coordinator
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .monospacedSystemFont(ofSize: UIMetrics.fontEmphasis, weight: .regular)
        field.textColor = NSColor(MuxyTheme.fg)
        field.stringValue = text
        field.onEscape = onEscape
        field.onCommandSubmit = onCommandSubmit
        DispatchQueue.main.async {
            field.window?.makeFirstResponder(field)
            field.moveCursorToEnd()
        }
        return field
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
        context.coordinator.parent = self
        if nsView.stringValue != text {
            nsView.stringValue = text
        }
        if let field = nsView as? ProjectPickerNSTextField {
            field.onEscape = onEscape
            field.onCommandSubmit = onCommandSubmit
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: ProjectPickerPathField

        init(parent: ProjectPickerPathField) {
            self.parent = parent
        }

        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            parent.text = field.stringValue
        }

        func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                parent.onSubmit()
                return true
            }
            if commandSelector == #selector(NSResponder.insertTab(_:)) {
                parent.onTab()
                return true
            }
            if commandSelector == #selector(NSResponder.moveUp(_:)) {
                parent.onArrowUp()
                return true
            }
            if commandSelector == #selector(NSResponder.moveDown(_:)) {
                parent.onArrowDown()
                return true
            }
            if commandSelector == #selector(NSResponder.deleteWordBackward(_:)) {
                parent.onGoUp()
                return true
            }
            if commandSelector == #selector(NSResponder.deleteBackward(_:)), shouldGoUpOnDeleteBackward(textView) {
                parent.onGoUp()
                return true
            }
            return false
        }

        private func shouldGoUpOnDeleteBackward(_ textView: NSTextView) -> Bool {
            let selectedRange = textView.selectedRange()
            guard selectedRange.length == 0, selectedRange.location == textView.string.utf16.count else { return false }
            let value = textView.string.trimmingCharacters(in: .whitespacesAndNewlines)
            return value.isEmpty || value == "~" || value.hasSuffix("/")
        }
    }
}

private final class ProjectPickerNSTextField: NSTextField {
    var onEscape: (() -> Void)?
    var onCommandSubmit: (() -> Void)?

    func moveCursorToEnd() {
        guard let editor = currentEditor() else { return }
        editor.selectedRange = NSRange(location: stringValue.utf16.count, length: 0)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.keyCode == kVK_Escape {
            onEscape?()
            return true
        }
        if event.keyCode == kVK_Return, event.modifierFlags.contains(.command) {
            onCommandSubmit?()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
}
