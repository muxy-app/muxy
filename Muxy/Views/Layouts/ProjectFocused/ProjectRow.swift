import AppKit
import MuxyShared
import SwiftUI

struct ProjectRow: View {
    let project: Project
    let shortcutIndex: Int?
    let isAnyDragging: Bool
    let onSelect: () -> Void
    let onRemove: () -> Void
    let onRename: (String) -> Void
    let onSetLogo: (String?) -> Void
    let onSetIcon: (String?) -> Void
    let onSetIconColor: (String?) -> Void
    let onSetWorktreesEnabled: (Bool) -> Void
    let onSetPinned: (Bool) -> Void

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @State private var showWorktreePopover = false
    @State private var isGitRepo = false
    @State private var isCheckingGitRepo = true
    @State private var showCreateWorktreeSheet = false
    @State private var logoCropImage: IdentifiableImage?
    @State private var isRefreshingWorktrees = false
    @State private var showColorPicker = false
    @State private var showSymbolPicker = false

    private var isActive: Bool {
        appState.activeProjectID == project.id
    }

    private var projectActivity: TerminalActivity? {
        let progressStore = TerminalProgressStore.shared
        let agentStore = AgentStatusStore.shared
        let hasProgress = progressStore.hasActiveProgress(for: project.id)
        return TerminalActivity.resolve(
            progress: hasProgress ? TerminalProgress(kind: .indeterminate, percent: nil) : nil,
            agentStatus: agentStore.status(forProject: project.id),
            unreadCount: NotificationStore.shared.unreadCount(for: project.id),
            completionPending: progressStore.hasCompletionPending(for: project.id)
                || agentStore.hasCompletionPending(forProject: project.id)
        )
    }

    private var worktrees: [Worktree] {
        worktreeStore.list(for: project.id)
    }

    private var hasWorktreeUI: Bool {
        project.worktreesEnabled && (isGitRepo || worktrees.count > 1)
    }

    private var worktreesEnabledBinding: Binding<Bool> {
        Binding(
            get: { project.worktreesEnabled },
            set: { onSetWorktreesEnabled($0) }
        )
    }

    private var displayLetter: String {
        String(project.localizedDisplayName.prefix(1)).uppercased()
    }

    private func hideHome() {
        HomeProjectPreferences.isVisible = false
    }

