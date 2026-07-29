import AppKit
import MuxyShared
import SwiftUI

enum TabFocusedActivityScope {
    static func worktreeIDs(
        rowWorktreeID: UUID?,
        primaryWorktreeID: UUID?,
        hiddenWorktreeIDs: Set<UUID>
    ) -> Set<UUID> {
        if let rowWorktreeID {
            return [rowWorktreeID]
        }
        guard let primaryWorktreeID else { return hiddenWorktreeIDs }
        return hiddenWorktreeIDs.union([primaryWorktreeID])
    }
}

struct TabFocusedProjectRow: View {
    let project: Project
    var worktree: Worktree?
    let shortcutNumbers: [UUID: Int]
    var content: TabFocusedSidebarContent = .tabs
    let groupWorktrees: Bool

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @State private var expansionStore = TabFocusedSidebarState.shared
    @State private var notificationStore = NotificationStore.shared
    @State private var progressStore = TerminalProgressStore.shared
    @AppStorage(WorktreeListPreferences.showUnreadIndicatorKey)
    private var showUnreadIndicator = WorktreeListPreferences.defaultShowUnreadIndicator

    @State private var hovered = false
    @State private var isGitRepo = false
    @State private var isCheckingGitRepo = true
    @State private var isRenaming = false
    @State private var renameText = ""
    @State private var showSymbolPicker = false
    @State private var showColorPicker = false
    @State private var showCreateWorktreeSheet = false
    @State private var showAgentProviderMenu = false
    @State private var logoCropImage: IdentifiableProjectImage?
    @State private var projectPendingRemoval = false
    @FocusState private var renameFieldFocused: Bool

    private var isWorktreeRow: Bool { worktree != nil }

    private var rowID: UUID { worktree?.id ?? project.id }

    private var rowTitle: String {
        worktree?.name ?? project.localizedDisplayName
    }

    private var listWorktree: Worktree? {
        worktree ?? worktreeStore.primary(for: project.id)
    }

    private var isActive: Bool {
        guard appState.activeProjectID == project.id else { return false }
        guard let worktree else { return isActiveWorktreePrimary }
        return appState.activeWorktreeID[project.id] == worktree.id
    }

    private var isActiveWorktreePrimary: Bool {
        guard let activeID = appState.activeWorktreeID[project.id] else { return true }
        return worktreeStore.primary(for: project.id)?.id == activeID
    }

    private var hiddenWorktreeIDs: Set<UUID> {
        guard !isWorktreeRow else { return [] }
        let secondaries = worktreeStore.list(for: project.id).filter { !$0.isPrimary }
        guard !secondaries.isEmpty else { return [] }
        if groupWorktrees, project.worktreesEnabled, isExpanded {
            return []
        }

        let revealed = TabFocusedSidebarRows.standaloneWorktrees(
            project: project,
            content: content,
            groupWorktrees: groupWorktrees,
            worktrees: secondaries,
            hasTabs: { appState.hasTabs(for: $0) }
        )
        return Set(secondaries.map(\.id)).subtracting(revealed.map(\.id))
    }

    private var rowActivity: TerminalActivity? {
        let worktreeIDs = TabFocusedActivityScope.worktreeIDs(
            rowWorktreeID: worktree?.id,
            primaryWorktreeID: worktreeStore.primary(for: project.id)?.id,
            hiddenWorktreeIDs: hiddenWorktreeIDs
        )
        guard !worktreeIDs.isEmpty else { return nil }

        let agentStore = AgentStatusStore.shared
        let hasProgress = worktreeIDs.contains { progressStore.hasActiveProgress(forWorktree: $0) }
        let agentStatus = worktreeIDs
            .compactMap { agentStore.status(forWorktree: $0) }
            .max { $0.priority < $1.priority }
        let unreadCount = showUnreadIndicator
            ? worktreeIDs.reduce(0) { $0 + notificationStore.unreadCount(for: project.id, worktreeID: $1) }
            : 0
        let completionPending = worktreeIDs.contains {
            progressStore.hasCompletionPending(forWorktree: $0)
                || agentStore.hasCompletionPending(forWorktree: $0)
        }

        return TerminalActivity.resolve(
            progress: hasProgress ? TerminalProgress(kind: .indeterminate, percent: nil) : nil,
            agentStatus: agentStatus,
            unreadCount: unreadCount,
            completionPending: completionPending
        )
    }

