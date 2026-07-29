import SwiftUI

struct WorktreeLeafRow: View {
    let project: Project
    let worktree: Worktree
    let depth: Int
    let shortcutNumbers: [UUID: Int]
    let content: TabFocusedSidebarContent

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @State private var expansionStore = TabFocusedSidebarState.shared
    @State private var notificationStore = NotificationStore.shared
    @State private var progressStore = TerminalProgressStore.shared
    @AppStorage(WorktreeListPreferences.showUnreadIndicatorKey)
    private var showUnreadIndicator = WorktreeListPreferences.defaultShowUnreadIndicator
    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @State private var showAgentProviderMenu = false
    @FocusState private var renameFieldFocused: Bool

    private var rowTitle: String {
        worktree.sidebarDisplayName
    }

    private var isActive: Bool {
        guard appState.activeProjectID == project.id else { return false }
        guard let activeWorktreeID = appState.activeWorktreeID[project.id] else { return worktree.isPrimary }
        return activeWorktreeID == worktree.id
    }

    private var isExpanded: Bool {
        expansionStore.isExpanded(worktree.id, default: false)
    }

    private var leafIcon: some View {
        Image(systemName: "arrow.triangle.branch")
            .font(.system(size: UIMetrics.fontBody, weight: .semibold))
            .foregroundStyle(isExpanded ? MuxyTheme.fg : MuxyTheme.fgMuted)
    }

    private var leafTrailing: some View {
        HStack(spacing: UIMetrics.spacing2) {
            if let badge = shortcutBadge {
                Text(L10n.resource("\(badge)"))
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .padding(.horizontal, UIMetrics.spacing2)
                    .padding(.vertical, 2)
                    .background(
                        Capsule()
                            .fill(MuxyTheme.surface)
                    )
            }

            if isRemoving {
                ProgressView()
                    .scaleEffect(0.5)
            }

            if hovered || showAgentProviderMenu {
                leafAction
            } else {
                statusIndicator
            }
        }
    }

    private var rowActivity: TerminalActivity? {
        let agentStore = AgentStatusStore.shared
        return TerminalActivity.resolve(
            progress: progressStore.hasActiveProgress(forWorktree: worktree.id)
                ? TerminalProgress(kind: .indeterminate, percent: nil)
                : nil,
            agentStatus: agentStore.status(forWorktree: worktree.id),
            unreadCount: showUnreadIndicator
                ? notificationStore.unreadCount(for: project.id, worktreeID: worktree.id)
                : 0,
            completionPending: progressStore.hasCompletionPending(forWorktree: worktree.id)
                || agentStore.hasCompletionPending(forWorktree: worktree.id)
        )
    }

    @ViewBuilder
    private var statusIndicator: some View {
        if let rowActivity {
            TerminalActivityIndicator(activity: rowActivity)
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
        }
    }

    private var shortcutBadge: Int? {
        shortcutNumbers[worktree.id]
    }

