import AppKit
import MuxyShared
import SwiftUI

struct TabFocusedProjectRow: View {
    let project: Project
    var worktree: Worktree?
    let shortcutNumbers: [UUID: Int]

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @State private var expansionStore = TabFocusedSidebarState.shared
    @State private var notificationStore = NotificationStore.shared
    @State private var progressStore = TerminalProgressStore.shared

    @State private var hovered = false
    @State private var isGitRepo = false
    @State private var isCheckingGitRepo = true
    @State private var isRenaming = false
    @State private var renameText = ""
    @State private var showSymbolPicker = false
    @State private var showColorPicker = false
    @State private var showCreateWorktreeSheet = false
    @State private var logoCropImage: IdentifiableProjectImage?
    @State private var projectPendingRemoval = false
    @FocusState private var renameFieldFocused: Bool

    private var isWorktreeRow: Bool { worktree != nil }

    private var rowID: UUID { worktree?.id ?? project.id }

    private var rowTitle: String {
        worktree?.name ?? project.name
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

    private var isExpanded: Bool {
        expansionStore.isExpanded(rowID, default: false)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            if isExpanded, let listWorktree {
                TabFocusedTabsList(project: project, worktree: listWorktree, shortcutNumbers: shortcutNumbers)
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
                    .help("Pinned")
                    .accessibilityLabel("Pinned")
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
        .onTapGesture { toggle() }
        .contextMenu {
            if let worktree {
                worktreeContextMenu(worktree)
            } else if project.isHome {
                Button("Hide Home") { HomeProjectPreferences.isVisible = false }
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
                    let path = ProjectLogoStorage.save(croppedImage: cropped, forProjectID: project.id)
                    projectStore.setLogo(id: project.id, to: path)
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
            "Remove \"\(project.name)\"?",
            isPresented: $projectPendingRemoval
        ) {
            Button("Remove", role: .destructive) { performRemove() }
                .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) {}
                .keyboardShortcut(.cancelAction)
        } message: {
            Text("This will remove the project from Muxy. Project files on disk will not be deleted.")
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
                if !focused, isRenaming { commitRename() }
            }
    }

    @ViewBuilder
    private var projectContextMenu: some View {
        if !project.isRemote {
            Button(project.isPinned ? "Unpin" : "Pin") {
                projectStore.setPinned(id: project.id, to: !project.isPinned)
            }
            Divider()
        }
        Button("Set Logo…") { pickLogoImage() }
        if project.logo != nil {
            Button("Remove Logo") { projectStore.setLogo(id: project.id, to: nil) }
        }
        Button("Set Icon…") { showSymbolPicker = true }
        if project.icon != nil {
            Button("Remove Icon") { projectStore.setIcon(id: project.id, to: nil) }
        }
        Button("Set Icon Color…") { showColorPicker = true }
        if project.iconColor != nil {
            Button("Reset Icon Color") { projectStore.setIconColor(id: project.id, to: nil) }
        }
        Divider()
        Button("Rename Project") { startRename() }
        if isGitRepo {
            Divider()
            Toggle("Worktrees", isOn: worktreesEnabledBinding)
            if project.worktreesEnabled {
                Button("Refresh Worktrees") { Task { await refreshWorktrees() } }
                Button("New Worktree…") { showCreateWorktreeSheet = true }
            }
        } else if isCheckingGitRepo {
            Divider()
            Button("Loading Worktrees…") {}
                .disabled(true)
        }
        if !projectGroupStore.groups.isEmpty {
            Divider()
            ProjectGroupMembershipMenu(project: project)
        }
        Divider()
        Button("Remove Project", role: .destructive) { projectPendingRemoval = true }
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
        Button("New Terminal Tab") {
            selectWorktreeIfNeeded(worktree)
            appState.createTab(projectID: project.id)
        }
        Divider()
        Button("Rename Worktree") { startRename() }
        if worktree.canBeRemoved {
            Divider()
            Button("Remove Worktree", role: .destructive) {
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
        worktreeStore.beginRemoval(worktree: worktree, repoPath: project.path, context: context) {
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
        let unread = notificationStore.unreadCount(for: project.id)
        if progressStore.hasActiveProgress(for: project.id) {
            ProgressView()
                .controlSize(.mini)
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
        } else if unread > 0 {
            NotificationBadge(count: unread)
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
        } else if progressStore.hasCompletionPending(for: project.id) {
            Circle()
                .fill(MuxyTheme.accent)
                .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
        }
    }

    private var trailingControls: some View {
        HStack(spacing: 0) {
            if hovered {
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
        TabFocusedTabActions(project: project, worktree: worktree)
        if !isWorktreeRow, !project.isHome, !isFocused {
            focusModeButton
        }
    }

    private var isFocused: Bool {
        !isWorktreeRow && expansionStore.focusMode && isActive
    }

    private var focusModeButton: some View {
        SidebarActionButton(
            symbol: "scope",
            label: isFocused ? "Exit Focus Mode" : "Focus This Project",
            isActive: isFocused,
            action: toggleFocusMode
        )
    }

    private func toggleFocusMode() {
        if isFocused {
            expansionStore.focusMode = false
            return
        }
        activateProjectIfNeeded()
        expansionStore.focusMode = true
    }

    private func activateProjectIfNeeded() {
        guard !isActive else { return }
        worktreeStore.ensurePrimary(for: project)
        guard let target = worktreeStore.preferred(
            for: project.id,
            matching: appState.activeWorktreeID[project.id]
        )
        else { return }
        appState.selectProject(project, worktree: target)
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

    private var worktreeIndicator: some View {
        Image(systemName: "arrow.triangle.branch")
            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
            .foregroundStyle(MuxyTheme.fgMuted)
            .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
            .help("Worktree")
            .accessibilityLabel("Worktree")
    }

    private var headerBackground: AnyShapeStyle {
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
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
        isGitRepo = await GitWorktreeService.shared.isGitRepository(project.path, context: context)
        isCheckingGitRepo = false
        GitRepoStatusCache.shared.update(path: project.path, context: context, isGitRepo: isGitRepo)
    }

    private func pickLogoImage() {
        let panel = NSOpenPanel()
        panel.title = "Choose a Logo Image"
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
            try? await ProjectRemovalService.remove(
                project,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        }
    }
}

private struct IdentifiableProjectImage: Identifiable {
    let id = UUID()
    let image: NSImage
}