    private var isExpanded: Bool {
        expansionStore.isExpanded(rowID, default: false)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            if isExpanded {
                if groupWorktrees, !isWorktreeRow, project.worktreesEnabled {
                    TabFocusedWorktreeTree(
                        project: project,
                        worktrees: worktreeStore.list(for: project.id),
                        shortcutNumbers: shortcutNumbers,
                        content: content
                    )
                } else if let listWorktree {
                    switch content {
                    case .tabs:
                        TabFocusedTabsList(
                            project: project,
                            worktree: listWorktree,
                            shortcutNumbers: shortcutNumbers
                        )
                    case .agents:
                        AgentsFocusedTabsList(project: project, worktree: listWorktree)
                    }
                }
            }
        }
        .onAppear { applyDefaultExpansion() }
        .onChange(of: isActive) { _, active in
            guard active, !isExpanded else { return }
            withAnimation(.easeInOut(duration: 0.15)) {
                expansionStore.set(rowID, expanded: true)
            }
        }
        .task(id: project.path) { await checkGitRepo() }
    }

    private var header: some View {
        HStack(spacing: TabFocusedSidebarMetrics.iconTitleGap) {
            rowIcon
            if isRenaming {
                renameField
            } else {
                Text(rowTitle)
                    .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                    .foregroundStyle(projectTitleColor)
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            Spacer(minLength: UIMetrics.spacing2)
            trailingControls
            if !isWorktreeRow, project.isPinned {
                Image(systemName: "pin.fill")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
                    .help(L10n.string("Pinned"))
                    .accessibilityLabel(L10n.string("Pinned"))
            }
        }
        .padding(.horizontal, TabFocusedSidebarMetrics.rowHorizontalInset)
        .frame(minHeight: TabFocusedSidebarMetrics.rowHeight)
        .background {
            RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous)
                .fill(headerBackground)
        }
        .padding(.horizontal, TabFocusedSidebarMetrics.rowOuterInset)
        .padding(.vertical, TabFocusedSidebarMetrics.rowVerticalPadding)
        .contentShape(RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous))
        .onHover { hovered = $0 }
        .onTapGesture { handleTap() }
        .contextMenu {
            if let worktree {
                worktreeContextMenu(worktree)
            } else if project.isHome {
                ProjectContextMenuFooter(
                    path: project.path,
                    workspaceContext: projectGroupStore.workspaceContext(for: project),
                    separatesFromPreviousActions: false
                ) {
                    Button(L10n.string("Hide Home")) { HomeProjectPreferences.isVisible = false }
                }
            } else {
                projectContextMenu
            }
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
        .popover(isPresented: $showColorPicker, arrowEdge: .trailing) {
            ProjectIconColorPicker(selectedID: project.iconColor) { id in
                projectStore.setIconColor(id: project.id, to: id)
                showColorPicker = false
            }
        }
        .popover(isPresented: $showSymbolPicker, arrowEdge: .trailing) {
            SFSymbolPicker(selectedName: project.icon) { name in
                projectStore.setIcon(id: project.id, to: name)
                showSymbolPicker = false
            }
        }
        .alert(
            L10n.string("Remove \"\(project.name)\"?"),
            isPresented: $projectPendingRemoval
        ) {
            Button(L10n.string("Remove"), role: .destructive) { performRemove() }
                .keyboardShortcut(.defaultAction)
            Button(L10n.string("Cancel"), role: .cancel) {}
                .keyboardShortcut(.cancelAction)
        } message: {
            Text(L10n.resource("This will remove the project from Muxy. Project files on disk will not be deleted."))
        }
    }

    private var renameField: some View {
        TextField("", text: $renameText)
            .textFieldStyle(.plain)
            .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
            .foregroundStyle(MuxyTheme.fg)
            .focused($renameFieldFocused)
            .onSubmit { commitRename() }
            .onExitCommand { isRenaming = false }
            .onChange(of: renameFieldFocused) { _, focused in
                if !focused, isRenaming {
                    commitRename()
                }
            }
    }

    @ViewBuilder
    private var projectContextMenu: some View {
        if !project.isRemote {
            Button(
                project.isPinned
                    ? L10n.string("Unpin")
                    : L10n.string("Pin")
            ) {
                projectStore.setPinned(id: project.id, to: !project.isPinned)
            }
            Divider()
        }
        Button(L10n.string("Set Logo…")) { pickLogoImage() }
        if project.logo != nil {
            Button(L10n.string("Remove Logo")) { projectStore.setLogo(id: project.id, to: nil) }
        }
        Button(L10n.string("Set Icon…")) { showSymbolPicker = true }
        if project.icon != nil {
            Button(L10n.string("Remove Icon")) { projectStore.setIcon(id: project.id, to: nil) }
        }
        Button(L10n.string("Set Icon Color…")) { showColorPicker = true }
        if project.iconColor != nil {
            Button(L10n.string("Reset Icon Color")) { projectStore.setIconColor(id: project.id, to: nil) }
        }
        Divider()
        Button(L10n.string("Rename Project")) { startRename() }
        if isGitRepo {
            Divider()
            Toggle(L10n.string("Worktrees"), isOn: worktreesEnabledBinding)
            if project.worktreesEnabled {
                Button(L10n.string("Refresh Worktrees")) { Task { await refreshWorktrees() } }
                Button(L10n.string("New Worktree…")) { showCreateWorktreeSheet = true }
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
            Button(L10n.string("Remove Project"), role: .destructive) { projectPendingRemoval = true }
        }
    }

    private var worktreesEnabledBinding: Binding<Bool> {
        Binding(
            get: { project.worktreesEnabled },
            set: { enabled in
                projectStore.setWorktreesEnabled(id: project.id, to: enabled)
            }
        )
    }

    @ViewBuilder
    private func worktreeContextMenu(_ worktree: Worktree) -> some View {
        Button(L10n.string("New Terminal Tab")) {
            selectWorktreeIfNeeded(worktree)
            appState.createTab(projectID: project.id)
        }
        Divider()
        Button(L10n.string("Rename Worktree")) { startRename() }
        if worktree.canBeRemoved {
            Divider()
            Button(L10n.string("Remove Worktree"), role: .destructive) {
                Task { await requestRemoveWorktree(worktree) }
            }
        }
    }

    private func selectWorktreeIfNeeded(_ worktree: Worktree) {
        guard appState.activeWorktreeID[project.id] != worktree.id else { return }
        appState.selectProject(project, worktree: worktree)
    }

    private func requestRemoveWorktree(_ worktree: Worktree) async {
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

    @ViewBuilder
    private var statusIndicator: some View {
        if let rowActivity {
            TerminalActivityIndicator(activity: rowActivity)
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
        }
    }

    private var trailingControls: some View {
        HStack(spacing: 0) {
            if hovered || showAgentProviderMenu {
                actions
            } else if !isFocused {
                if isWorktreeRow {
                    worktreeIndicator
                } else if !isExpanded {
                    statusIndicator
                }
            }
            if isFocused {
                focusModeButton
            }
        }
    }

    @ViewBuilder
    private var actions: some View {
        switch content {
        case .tabs:
            if showsProjectLevelActions {
                TabFocusedTabActions(project: project, worktree: listWorktree)
            }
        case .agents:
            if showsProjectLevelActions {
                AgentsFocusedTabActions(
                    project: project,
                    worktree: listWorktree,
                    showingProviders: $showAgentProviderMenu
                )
            }
        }
        if content == .tabs, !isWorktreeRow, !project.isHome, !isFocused {
            focusModeButton
        }
    }

    private var isFocused: Bool {
        content == .tabs && !isWorktreeRow && expansionStore.focusMode && appState.activeProjectID == project.id
    }

    private var showsProjectLevelActions: Bool {
        isWorktreeRow || !groupWorktrees || !project.worktreesEnabled
    }

    private var focusModeButton: some View {
        SidebarActionButton(
            symbol: "scope",
            label: isFocused ? L10n.string("Exit Focus Mode") : L10n.string("Focus This Project"),
            isActive: isFocused,
            action: toggleFocusMode
        )
    }

    private func toggleFocusMode() {
        if isFocused {
            expansionStore.focusMode = false
            return
        }
        activate(worktree)
        expansionStore.focusMode = true
    }

    private func activate(_ target: Worktree?) {
        TabFocusedSidebarTarget.activate(
            project: project,
            worktree: target,
            appState: appState,
            worktreeStore: worktreeStore
        )
    }

    private var projectTitleColor: Color {
        hovered ? MuxyTheme.fg : MuxyTheme.fgMuted
    }

    private var rowIcon: some View {
        Image(systemName: isExpanded ? "folder.fill" : "folder")
            .font(.system(size: UIMetrics.fontHeadline, weight: .regular))
            .foregroundStyle(projectTitleColor)
            .frame(width: TabFocusedSidebarMetrics.folderIconSize, height: TabFocusedSidebarMetrics.folderIconSize)
            .accessibilityHidden(true)
    }

    @ViewBuilder
    private var worktreeIndicator: some View {
        if let rowActivity {
            TerminalActivityIndicator(activity: rowActivity)
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
        } else {
            Image(systemName: "arrow.triangle.branch")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
                .help(L10n.string("Worktree"))
                .accessibilityLabel(L10n.string("Worktree"))
        }
    }

    private var headerBackground: AnyShapeStyle {
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
            activateRow()
        }
    }

    private func activateRow() {
        activate(listWorktree)
        withAnimation(.easeInOut(duration: 0.15)) {
            expansionStore.set(rowID, expanded: true)
        }
    }

    private func toggle() {
        withAnimation(.easeInOut(duration: 0.15)) {
            expansionStore.set(rowID, expanded: !isExpanded)
        }
    }

    private func applyDefaultExpansion() {
        let key = TabFocusedSidebarPreferences.projectExpandedKey(rowID)
        guard UserDefaults.standard.object(forKey: key) == nil, isActive, !isExpanded else { return }
        expansionStore.set(rowID, expanded: true)
    }

    private func checkGitRepo() async {
        guard !project.isHome else {
            isGitRepo = false
            isCheckingGitRepo = false
            return
        }
        let context = projectGroupStore.workspaceContext(for: project)
        if let cached = GitRepoStatusCache.shared.cachedStatus(for: project.path, context: context) {
            isGitRepo = cached
            isCheckingGitRepo = false
            return
        }
        let result = await GitWorktreeService.shared.isGitRepository(project.path, context: context)
        guard !Task.isCancelled else { return }
        isGitRepo = result
        isCheckingGitRepo = false
        GitRepoStatusCache.shared.update(path: project.path, context: context, isGitRepo: isGitRepo)
    }

    private func pickLogoImage() {
        let panel = NSOpenPanel()
        panel.title = L10n.string("Choose a Logo Image")
        panel.allowedContentTypes = [.image]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.directoryURL = URL(fileURLWithPath: project.path)
        guard panel.runModal() == .OK, let url = panel.url, let image = NSImage(contentsOf: url) else { return }
        logoCropImage = IdentifiableProjectImage(image: image)
    }

    private func startRename() {
        renameText = rowTitle
        isRenaming = true
        renameFieldFocused = true
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespaces)
        if !trimmed.isEmpty {
            if let worktree {
                worktreeStore.rename(worktreeID: worktree.id, in: project.id, to: trimmed)
            } else {
                projectStore.rename(id: project.id, to: trimmed)
            }
        }
        isRenaming = false
    }

    private func refreshWorktrees() async {
        await WorktreeRefreshHelper.refresh(
            project: project,
            appState: appState,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }

    private func handleCreateWorktreeResult(_ result: CreateWorktreeResult) {
        guard case let .created(worktree, runSetup) = result else { return }
        appState.selectWorktree(projectID: project.id, worktree: worktree)
        expansionStore.set(project.id, expanded: true)
        guard runSetup,
              let paneID = appState.focusedArea(for: project.id)?.activeTab?.content.pane?.id
        else { return }
        Task {
            await WorktreeSetupRunner.run(sourceProjectPath: project.path, paneID: paneID)
        }
    }

    private func performRemove() {
        Task {
            do {
                try await ProjectRemovalService.remove(
                    project,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore
                )
            } catch {
                ToastState.shared.show(title: L10n.string("Could not remove project"), body: error.localizedDescription)
            }
        }
    }
}

private struct IdentifiableProjectImage: Identifiable {
    let id = UUID()
    let image: NSImage
}
