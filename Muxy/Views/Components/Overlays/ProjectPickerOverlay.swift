import AppKit
import SwiftUI

struct ProjectPickerOverlay: View {
    let projectPaths: [String]
    let onConfirm: (String, Bool) -> ProjectOpenConfirmationResult
    let onChooseFinder: () -> Void
    let onDismiss: () -> Void
    let isRemote: Bool

    @State private var workflow: ProjectPickerWorkflow

    init(
        projectPaths: [String],
        context: WorkspaceContext = .local,
        onConfirm: @escaping (String, Bool) -> ProjectOpenConfirmationResult,
        onChooseFinder: @escaping () -> Void,
        onDismiss: @escaping () -> Void
    ) {
        self.projectPaths = projectPaths
        self.onConfirm = onConfirm
        self.onChooseFinder = onChooseFinder
        self.onDismiss = onDismiss
        isRemote = context.isRemote
        _workflow = State(initialValue: ProjectPickerWorkflow(projectPaths: projectPaths, context: context))
    }

    private var inputBinding: Binding<String> {
        Binding(
            get: { workflow.session.input },
            set: { execute(workflow.setInput($0)) }
        )
    }

    var body: some View {
        ZStack {
            Color.black.opacity(0.3)
                .ignoresSafeArea()
                .onTapGesture { handleCommand(.dismiss) }

            OverlayPanel(width: UIMetrics.scaled(640), height: UIMetrics.scaled(460)) {
                VStack(spacing: 0) {
                    pathBar
                    Divider().overlay(MuxyTheme.border)
                    directoryContent
                    Divider().overlay(MuxyTheme.border)
                    footer
                }
            }
        }
        .onAppear { workflow.appear() }
        .onChange(of: projectPaths) { workflow.setProjectPaths($1) }
        .onDisappear { workflow.cancel() }
    }

