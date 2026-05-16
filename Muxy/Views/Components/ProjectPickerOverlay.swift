import AppKit
import SwiftUI

struct ProjectPickerOverlay: View {
    let projectPaths: [String]
    let onConfirm: (String, Bool) -> ProjectOpenConfirmationResult
    let onChooseFinder: () -> Void
    let onDismiss: () -> Void

    @Environment(\.openSettings) private var openSettings
    @State private var workflow: ProjectPickerWorkflow

    init(
        projectPaths: [String],
        onConfirm: @escaping (String, Bool) -> ProjectOpenConfirmationResult,
        onChooseFinder: @escaping () -> Void,
        onDismiss: @escaping () -> Void
    ) {
        self.projectPaths = projectPaths
        self.onConfirm = onConfirm
        self.onChooseFinder = onChooseFinder
        self.onDismiss = onDismiss
        _workflow = State(initialValue: ProjectPickerWorkflow(projectPaths: projectPaths))
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
                        Label("Fix Default Location", systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(.orange)
                    } else {
                        Label("Edit Default Location", systemImage: "gearshape")
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
            ForEach(ProjectPickerFooterShortcut.ordered(actionTitle: workflow.session.topRightActionTitle), id: \.self) { shortcut in
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
                openSettings()
                SettingsFocusCoordinator.shared.request(.projectPickerDefaultLocation)
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
        "No project folders found"
    }

    private var unavailableProjectDescription: String {
        "Use the action above to open or create this project, go up, or choose with Finder."
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