    private var isRemoving: Bool {
        worktreeStore.isRemoving(worktreeID: worktree.id)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            leafHeader
            if isExpanded {
                switch content {
                case .tabs:
                    TabFocusedTabsList(
                        project: project,
                        worktree: worktree,
                        shortcutNumbers: shortcutNumbers
                    )
                case .agents:
                    AgentsFocusedTabsList(project: project, worktree: worktree)
                }
            }
        }
        .onAppear { applyDefaultExpansion() }
        .onChange(of: isActive) { _, active in
            guard active, !isExpanded else { return }
            withAnimation(.easeInOut(duration: 0.15)) {
                expansionStore.set(worktree.id, expanded: true)
            }
        }
    }

    private func applyDefaultExpansion() {
        let key = TabFocusedSidebarPreferences.projectExpandedKey(worktree.id)
        guard UserDefaults.standard.object(forKey: key) == nil, isActive, !isExpanded else { return }
        expansionStore.set(worktree.id, expanded: true)
    }

    private var leafHeader: some View {
        HStack(spacing: TabFocusedSidebarMetrics.iconTitleGap) {
            leafIcon
            if isRenaming {
                renameField
            } else {
                Text(rowTitle)
                    .font(.system(size: UIMetrics.fontHeadline, weight: .regular))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            Spacer(minLength: UIMetrics.spacing2)
            leafTrailing
        }
        .padding(.leading, UIMetrics.spacing6 * CGFloat(depth))
        .padding(.trailing, TabFocusedSidebarMetrics.rowHorizontalInset)
        .frame(minHeight: TabFocusedSidebarMetrics.rowHeight)
        .background {
            RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous)
                .fill(leafBackground)
        }
        .padding(.horizontal, TabFocusedSidebarMetrics.rowOuterInset)
        .padding(.vertical, TabFocusedSidebarMetrics.rowVerticalPadding)
        .contentShape(RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous))
        .onHover { hovered = $0 }
        .onTapGesture { handleTap() }
        .contextMenu { worktreeContextMenu }
    }

    @ViewBuilder
    private var leafAction: some View {
        switch content {
        case .tabs:
            SidebarActionButton(symbol: "plus", label: L10n.string("New Terminal Tab")) {
                activate()
                appState.createTab(projectID: project.id)
            }
        case .agents:
            AgentsFocusedTabActions(
                project: project,
                worktree: worktree,
                showingProviders: $showAgentProviderMenu
            )
        }
    }

    private var leafBackground: AnyShapeStyle {
        if hovered {
            return AnyShapeStyle(MuxyTheme.hover)
        }
        return AnyShapeStyle(Color.clear)
    }

    private func handleTap() {
        switch TabFocusedSidebarRowTap.resolve(content: content, isActive: isActive) {
        case .toggleExpansion:
            toggle()
        case .activateRow:
            activate()
            withAnimation(.easeInOut(duration: 0.15)) {
                expansionStore.set(worktree.id, expanded: true)
            }
        }
    }

    private func activate() {
        TabFocusedSidebarTarget.activate(
            project: project,
            worktree: worktree,
            appState: appState,
            worktreeStore: worktreeStore
        )
    }

    private func toggle() {
        withAnimation(.easeInOut(duration: 0.15)) {
            expansionStore.set(worktree.id, expanded: !isExpanded)
        }
    }

    @ViewBuilder
    private var worktreeContextMenu: some View {
        Button(L10n.string("New Terminal Tab")) {
            appState.selectProject(project, worktree: worktree)
            appState.createTab(projectID: project.id)
        }
        Divider()
        Button(L10n.string("Rename Worktree")) { startRename() }
        if worktree.canBeRemoved {
            Divider()
            Button(L10n.string("Remove Worktree"), role: .destructive) {
                Task { await requestRemoveWorktree() }
            }
        }
    }

    private func startRename() {
        renameText = worktree.name
        isRenaming = true
        renameFieldFocused = true
    }

    private var renameField: some View {
        TextField("", text: $renameText)
            .font(.system(size: UIMetrics.fontHeadline, weight: .regular))
            .foregroundStyle(MuxyTheme.fg)
            .textFieldStyle(.plain)
            .focused($renameFieldFocused)
            .onSubmit { commitRename() }
            .onChange(of: renameFieldFocused) { _, focused in
                if !focused {
                    commitRename()
                }
            }
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty, trimmed != worktree.name else {
            isRenaming = false
            return
        }
        worktreeStore.rename(worktreeID: worktree.id, in: project.id, to: trimmed)
        isRenaming = false
    }

    private func requestRemoveWorktree() async {
        let context = projectGroupStore.workspaceContext(for: project)
        worktreeStore.beginRemoval(
            worktree: worktree,
            projectID: project.id,
            repoPath: project.path,
            context: context
        ) {
            appState.removeWorktree(
                projectID: project.id,
                worktree: worktree,
                replacement: worktreeStore.preferred(
                    for: project.id,
                    matching: appState.activeWorktreeID[project.id]
                )
            )
        }
    }
}