    private var pathBar: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)

            ZStack(alignment: .leading) {
                if workflow.session.input.isEmpty {
                    Text("Search folders or enter a path…")
                        .font(.system(size: UIMetrics.fontEmphasis, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .allowsHitTesting(false)
                }
                ghostTextPreview
                ProjectPickerPathField(
                    text: inputBinding,
                    onCommand: handleCommand
                )
            }

            topRightActionMenu
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing5)
    }

    private var topRightActionMenu: some View {
        let defaultLocationNeedsFix = !ProjectPickerDefaultLocation.state.isReady

        return HStack(spacing: 0) {
            Button(
                action: { handleCommand(.confirmTypedPath) },
                label: {
                    HStack(spacing: UIMetrics.spacing2) {
                        Image(systemName: "plus")
                            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                        Text(workflow.session.topRightActionTitle)
                            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    }
                    .padding(.leading, UIMetrics.spacing3)
                    .padding(.trailing, UIMetrics.spacing4)
                    .padding(.vertical, UIMetrics.spacing2)
                    .contentShape(Rectangle())
                }
            )
            .buttonStyle(.plain)
            .disabled(workflow.session.confirmationPath == nil)

            if !isRemote {
                Rectangle()
                    .fill(MuxyTheme.border)
                    .frame(width: 1)

                Menu {
                    Button {
                        chooseWithFinder()
                    } label: {
                        Label("Choose in Finder", systemImage: "folder")
                    }
                    Button {
                        editDefaultLocation()
                    } label: {
                        if defaultLocationNeedsFix {
                            Label("Fix Search Location", systemImage: "exclamationmark.triangle.fill")
                                .foregroundStyle(.orange)
                        } else {
                            Label("Edit Search Location", systemImage: "gearshape")
                        }
                    }
                } label: {
                    Image(systemName: "chevron.down")
                        .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                        .padding(.horizontal, UIMetrics.spacing3)
                        .padding(.vertical, UIMetrics.spacing2)
                        .contentShape(Rectangle())
                }
                .menuStyle(.button)
                .menuIndicator(.hidden)
                .buttonStyle(.plain)
            }
        }
        .foregroundStyle(MuxyTheme.fg)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusMD).stroke(MuxyTheme.border, lineWidth: 1))
        .fixedSize()
    }

    private var ghostTextPreview: some View {
        HStack(spacing: 0) {
            Text(workflow.session.input)
                .foregroundStyle(.clear)
            Text(workflow.session.ghostText)
                .foregroundStyle(MuxyTheme.fgDim.opacity(0.65))
        }
        .font(.system(size: UIMetrics.fontEmphasis, design: .monospaced))
        .lineLimit(1)
        .allowsHitTesting(false)
    }

    private var directoryContent: some View {
        Group {
            if workflow.session.directoryLoadState.isLoading {
                loadingProjectContent
            } else if workflow.session.showsUnavailableProjectState {
                unavailableProjectContent
            } else if workflow.session.inputMode == .folderSearch {
                folderSearchRows
            } else {
                directoryRows
            }
        }
        .frame(maxHeight: .infinity)
    }

    private var loadingProjectContent: some View {
        VStack {
            Spacer()
            if workflow.session.directoryLoadState.showsMessage {
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
            if workflow.session.hasParentRow {
                parentDirectoryRow
            }
            unavailableProjectMessage
        }
    }

    private var parentDirectoryRow: some View {
        ProjectPickerDirectoryRowView(
            row: .parent,
            isHighlighted: workflow.session.highlightedIndex == 0
        )
        .onTapGesture { execute(workflow.activate(row: .parent)) }
    }

    private var directoryRows: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: true) {
                LazyVStack(spacing: 0) {
                    ForEach(Array(workflow.session.rows.enumerated()), id: \.element) { index, row in
                        ProjectPickerDirectoryRowView(
                            row: row,
                            isHighlighted: index == workflow.session.highlightedIndex
                        )
                        .onTapGesture {
                            workflow.selectRow(at: index)
                            execute(workflow.activate(row: row))
                        }
                        .id(row)
                    }
                }
            }
            .onChange(of: workflow.session.highlightedIndex) { _, newIndex in
                guard let newIndex, newIndex < workflow.session.rows.count else { return }
                proxy.scrollTo(workflow.session.rows[newIndex], anchor: nil)
            }
        }
    }

    private var folderSearchRows: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: true) {
                LazyVStack(spacing: 0) {
                    ForEach(Array(workflow.session.searchResults.enumerated()), id: \.element.path) { index, result in
                        ProjectPickerFolderSearchRowView(
                            result: result,
                            isHighlighted: index == workflow.session.highlightedIndex
                        )
                        .onTapGesture {
                            workflow.selectRow(at: index)
                            execute(workflow.activate(searchResult: result))
                        }
                        .id(result.path)
                    }
                    if let searchResultsNotice {
                        Text(searchResultsNotice)
                            .font(.system(size: UIMetrics.fontFootnote))
                            .foregroundStyle(MuxyTheme.fgDim)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, UIMetrics.spacing3)
                    }
                }
            }
            .onChange(of: workflow.session.highlightedIndex) { _, newIndex in
                guard let newIndex, newIndex < workflow.session.searchResults.count else { return }
                proxy.scrollTo(workflow.session.searchResults[newIndex].path, anchor: nil)
            }
        }
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
        HStack(spacing: UIMetrics.scaled(18)) {
            ForEach(ProjectPickerFooterShortcut.ordered(
                inputMode: workflow.session.inputMode,
                actionTitle: workflow.session.inputMode == .folderSearch
                    ? workflow.session.actionTitle
                    : workflow.session.topRightActionTitle
            ), id: \.self) { shortcut in
                ProjectPickerShortcutHint(shortcut: shortcut)
            }
        }
        .frame(maxWidth: .infinity, alignment: .center)
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing4)
    }

    private func chooseWithFinder() {
        execute(workflow.chooseWithFinder())
    }

    private func editDefaultLocation() {
        execute(workflow.editDefaultLocation())
    }

    private func handleCommand(_ command: ProjectPickerCommand) {
        execute(workflow.handle(command))
    }

    private func execute(_ requests: [ProjectPickerWorkflowRequest]) {
        for request in requests {
            executeSingle(request)
        }
    }

    private func executeSingle(_ request: ProjectPickerWorkflowRequest) {
        switch request {
        case let .askCreateDirectory(path):
            execute(workflow.handleCreateDirectoryDecision(path: path, accepted: confirmCreateDirectory(path: path)))
        case let .confirmProjectPath(path, createIfMissing):
            let result = onConfirm(path, createIfMissing)
            execute(workflow.handleProjectPathConfirmationResult(result, path: path))
        case .chooseFinder:
            DispatchQueue.main.async { onChooseFinder() }
        case .openSettingsFocusedOnDefaultLocation:
            DispatchQueue.main.async {
                SettingsFocusCoordinator.shared.request(.projectPickerDefaultLocation)
                NotificationCenter.default.post(name: .openSettingsModal, object: nil)
            }
        case .dismiss:
            onDismiss()
        case let .showFailure(presentation):
            showConfirmationFailureAlert(presentation)
        }
    }

    private func confirmCreateDirectory(path: String) -> Bool {
        let alert = NSAlert()
        alert.messageText = "Create Project Folder?"
        alert.informativeText = "Muxy will create \"\(path)\" and add it as a project."
        alert.addButton(withTitle: "Create & Add")
        alert.addButton(withTitle: "Cancel")
        return alert.runModal() == .alertFirstButtonReturn
    }

    private func showConfirmationFailureAlert(_ presentation: ProjectPickerConfirmationFailurePresentation) {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = presentation.title
        alert.informativeText = presentation.message
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    private var unavailableProjectTitle: String {
        guard workflow.session.inputMode == .folderSearch else { return "No project folders found" }
        if workflow.session.directoryLoadState.readFailed { return "Folder search unavailable" }
        return workflow.session.searchQuery.isEmpty ? "Find a project folder" : "No matching folders"
    }

    private var unavailableProjectDescription: String {
        guard workflow.session.inputMode == .folderSearch else {
            return "Use the action above to open or create this project, go up, or choose with Finder."
        }
        let root = workflow.session.pathService.abbreviatedDirectoryDisplayPath(workflow.session.searchRootPath)
        if workflow.session.directoryLoadState.readFailed {
            return "Check the folder search location, enter a path, or choose with Finder."
        }
        if workflow.session.searchQuery.isEmpty {
            return "Type a folder name to search inside \(root), or enter a path."
        }
        if workflow.session.folderSearchIsTruncated {
            return "The folder index reached its safety limit. Refine the search location or enter a path."
        }
        return "No folders in \(root) match “\(workflow.session.searchQuery)”. You can still enter a path."
    }

    private var searchResultsNotice: String? {
        if workflow.session.folderSearchHasMoreResults {
            return "More matches available — keep typing to narrow the results."
        }
        if workflow.session.folderSearchIsTruncated {
            return "Folder index safety limit reached — some paths may require direct entry."
        }
        return nil
    }
}