    var body: some View {
        iconOrBadge
            .help(project.localizedDisplayName)
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusLG))
            .accessibilityElement(children: .combine)
            .accessibilityLabel(project.localizedDisplayName)
            .accessibilityValue(isActive ? L10n.string("Active") : "")
            .accessibilityAddTraits(isActive ? .isSelected : [])
            .accessibilityAddTraits(.isButton)
            .onHover { hovering in
                guard !isAnyDragging else { return }
                hovered = hovering
            }
            .onChange(of: isAnyDragging) { _, dragging in
                if dragging {
                    hovered = false
                }
            }
            .onTapGesture {
                guard !isAnyDragging else { return }
                onSelect()
            }
            .task(id: project.path) {
                guard !project.isHome else {
                    isCheckingGitRepo = false
                    return
                }
                let context = projectGroupStore.workspaceContext(for: project)
                if let cached = GitRepoStatusCache.shared.cachedStatus(for: project.path, context: context) {
                    isGitRepo = cached
                    isCheckingGitRepo = false
                }
                try? await Task.sleep(for: .seconds(2))
                guard !Task.isCancelled else { return }
                let result = await GitWorktreeService.shared.isGitRepository(
                    project.path,
                    context: context
                )
                guard !Task.isCancelled else { return }
                isGitRepo = result
                isCheckingGitRepo = false
                GitRepoStatusCache.shared.update(path: project.path, context: context, isGitRepo: isGitRepo)
            }
            .contextMenu {
                if project.isHome {
                    ProjectContextMenuFooter(
                        path: project.path,
                        workspaceContext: projectGroupStore.workspaceContext(for: project),
                        separatesFromPreviousActions: false
                    ) {
                        Button(L10n.string("Hide Home")) { hideHome() }
                    }
                } else {
                    projectContextMenu
                }
            }
            .popover(isPresented: $showWorktreePopover, arrowEdge: .trailing) {
                WorktreePopover(
                    project: project,
                    isGitRepo: isGitRepo,
                    onDismiss: { showWorktreePopover = false },
                    onRequestCreate: {
                        showWorktreePopover = false
                        showCreateWorktreeSheet = true
                    },
                    onRequestRemove: { worktree in
                        showWorktreePopover = false
                        beginRemove(worktree: worktree)
                    }
                )
                .environment(appState)
                .environment(worktreeStore)
            }
            .sheet(isPresented: $showCreateWorktreeSheet) {
                CreateWorktreeSheet(project: project) { result in
                    showCreateWorktreeSheet = false
                    handleCreateWorktreeResult(result)
                }
            }
            .sheet(item: $logoCropImage) { item in
                LogoCropperSheet(
                    sourceImage: item.image,
                    onConfirm: { cropped in
                        logoCropImage = nil
                        projectStore.setLogo(id: project.id, croppedImage: cropped)
                    },
                    onCancel: { logoCropImage = nil }
                )
            }
            .popover(isPresented: $isRenaming, arrowEdge: .trailing) {
                RenamePopover(
                    text: $renameText,
                    onCommit: { commitRename() },
                    onCancel: { cancelRename() }
                )
            }
            .popover(isPresented: $showColorPicker, arrowEdge: .trailing) {
                ProjectIconColorPicker(selectedID: project.iconColor) { id in
                    onSetIconColor(id)
                    showColorPicker = false
                }
            }
            .popover(isPresented: $showSymbolPicker, arrowEdge: .trailing) {
                SFSymbolPicker(selectedName: project.icon) { name in
                    onSetIcon(name)
                    showSymbolPicker = false
                }
            }
    }

    @ViewBuilder
    private var projectContextMenu: some View {
        if !project.isRemote, !project.isHome {
            Button(
                project.isPinned
                    ? L10n.string("Unpin")
                    : L10n.string("Pin")
            ) {
                onSetPinned(!project.isPinned)
            }
            Divider()
        }
        Button(L10n.string("Set Logo...")) { pickLogoImage() }
        if project.logo != nil {
            Button(L10n.string("Remove Logo")) { onSetLogo(nil) }
        }
        Button(L10n.string("Set Icon...")) { showSymbolPicker = true }
        if project.icon != nil {
            Button(L10n.string("Remove Icon")) { onSetIcon(nil) }
        }
        Button(L10n.string("Set Icon Color...")) { showColorPicker = true }
        if project.iconColor != nil {
            Button(L10n.string("Reset Icon Color")) { onSetIconColor(nil) }
        }
        Divider()
        Button(L10n.string("Rename Project")) { startRename() }
        if isGitRepo {
            Divider()
            Toggle(L10n.string("Worktrees"), isOn: worktreesEnabledBinding)
            if project.worktreesEnabled {
                Button(L10n.string("Refresh Worktrees")) { Task { await refreshWorktrees() } }
                Button(L10n.string("New Worktree…")) { showCreateWorktreeSheet = true }
                if worktrees.count > 1 {
                    Button(L10n.string("Switch Worktree…")) { showWorktreePopover = true }
                }
            }
        } else if isCheckingGitRepo {
            Divider()
            Button(L10n.string("Loading Worktrees…")) {}
                .disabled(true)
        }
        if !projectGroupStore.groups.isEmpty {
            Divider()
            ProjectGroupMembershipMenu(project: project)
        }
        ProjectContextMenuFooter(
            path: project.path,
            workspaceContext: projectGroupStore.workspaceContext(for: project)
        ) {
            Button(L10n.string("Remove Project"), role: .destructive, action: onRemove)
        }
    }

    private var resolvedLogo: NSImage? {
        guard let filename = project.logo,
              let path = ProjectLogoStorage.safeLogoPath(for: filename)
        else { return nil }
        return NSImage(contentsOfFile: path)
    }

    @ViewBuilder
    private var iconOrBadge: some View {
        if let shortcutIndex, let hint = shortcutHint {
            ShortcutIconBadge(number: shortcutIndex, size: UIMetrics.iconXXL, combo: hint)
                .padding(UIMetrics.scaled(3))
        } else {
            projectIcon
        }
    }

    private var projectIcon: some View {
        let logo = resolvedLogo
        return ZStack {
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous)
                .fill(iconBackground(hasLogo: logo != nil))

            if project.isHome {
                Image(systemName: Project.homeIcon)
                    .font(.system(size: UIMetrics.fontTitleLarge, weight: .medium))
                    .foregroundStyle(MuxyTheme.accentForeground)
            } else if let logo {
                Image(nsImage: logo)
                    .resizable()
                    .scaledToFill()
                    .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconXXL)
                    .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous))
            } else if let iconName = project.icon {
                Image(systemName: iconName)
                    .font(.system(size: UIMetrics.fontTitleLarge, weight: .medium))
                    .foregroundStyle(letterForeground)
            } else {
                Text(displayLetter)
                    .font(.system(size: UIMetrics.fontEmphasis, weight: .bold))
                    .foregroundStyle(letterForeground)
            }
        }
        .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconXXL)
        .padding(UIMetrics.scaled(3))
        .overlay(alignment: .topTrailing) {
            if let projectActivity {
                TerminalActivityIndicator(activity: projectActivity)
                    .offset(
                        x: projectActivity.isUnread ? UIMetrics.spacing2 : UIMetrics.spacing1,
                        y: projectActivity.isUnread ? -UIMetrics.spacing2 : -UIMetrics.spacing1
                    )
            }
        }
        .overlay {
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD + UIMetrics.scaled(3), style: .continuous)
                .strokeBorder(isActive ? MuxyTheme.accent : .clear, lineWidth: 1.5)
                .animation(.easeInOut(duration: 0.15), value: isActive)
        }
        .overlay(alignment: .bottomTrailing) {
            if isRefreshingWorktrees {
                ProgressView()
                    .controlSize(.mini)
                    .padding(UIMetrics.spacing2)
            }
        }
    }

    private func iconBackground(hasLogo: Bool) -> AnyShapeStyle {
        if project.isHome {
            return AnyShapeStyle(hovered ? MuxyTheme.accent.opacity(0.85) : MuxyTheme.accent)
        }
        if hasLogo {
            return AnyShapeStyle(Color.clear)
        }
        if let tint = ProjectIconColor.color(for: project.iconColor) {
            return AnyShapeStyle(hovered ? tint.opacity(0.85) : tint)
        }
        if hovered {
            return AnyShapeStyle(MuxyTheme.fg.opacity(0.22))
        }
        return AnyShapeStyle(MuxyTheme.fg.opacity(0.18))
    }

    private var letterForeground: Color {
        if let foreground = ProjectIconColor.foreground(for: project.iconColor) {
            return foreground
        }
        return isActive ? MuxyTheme.fg : MuxyTheme.fgMuted
    }

    private var shortcutHint: KeyCombo? {
        guard let shortcutIndex,
              let action = ShortcutAction.projectAction(for: shortcutIndex)
        else { return nil }
        return ModifierKeyMonitor.shared.hint(for: action)
    }

    private func pickLogoImage() {
        let panel = NSOpenPanel()
        panel.title = L10n.string("Choose a Logo Image")
        panel.allowedContentTypes = [.image]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.directoryURL = URL(fileURLWithPath: project.path)

        guard panel.runModal() == .OK,
              let url = panel.url,
              let image = NSImage(contentsOf: url)
        else { return }

        logoCropImage = IdentifiableImage(image: image)
    }

    private func handleCreateWorktreeResult(_ result: CreateWorktreeResult) {
        switch result {
        case let .created(worktree, runSetup):
            appState.selectWorktree(projectID: project.id, worktree: worktree)
            if runSetup,
               let paneID = appState.focusedArea(for: project.id)?.activeTab?.content.pane?.id
            {
                Task {
                    await WorktreeSetupRunner.run(
                        sourceProjectPath: project.path,
                        paneID: paneID
                    )
                }
            }
        case .cancelled:
            break
        }
    }

    private func startRename() {
        renameText = project.name
        isRenaming = true
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespaces)
        if !trimmed.isEmpty {
            onRename(trimmed)
        }
        isRenaming = false
    }

    private func cancelRename() {
        isRenaming = false
    }

    private func refreshWorktrees() async {
        await WorktreeRefreshHelper.refresh(
            project: project,
            appState: appState,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            isRefreshing: $isRefreshingWorktrees
        )
    }

    private func beginRemove(worktree: Worktree) {
        let activeWorktreeID = appState.activeWorktreeID[project.id]
        let remaining = worktrees.filter { $0.id != worktree.id }
        let replacement = remaining.first(where: { $0.id == activeWorktreeID })
            ?? remaining.first(where: { $0.isPrimary })
            ?? remaining.first
        worktreeStore.beginRemoval(
            worktree: worktree,
            projectID: project.id,
            repoPath: project.path,
            context: projectGroupStore.workspaceContext(for: project),
            onSuccess: {
                appState.removeWorktree(
                    projectID: project.id,
                    worktree: worktree,
                    replacement: replacement
                )
                worktreeStore.remove(worktreeID: worktree.id, from: project.id)
            }
        )
    }
}

private struct RenamePopover: View {
    @Binding var text: String
    let onCommit: () -> Void
    let onCancel: () -> Void
    @FocusState private var isFocused: Bool

    var body: some View {
        VStack(spacing: UIMetrics.spacing4) {
            Text(L10n.resource("Rename Project"))
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            TextField(L10n.string("Project name"), text: $text)
                .textFieldStyle(.roundedBorder)
                .font(.system(size: UIMetrics.fontBody))
                .focused($isFocused)
                .onSubmit { onCommit() }
                .onExitCommand { onCancel() }
        }
        .padding(UIMetrics.spacing6)
        .frame(width: UIMetrics.scaled(200))
        .onAppear { isFocused = true }
    }
}

private struct IdentifiableImage: Identifiable {
    let id = UUID()
    let image: NSImage
}
