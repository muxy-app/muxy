import AppKit
import MuxyShared
import SwiftUI

struct TabFocusedProjectRow: View {
    let project: Project
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

    private var isActive: Bool {
        appState.activeProjectID == project.id
    }

    private var isExpanded: Bool {
        expansionStore.isExpanded(project.id, default: false)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            if isExpanded {
                TabFocusedTabsList(project: project, shortcutNumbers: shortcutNumbers)
            }
        }
        .onAppear { applyDefaultExpansion() }
        .task(id: project.path) { await checkGitRepo() }
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing3) {
            icon
            if isRenaming {
                renameField
            } else {
                Text(project.name)
                    .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            Spacer(minLength: UIMetrics.spacing2)
            if hovered {
                HStack(spacing: 0) {
                    actions
                    chevron
                }
            } else {
                statusIndicator
            }
            if project.isPinned {
                Image(systemName: "pin.fill")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .help("Pinned")
                    .accessibilityLabel("Pinned")
            }
        }
        .padding(.horizontal, TabFocusedSidebarMetrics.rowHorizontalInset)
        .padding(.vertical, UIMetrics.spacing3)
        .background(headerBackground)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .onTapGesture { toggle() }
        .contextMenu {
            if project.isHome {
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
                if !enabled, isGroupedByWorktree {
                    expansionStore.setGroupedByWorktree(project.id, grouped: false)
                }
            }
        )
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

    private var hasWorktreeUI: Bool {
        guard project.worktreesEnabled, !project.isHome else { return false }
        return isGitRepo || worktreeStore.list(for: project.id).count > 1
    }

    private var isGroupedByWorktree: Bool {
        expansionStore.isGroupedByWorktree(project.id)
    }

    @ViewBuilder
    private var actions: some View {
        if !isGroupedByWorktree {
            TabFocusedTabActions(project: project, worktree: nil)
        }
        if hasWorktreeUI {
            SidebarActionButton(
                symbol: "point.3.connected.trianglepath.dotted",
                label: isGroupedByWorktree ? "Ungroup Worktree Tabs" : "Group Tabs by Worktree",
                isActive: isGroupedByWorktree
            ) {
                expansionStore.setGroupedByWorktree(project.id, grouped: !isGroupedByWorktree)
            }
        }
    }

    private var chevron: some View {
        Button(action: toggle) {
            Image(systemName: "chevron.right")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .rotationEffect(.degrees(isExpanded ? 90 : 0))
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(isExpanded ? "Collapse \(project.name)" : "Expand \(project.name)")
    }

    private var headerBackground: AnyShapeStyle {
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }

    private var displayLetter: String {
        String(project.name.prefix(1)).uppercased()
    }

    private var icon: some View {
        let logo = resolvedLogo
        return ZStack {
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous)
                .fill(iconBackground(hasLogo: logo != nil))
            if project.isHome {
                Image(systemName: Project.homeIcon)
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.accentForeground)
            } else if let logo {
                Image(nsImage: logo)
                    .resizable()
                    .scaledToFill()
                    .frame(width: UIMetrics.iconXL, height: UIMetrics.iconXL)
                    .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous))
            } else if let iconName = project.icon {
                Image(systemName: iconName)
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(letterForeground)
            } else {
                Text(displayLetter)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .bold))
                    .foregroundStyle(letterForeground)
            }
        }
        .frame(width: UIMetrics.iconXL, height: UIMetrics.iconXL)
    }

    private func iconBackground(hasLogo: Bool) -> AnyShapeStyle {
        if project.isHome { return AnyShapeStyle(MuxyTheme.accent) }
        if hasLogo { return AnyShapeStyle(Color.clear) }
        if let tint = ProjectIconColor.color(for: project.iconColor) {
            return AnyShapeStyle(tint)
        }
        return AnyShapeStyle(MuxyTheme.fg.opacity(0.18))
    }

    private var letterForeground: Color {
        ProjectIconColor.foreground(for: project.iconColor) ?? MuxyTheme.fg
    }

    private var resolvedLogo: NSImage? {
        guard let filename = project.logo,
              let path = ProjectLogoStorage.safeLogoPath(for: filename)
        else { return nil }
        return NSImage(contentsOfFile: path)
    }

    private func toggle() {
        withAnimation(.easeInOut(duration: 0.15)) {
            expansionStore.set(project.id, expanded: !isExpanded)
        }
    }

    private func applyDefaultExpansion() {
        let key = TabFocusedSidebarPreferences.projectExpandedKey(project.id)
        guard UserDefaults.standard.object(forKey: key) == nil, isActive, !isExpanded else { return }
        expansionStore.set(project.id, expanded: true)
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
        renameText = project.name
        isRenaming = true
        renameFieldFocused = true
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespaces)
        if !trimmed.isEmpty {
            projectStore.rename(id: project.id, to: trimmed)
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
