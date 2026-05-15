import AppKit
import SwiftUI

struct ProjectPickerOverlay: View {
    let projectPaths: [String]
    let onConfirm: (String, Bool) -> ProjectOpenConfirmationResult
    let onChooseFinder: () -> Void
    let onDismiss: () -> Void

    @Environment(\.openSettings) private var openSettings
    @AppStorage(ProjectPickerDefaultLocation.storageKey) private var projectPickerDefaultLocationPath = ""
    @State private var session = ProjectPickerSession(defaultDisplayPath: "", projectPaths: [])
    @State private var didInitializeInput = false
    @State private var directoryLoadID = UUID()
    @State private var reloadTask: Task<Void, Never>?
    @State private var loadingMessageTask: Task<Void, Never>?

    private var inputBinding: Binding<String> {
        Binding(
            get: { session.input },
            set: { execute(session.setInput($0)) }
        )
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
        .onAppear { initializeInputIfNeeded() }
        .onChange(of: projectPaths) { session.setProjectPaths($1) }
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
        let defaultLocationNeedsFix = ProjectPickerDefaultLocation.status != .ready

        return HStack(spacing: 0) {
            Button(
                action: { handleCommand(.confirmTypedPath) },
                label: {
                    HStack(spacing: UIMetrics.spacing2) {
                        Image(systemName: "plus")
                            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                        Text(session.topRightActionTitle)
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
            Text(session.input)
                .foregroundStyle(.clear)
            Text(session.ghostText)
                .foregroundStyle(MuxyTheme.fgDim.opacity(0.65))
        }
        .font(.system(size: UIMetrics.fontEmphasis, design: .monospaced))
        .lineLimit(1)
        .allowsHitTesting(false)
    }

    private var directoryContent: some View {
        Group {
            if session.directoryLoadState.isLoading {
                loadingProjectContent
            } else if session.showsUnavailableProjectState {
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
            if session.directoryLoadState.showsMessage {
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
            if session.hasParentRow {
                parentDirectoryRow
            }
            unavailableProjectMessage
        }
    }

    private var parentDirectoryRow: some View {
        ProjectPickerDirectoryRow(
            row: ProjectPickerNavigator.parentDirectoryRow,
            isParent: true,
            isHighlighted: session.highlightedIndex == 0
        )
        .onTapGesture { execute(session.activate(row: ProjectPickerNavigator.parentDirectoryRow)) }
    }

    private var directoryRows: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: true) {
                LazyVStack(spacing: 0) {
                    ForEach(Array(session.rows.enumerated()), id: \.element) { index, row in
                        ProjectPickerDirectoryRow(
                            row: row,
                            isParent: session.isParentDirectoryRow(row),
                            isHighlighted: index == session.highlightedIndex
                        )
                        .onTapGesture {
                            session.selectRow(at: index)
                            execute(session.activate(row: row))
                        }
                        .id(row)
                    }
                }
            }
            .onChange(of: session.highlightedIndex) { _, newIndex in
                guard let newIndex, newIndex < session.rows.count else { return }
                proxy.scrollTo(session.rows[newIndex], anchor: nil)
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
            ForEach(ProjectPickerFooterShortcut.ordered(actionTitle: session.topRightActionTitle), id: \.self) { shortcut in
                ProjectPickerShortcutHint(keycap: shortcut.keycap, label: shortcut.label)
            }
        }
        .frame(maxWidth: .infinity, alignment: .center)
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing4)
    }

    private func chooseWithFinder() {
        execute([.dismiss, .chooseFinder])
    }

    private func editDefaultLocation() {
        execute([.dismiss, .openSettingsFocusedOnDefaultLocation])
    }

    private func initializeInputIfNeeded() {
        guard !didInitializeInput else { return }
        didInitializeInput = true
        session = ProjectPickerSession(
            defaultDisplayPath: ProjectPickerDefaultLocation.displayPath(storedCustomPath: projectPickerDefaultLocationPath),
            projectPaths: projectPaths
        )
        execute(.requestDirectoryReload(session.navigator))
    }

    private func handleCommand(_ command: ProjectPickerCommand) {
        execute(session.handle(command))
    }

    private func execute(_ effect: ProjectPickerEffect) {
        execute([effect])
    }

    private func execute(_ effects: [ProjectPickerEffect]) {
        for effect in effects {
            executeSingle(effect)
        }
    }

    private func executeSingle(_ effect: ProjectPickerEffect) {
        switch effect {
        case let .requestDirectoryReload(navigator):
            scheduleDirectoryReload(navigator: navigator)
        case let .confirmCreateDirectory(path):
            guard confirmCreateDirectory(path: path) else { return }
            execute(session.confirmCreateDirectoryAccepted())
        case let .confirmProjectPath(path, createIfMissing):
            let result = onConfirm(path, createIfMissing)
            guard !result.didConfirm else {
                onDismiss()
                return
            }
            showConfirmationFailureAlert(session.confirmationFailurePresentation(for: result))
        case .chooseFinder:
            DispatchQueue.main.async { onChooseFinder() }
        case .openSettingsFocusedOnDefaultLocation:
            DispatchQueue.main.async {
                openSettings()
                SettingsFocusCoordinator.shared.request(.projectPickerDefaultLocation)
            }
        case .dismiss:
            onDismiss()
        }
    }

    private func scheduleDirectoryReload(navigator: ProjectPickerNavigator) {
        let loadID = UUID()
        directoryLoadID = loadID
        cancelDirectoryReload()
        loadingMessageTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard directoryLoadID == loadID else { return }
                session.showLoadingMessage()
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
                apply(snapshot)
            }
        }
    }

    private func cancelDirectoryReload() {
        reloadTask?.cancel()
        loadingMessageTask?.cancel()
    }

    private func apply(_ snapshot: ProjectPickerDirectorySnapshot) {
        loadingMessageTask?.cancel()
        session.applyDirectorySnapshot(snapshot)
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
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing3)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
    }
}