private struct ProjectPickerFolderSearchRowView: View {
    let result: ProjectPickerFolderSearchResult
    let isHighlighted: Bool
    @State private var hovered = false

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "folder")
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(16), height: UIMetrics.scaled(16))

            VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                Text(result.name)
                    .font(.system(size: UIMetrics.fontBody, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fg)
                Text(result.displayPath)
                    .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            Spacer()
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing3)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
    }
}

private struct ProjectPickerDirectoryRowView: View {
    let row: ProjectPickerDirectoryItem
    let isHighlighted: Bool
    @State private var hovered = false

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            icon
                .frame(width: UIMetrics.scaled(16), height: UIMetrics.scaled(16))
            Text(row.name)
                .font(.system(size: UIMetrics.fontBody, design: .monospaced))
            Spacer()
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing3)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
    }

    @ViewBuilder
    private var icon: some View {
        if row.isParent {
            Image(systemName: "arrow.turn.up.left")
                .foregroundStyle(MuxyTheme.fgMuted)
        } else if row.isDirectorySymlink {
            ZStack(alignment: .bottomTrailing) {
                Image(systemName: "folder")
                    .foregroundStyle(MuxyTheme.fgMuted)
                Image(systemName: "arrow.up.forward")
                    .font(.system(size: UIMetrics.scaled(7), weight: .bold))
                    .foregroundStyle(MuxyTheme.fg)
                    .padding(1)
                    .background(MuxyTheme.surface, in: Circle())
                    .offset(x: UIMetrics.scaled(3), y: UIMetrics.scaled(2))
            }
        } else {
            Image(systemName: "folder")
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }
}
