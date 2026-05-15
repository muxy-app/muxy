import AppKit
import SwiftUI

struct ProjectPickerOverlay: View {
    let projectPaths: [String]
    let onConfirm: (String, Bool) -> Bool
    let onChooseFinder: () -> Void
    let onDismiss: () -> Void

    @State private var input = ProjectPickerStartingDirectory.displayPath
    @State private var rows: [String] = []
    @State private var highlightedIndex: Int?
    @State private var readFailure: ProjectPickerDirectoryReadFailure?

    private var navigator: ProjectPickerNavigator {
        ProjectPickerNavigator(input: input, homeDirectory: ProjectPickerStartingDirectory.path)
    }

    private var highlightedRow: String? {
        guard let highlightedIndex, highlightedIndex < rows.count else { return nil }
        return rows[highlightedIndex]
    }

    private var standardizedTypedPath: String {
        URL(fileURLWithPath: navigator.confirmPath).standardizedFileURL.path
    }

    private var typedPathExists: Bool {
        var isDirectory: ObjCBool = false
        return FileManager.default.fileExists(atPath: standardizedTypedPath, isDirectory: &isDirectory) && isDirectory.boolValue
    }

    private var isExistingProject: Bool {
        projectPaths.contains(standardizedTypedPath)
    }

    private var actionTitle: String {
        if isExistingProject { return "Open" }
        return typedPathExists ? "Add" : "Create & Add"
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
            .frame(width: UIMetrics.scaled(560), height: UIMetrics.scaled(460))
            .background(MuxyTheme.bg)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusXL))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusXL).stroke(MuxyTheme.border, lineWidth: 1))
            .shadow(color: .black.opacity(0.4), radius: UIMetrics.scaled(20), y: UIMetrics.scaled(8))
            .padding(.top, UIMetrics.scaled(60))
            .frame(maxHeight: .infinity, alignment: .top)
            .accessibilityAddTraits(.isModal)
        }
        .onAppear { reloadDirectory() }
        .onChange(of: input) { reloadDirectory() }
    }

    private var pathBar: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Button(action: goUp) {
                Text("←")
                    .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold, design: .monospaced))
            }
            .buttonStyle(.plain)
            .foregroundStyle(MuxyTheme.fgMuted)

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

    private var directoryContent: some View {
        VStack(spacing: 0) {
            HStack {
                Text("DIRECTORIES")
                    .font(.system(size: UIMetrics.fontXS, weight: .bold))
                    .tracking(0.6)
                    .foregroundStyle(MuxyTheme.fgDim)
                Spacer()
            }
            .padding(.horizontal, UIMetrics.spacing5)
            .padding(.vertical, UIMetrics.spacing3)

            if let readFailure {
                permissionState(readFailure)
            } else {
                directoryRows
            }
        }
        .frame(maxHeight: .infinity)
    }

    private var directoryRows: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: true) {
                LazyVStack(spacing: 0) {
                    ForEach(Array(rows.enumerated()), id: \.element) { index, row in
                        HStack(spacing: UIMetrics.spacing3) {
                            Image(systemName: row == ".." ? "arrow.turn.up.left" : "folder")
                                .foregroundStyle(MuxyTheme.fgMuted)
                            Text(row)
                                .font(.system(size: UIMetrics.fontBody, design: .monospaced))
                            Spacer()
                        }
                        .padding(.horizontal, UIMetrics.spacing5)
                        .padding(.vertical, UIMetrics.spacing3)
                        .background(index == highlightedIndex ? MuxyTheme.hover : .clear)
                        .contentShape(Rectangle())
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

    private func permissionState(_ failure: ProjectPickerDirectoryReadFailure) -> some View {
        VStack(spacing: UIMetrics.spacing4) {
            Text(errorMessage(for: failure.kind))
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
            HStack(spacing: UIMetrics.spacing4) {
                Button("Retry", action: reloadDirectory)
                if failure.kind == .permissionDenied {
                    Button("Open System Settings", action: openFilesAndFoldersSettings)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var footer: some View {
        HStack(spacing: UIMetrics.spacing5) {
            HStack(spacing: UIMetrics.spacing4) {
                ProjectPickerFooterHint(keys: "↑↓", label: "Navigate")
                ProjectPickerFooterHint(keys: "Tab", label: "Complete")
                ProjectPickerFooterHint(keys: "Enter", label: "Open")
                ProjectPickerFooterHint(keys: "⌘Enter", label: actionTitle)
                ProjectPickerFooterHint(keys: "Esc", label: "Close")
            }
            Spacer(minLength: UIMetrics.spacing6)
            Button("Choose with Finder…") {
                onChooseFinder()
                onDismiss()
            }
            .buttonStyle(.plain)
            .foregroundStyle(MuxyTheme.accent)
            Button(actionTitle, action: confirmTypedPath)
                .buttonStyle(.plain)
                .foregroundStyle(MuxyTheme.accent)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing4)
    }

    private func reloadDirectory() {
        do {
            let urls = try FileManager.default.contentsOfDirectory(
                at: URL(fileURLWithPath: navigator.directoryPath),
                includingPropertiesForKeys: [.isDirectoryKey],
                options: []
            )
            let names = urls.compactMap { url -> String? in
                guard (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true else { return nil }
                return url.lastPathComponent
            }
            readFailure = nil
            rows = navigator.directoryRows(from: names)
            highlightedIndex = nil
        } catch {
            readFailure = ProjectPickerDirectoryReadFailure(error: error)
            rows = []
            highlightedIndex = nil
        }
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
        let shouldCreate = !typedPathExists
        guard !shouldCreate || confirmCreateDirectory() else { return }
        if onConfirm(standardizedTypedPath, shouldCreate) {
            onDismiss()
        }
    }

    private func descend(_ row: String) {
        if row == ".." {
            goUp()
            return
        }
        input = navigator.completedPath(highlightedRow: row)
    }

    private func goUp() {
        input = navigator.parentDisplayPath
    }

    private func confirmCreateDirectory() -> Bool {
        let alert = NSAlert()
        alert.messageText = "Create Project Folder?"
        alert.informativeText = "Muxy will create \"\(standardizedTypedPath)\" and add it as a project."
        alert.addButton(withTitle: "Create & Add")
        alert.addButton(withTitle: "Cancel")
        return alert.runModal() == .alertFirstButtonReturn
    }

    private func errorMessage(for kind: ProjectPickerDirectoryReadFailureKind) -> String {
        switch kind {
        case .permissionDenied:
            "Muxy needs permission to read this folder"
        case .notFound:
            "No such folder"
        case .ioFailure:
            "Could not read this folder"
        }
    }

    private func openFilesAndFoldersSettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_FilesAndFolders") else { return }
        NSWorkspace.shared.open(url)
    }
}

private struct ProjectPickerFooterHint: View {
    let keys: String
    let label: String

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Text(keys)
                .font(.system(size: UIMetrics.fontXS, weight: .semibold, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.horizontal, UIMetrics.scaled(5))
                .padding(.vertical, UIMetrics.scaled(2))
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
            Text(label)
                .font(.system(size: UIMetrics.fontXS))
                .foregroundStyle(MuxyTheme.fgDim)
        }
    }
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
        field.onEscape = onEscape
        field.onCommandSubmit = onCommandSubmit
        DispatchQueue.main.async { field.window?.makeFirstResponder(field) }
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
            if commandSelector == #selector(NSResponder.moveLeft(_:)) {
                parent.onGoUp()
                return true
            }
            if commandSelector == #selector(NSResponder.deleteBackward(_:)), textView.string.trimmingCharacters(in: .whitespaces).isEmpty {
                parent.onGoUp()
                return true
            }
            return false
        }
    }
}

private final class ProjectPickerNSTextField: NSTextField {
    var onEscape: (() -> Void)?
    var onCommandSubmit: (() -> Void)?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.keyCode == 53 {
            onEscape?()
            return true
        }
        if event.keyCode == 36, event.modifierFlags.contains(.command) {
            onCommandSubmit?()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
}
