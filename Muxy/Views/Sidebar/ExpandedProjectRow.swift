import AppKit
import MuxyShared
import SwiftUI

struct ExpandedProjectRow: View {
    let project: Project
    let shortcutIndex: Int?
    let isAnyDragging: Bool
    let onSelect: () -> Void
    let onRemove: () -> Void
    let onRename: (String) -> Void
    let onSetLogo: (String?) -> Void
    let onSetIcon: (String?) -> Void
    let onSetIconColor: (String?) -> Void

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(ProjectCommandStore.self) private var projectCommandStore

    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch)
    private var autoExpandWorktrees = false

    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @State private var isGitRepo = false
    @State private var showCreateWorktreeSheet = false
    @State private var logoCropImage: IdentifiableExpandedImage?
    @State private var worktreesExpanded = false
    @State private var isRefreshingWorktrees = false
    @State private var showColorPicker = false
    @State private var showAddCommandSheet = false
    @State private var commandsSectionExpanded = true
    @State private var worktreesSectionExpanded = true
    @State private var activeCommandID: String?
    @State private var showSymbolPicker = false

    private var isActive: Bool {
        appState.activeProjectID == project.id
    }

    private var worktrees: [Worktree] {
        worktreeStore.list(for: project.id)
    }

    private var activeWorktreeID: UUID? {
        appState.activeWorktreeID[project.id]
    }

    private var activeWorktree: Worktree? {
        worktrees.first { $0.id == activeWorktreeID }
    }

    private var projectCommands: [ProjectCommand] {
        projectCommandStore.commands(for: project)
    }

    private var displayLetter: String {
        String(project.name.prefix(1)).uppercased()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            projectHeader
            if worktreesExpanded {
                expandedSections
            }
        }
        .task(id: project.path) {
            isGitRepo = await GitWorktreeService.shared.isGitRepository(project.path)
            if autoExpandWorktrees, isActive {
                worktreesExpanded = true
            }
        }
        .onChange(of: isActive) { _, active in
            guard autoExpandWorktrees, active else { return }
            withAnimation(.easeInOut(duration: 0.15)) {
                worktreesExpanded = true
            }
        }
        .contextMenu {
            Button("Set Logo...") { pickLogoImage() }
            if project.logo != nil {
                Button("Remove Logo") { onSetLogo(nil) }
            }
            Button("Set Icon...") { showSymbolPicker = true }
            if project.icon != nil {
                Button("Remove Icon") { onSetIcon(nil) }
            }
            Button("Set Icon Color...") { showColorPicker = true }
            if project.iconColor != nil {
                Button("Reset Icon Color") { onSetIconColor(nil) }
            }
            Divider()
            Button("Rename Project") { startRename() }
            if isGitRepo {
                Divider()
                Button("Refresh Worktrees") { Task { await refreshWorktrees() } }
                Button("New Worktree…") { showCreateWorktreeSheet = true }
            }
            if !projectGroupStore.groups.isEmpty {
                Divider()
                ProjectGroupMembershipMenu(project: project)
            }
            Divider()
            Button("Remove Project", role: .destructive, action: onRemove)
        }
        .sheet(isPresented: $showCreateWorktreeSheet) {
            CreateWorktreeSheet(project: project) { result in
                showCreateWorktreeSheet = false
                handleCreateWorktreeResult(result)
            }
        }
        .sheet(isPresented: $showAddCommandSheet) {
            AddProjectCommandSheet { name, command in
                showAddCommandSheet = false
                projectCommandStore.addManualCommand(name: name, command: command, to: project.id)
            } onLoadFromProject: {
                showAddCommandSheet = false
                projectCommandStore.loadDiscoveredCommands(from: project)
            } onCancel: {
                showAddCommandSheet = false
            }
        }
        .sheet(item: $logoCropImage) { item in
            LogoCropperSheet(
                sourceImage: item.image,
                onConfirm: { cropped in
                    logoCropImage = nil
                    let logoPath = ProjectLogoStorage.save(
                        croppedImage: cropped,
                        forProjectID: project.id
                    )
                    onSetLogo(logoPath)
                },
                onCancel: { logoCropImage = nil }
            )
        }
        .popover(isPresented: $isRenaming, arrowEdge: .trailing) {
            ExpandedRenamePopover(
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

    private var projectHeader: some View {
        HStack(spacing: UIMetrics.spacing4) {
            projectIcon

            VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
                Text(project.name)
                    .font(.system(size: UIMetrics.fontEmphasis, weight: isActive ? .semibold : .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.tail)

                if isGitRepo, let worktree = activeWorktree {
                    Text(worktree.isPrimary ? "primary" : worktree.name)
                        .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }

            Spacer(minLength: UIMetrics.spacing2)

            projectChevron
        }
        .padding(UIMetrics.spacing2)
        .background(headerBackground, in: RoundedRectangle(cornerRadius: UIMetrics.radiusLG))
        .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusLG))
        .accessibilityElement(children: .combine)
        .accessibilityLabel(projectHeaderAccessibilityLabel)
        .accessibilityAddTraits(isActive ? .isSelected : [])
        .accessibilityAddTraits(.isButton)
        .onHover { hovering in
            guard !isAnyDragging else { return }
            hovered = hovering
        }
        .onChange(of: isAnyDragging) { _, dragging in
            if dragging { hovered = false }
        }
        .onTapGesture {
            guard !isAnyDragging else { return }
            if isActive {
                withAnimation(.easeInOut(duration: 0.15)) {
                    worktreesExpanded.toggle()
                }
            } else {
                onSelect()
            }
        }
        .overlay {
            if showShortcutBadge, let shortcutIndex,
               let action = ShortcutAction.projectAction(for: shortcutIndex)
            {
                ShortcutBadge(label: KeyBindingStore.shared.combo(for: action).displayString)
            }
        }
    }

    private var projectChevron: some View {
        Button {
            withAnimation(.easeInOut(duration: 0.15)) {
                worktreesExpanded.toggle()
            }
        } label: {
            Image(systemName: "chevron.right")
                .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
                .rotationEffect(.degrees(worktreesExpanded ? 90 : 0))
                .animation(.easeInOut(duration: 0.15), value: worktreesExpanded)
                .frame(width: UIMetrics.scaled(18), height: UIMetrics.scaled(18))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(worktreesExpanded ? "Collapse Project" : "Expand Project")
    }

    private var projectIcon: some View {
        let logo = resolvedLogo
        let unread = NotificationStore.shared.unreadCount(for: project.id)
        let hasCompletion = TerminalProgressStore.shared.hasCompletionPending(for: project.id)
        return ZStack {
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .fill(iconBackground(hasLogo: logo != nil))

            if let logo {
                Image(nsImage: logo)
                    .resizable()
                    .scaledToFill()
                    .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconXXL)
                    .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
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
        .overlay(alignment: .topTrailing) {
            if unread > 0 {
                NotificationBadge(count: unread)
                    .offset(x: UIMetrics.spacing2, y: -UIMetrics.spacing2)
            } else if hasCompletion {
                Circle()
                    .fill(MuxyTheme.accent)
                    .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                    .offset(x: UIMetrics.spacing1, y: -UIMetrics.spacing1)
            }
        }
    }

    private var worktreeList: some View {
        VStack(spacing: UIMetrics.scaled(1)) {
            SidebarSectionHeader(
                title: "Worktrees",
                symbol: "square.stack.3d.up",
                expanded: worktreesSectionExpanded,
                onToggle: { worktreesSectionExpanded.toggle() }
            )

            if worktreesSectionExpanded {
                ForEach(worktrees) { worktree in
                    ExpandedWorktreeRow(
                        projectID: project.id,
                        worktree: worktree,
                        selected: worktree.id == activeWorktreeID,
                        onSelect: {
                            appState.selectWorktree(projectID: project.id, worktree: worktree)
                        },
                        onRename: { newName in
                            worktreeStore.rename(
                                worktreeID: worktree.id,
                                in: project.id,
                                to: newName
                            )
                        },
                        onRemove: worktree.canBeRemoved ? {
                            Task { await requestRemove(worktree: worktree) }
                        } : nil
                    )
                }

                ExpandedNewWorktreeButton {
                    showCreateWorktreeSheet = true
                }
            }
        }
        .padding(.top, UIMetrics.spacing1)
        .padding(.bottom, UIMetrics.spacing1)
    }

    private var expandedSections: some View {
        VStack(spacing: 0) {
            commandList
            if isGitRepo {
                worktreeList
            }
        }
        .padding(.top, UIMetrics.spacing2)
    }

    private var commandList: some View {
        VStack(spacing: UIMetrics.scaled(1)) {
            SidebarSectionHeader(
                title: "Commands",
                symbol: "command",
                expanded: commandsSectionExpanded,
                onToggle: { commandsSectionExpanded.toggle() }
            )
            if commandsSectionExpanded {
                ForEach(projectCommands) { command in
                    ProjectCommandRow(
                        command: command,
                        run: projectCommandStore.run(for: command.id, projectID: project.id),
                        active: activeCommandID == command.id,
                        onActivate: { activate(command) },
                        onRun: { run(command) },
                        onRestart: { restart(command) },
                        onStop: { requestStop(command) },
                        onDelete: { delete(command) }
                    )
                }
                ExpandedAddCommandButton { showAddCommandSheet = true }
            }
        }
    }

    private var projectHeaderAccessibilityLabel: String {
        var label = project.name
        if isGitRepo, let worktree = activeWorktree {
            label += ", worktree: \(worktree.isPrimary ? "primary" : worktree.name)"
        }
        return label
    }

    private var resolvedLogo: NSImage? {
        guard let filename = project.logo else { return nil }
        return NSImage(contentsOfFile: ProjectLogoStorage.logoPath(for: filename))
    }

    private func iconBackground(hasLogo: Bool) -> AnyShapeStyle {
        if hasLogo { return AnyShapeStyle(Color.clear) }
        if let tint = ProjectIconColor.color(for: project.iconColor) {
            return AnyShapeStyle(hovered ? tint.opacity(0.85) : tint)
        }
        if hovered { return AnyShapeStyle(MuxyTheme.fg.opacity(0.22)) }
        return AnyShapeStyle(MuxyTheme.fg.opacity(0.18))
    }

    private var letterForeground: Color {
        if let foreground = ProjectIconColor.foreground(for: project.iconColor) {
            return foreground
        }
        return isActive ? MuxyTheme.fg : MuxyTheme.fgMuted
    }

    private var headerBackground: AnyShapeStyle {
        if isActive { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }

    private var showShortcutBadge: Bool {
        guard let shortcutIndex,
              let action = ShortcutAction.projectAction(for: shortcutIndex)
        else { return false }
        return ModifierKeyMonitor.shared.isHolding(
            modifiers: KeyBindingStore.shared.combo(for: action).modifiers
        )
    }

    private func pickLogoImage() {
        let panel = NSOpenPanel()
        panel.title = "Choose a Logo Image"
        panel.allowedContentTypes = [.image]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false

        guard panel.runModal() == .OK,
              let url = panel.url,
              let image = NSImage(contentsOf: url)
        else { return }

        logoCropImage = IdentifiableExpandedImage(image: image)
    }

    private func handleCreateWorktreeResult(_ result: CreateWorktreeResult) {
        switch result {
        case let .created(worktree, runSetup):
            appState.selectWorktree(projectID: project.id, worktree: worktree)
            worktreesExpanded = true
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

    private func requestRemove(worktree: Worktree) async {
        let hasChanges = await GitWorktreeService.shared.hasUncommittedChanges(worktreePath: worktree.path)
        if !hasChanges {
            performRemove(worktree: worktree)
            return
        }
        presentRemoveConfirmation(worktree: worktree)
    }

    private func presentRemoveConfirmation(worktree: Worktree) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let alert = NSAlert()
        alert.messageText = "Remove worktree \"\(worktree.name)\"?"
        alert.informativeText = "This worktree has uncommitted changes. Removing it will permanently discard them."
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Remove")
        alert.addButton(withTitle: "Cancel")
        alert.buttons[0].keyEquivalent = "\r"
        alert.buttons[1].keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            guard response == .alertFirstButtonReturn else { return }
            performRemove(worktree: worktree)
        }
    }

    private func performRemove(worktree: Worktree) {
        let repoPath = project.path
        let remaining = worktrees.filter { $0.id != worktree.id }
        let replacement = remaining.first(where: { $0.id == activeWorktreeID })
            ?? remaining.first(where: { $0.isPrimary })
            ?? remaining.first
        appState.removeWorktree(
            projectID: project.id,
            worktree: worktree,
            replacement: replacement
        )
        worktreeStore.remove(worktreeID: worktree.id, from: project.id)
        Task.detached {
            await WorktreeStore.cleanupOnDisk(
                worktree: worktree,
                repoPath: repoPath
            )
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
            isRefreshing: $isRefreshingWorktrees
        )
    }

    private func run(_ command: ProjectCommand) {
        guard let created = appState.createProjectCommandTab(
            projectID: project.id,
            name: command.name,
            command: command.command
        )
        else { return }
        activeCommandID = command.id
        projectCommandStore.run(
            command,
            projectID: project.id,
            tabID: created.tabID,
            areaID: created.areaID,
            paneID: created.paneID
        )
    }

    private func activate(_ command: ProjectCommand) {
        activeCommandID = command.id
        guard let run = projectCommandStore.run(for: command.id, projectID: project.id) else { return }
        appState.selectTab(projectID: run.projectID, areaID: run.areaID, tabID: run.tabID)
    }

    private func restart(_ command: ProjectCommand) {
        guard let run = projectCommandStore.run(for: command.id, projectID: project.id) else {
            run(command)
            return
        }
        guard let replacement = appState.restartCommandTab(run, command: command) else { return }
        projectCommandStore.replaceRun(replacement)
    }

    private func requestStop(_ command: ProjectCommand) {
        guard let run = projectCommandStore.run(for: command.id, projectID: project.id) else { return }
        appState.interruptCommandTab(paneID: run.paneID)
        projectCommandStore.markStopped(command.id, projectID: project.id)
    }

    private func delete(_ command: ProjectCommand) {
        if activeCommandID == command.id {
            activeCommandID = nil
        }
        projectCommandStore.delete(command, from: project.id)
    }
}

private struct SidebarSectionHeader: View {
    let title: String
    let symbol: String
    let expanded: Bool
    let onToggle: () -> Void

    var body: some View {
        Button(action: onToggle) {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "chevron.down")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted.opacity(0.75))
                    .rotationEffect(.degrees(expanded ? 0 : -90))
                    .animation(.easeInOut(duration: 0.15), value: expanded)
                    .frame(width: UIMetrics.iconXS)

                Image(systemName: symbol)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.iconMD)

                Text(title.uppercased())
                    .font(.system(size: UIMetrics.fontMicro, weight: .bold))
                    .tracking(1.4)
                    .foregroundStyle(MuxyTheme.fgMuted)

                Rectangle()
                    .fill(MuxyTheme.border.opacity(0.8))
                    .frame(height: 1)
            }
        }
        .buttonStyle(.plain)
        .padding(.leading, UIMetrics.spacing3)
        .padding(.trailing, UIMetrics.spacing4)
        .padding(.top, UIMetrics.spacing2)
        .padding(.bottom, expanded ? UIMetrics.spacing3 : UIMetrics.spacing2)
        .accessibilityLabel(expanded ? "Collapse \(title)" : "Expand \(title)")
    }
}

private struct ProjectCommandRow: View {
    let command: ProjectCommand
    let run: ProjectCommandRun?
    let active: Bool
    let onActivate: () -> Void
    let onRun: () -> Void
    let onRestart: () -> Void
    let onStop: () -> Void
    let onDelete: () -> Void
    @State private var hovered = false

    private var isRunning: Bool {
        run?.state == .running
    }

    var body: some View {
        ProjectItem(
            title: command.name,
            color: active || isRunning ? MuxyTheme.accent : MuxyTheme.fg,
            onTap: onActivate,
            trailing: { hovered in
                Button(action: isRunning ? onStop : onRun) {
                    Image(systemName: isRunning ? "stop.fill" : "play.fill")
                        .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
                        .foregroundStyle(isRunning ? MuxyTheme.fg : MuxyTheme.accent)
                        .frame(width: UIMetrics.scaled(18), height: UIMetrics.scaled(18))
                }
                .buttonStyle(.plain)
                .opacity(hovered || isRunning ? 1 : 0)
                .accessibilityHidden(!hovered && !isRunning)
                .accessibilityLabel(isRunning ? "Stop Command" : "Run Command")
            }
        )
        .contextMenu {
            Button("Run", action: onRun)
            Button("Restart", action: onRestart)
                .disabled(run == nil)
            Button("Stop", action: onStop)
                .disabled(!isRunning)
            Divider()
            Button("Delete", role: .destructive, action: onDelete)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(command.name)
    }
}

@MainActor
private enum ProjectItemLayout {
    static var leadingPadding: CGFloat { UIMetrics.spacing9 }
    static var trailingPadding: CGFloat { UIMetrics.spacing4 }
}

private struct ProjectItem<Trailing: View>: View {
    let title: String
    var badge: String?
    var color: Color
    let onTap: () -> Void
    @ViewBuilder let trailing: (Bool) -> Trailing
    @State private var hovered = false

    var body: some View {
        HStack(alignment: .center, spacing: UIMetrics.spacing3) {
            HStack(spacing: UIMetrics.spacing2) {
                Text(title)
                    .font(.system(size: UIMetrics.fontBody, weight: .regular))
                    .foregroundStyle(color)
                    .lineLimit(1)
                    .truncationMode(.tail)

                if let badge {
                    ProjectItemBadge(title: badge)
                }
            }

            Spacer(minLength: UIMetrics.spacing1)

            trailing(hovered)
        }
        .padding(.leading, ProjectItemLayout.leadingPadding)
        .padding(.trailing, ProjectItemLayout.trailingPadding)
        .padding(.vertical, UIMetrics.scaled(5))
        .background(rowBackground, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .onHover { hovered = $0 }
        .onTapGesture(perform: onTap)
    }

    private var rowBackground: AnyShapeStyle {
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }
}

private struct ProjectItemBadge: View {
    let title: String

    var body: some View {
        Text(title)
            .font(.system(size: UIMetrics.fontMicro, weight: .bold))
            .tracking(0.4)
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, UIMetrics.spacing2)
            .padding(.vertical, UIMetrics.scaled(1))
            .background(MuxyTheme.surface, in: Capsule())
    }
}

private struct ExpandedAddCommandButton: View {
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "plus")
                    .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                Text("Add Command")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                Spacer()
            }
            .padding(.leading, ProjectItemLayout.leadingPadding)
            .padding(.trailing, ProjectItemLayout.trailingPadding)
            .padding(.vertical, UIMetrics.scaled(5))
            .background(hovered ? MuxyTheme.hover : Color.clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel("Add Command")
    }
}

private struct AddProjectCommandSheet: View {
    let onAdd: (String, String) -> Void
    let onLoadFromProject: () -> Void
    let onCancel: () -> Void
    @State private var name = ""
    @State private var command = ""
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing4) {
            Text("Add Command")
                .font(.system(size: UIMetrics.fontTitle, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            TextField("Name", text: $name)
                .textFieldStyle(.roundedBorder)

            TextField("Command", text: $command)
                .textFieldStyle(.roundedBorder)
                .focused($focused)
                .onSubmit { submit() }

            HStack {
                Button("Load from Project", action: onLoadFromProject)

                Spacer()
                Button("Cancel", action: onCancel)
                Button("Add", action: submit)
                    .disabled(command.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(UIMetrics.spacing6)
        .frame(width: UIMetrics.scaled(360))
        .onAppear { focused = true }
    }

    private func submit() {
        guard !command.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        onAdd(name, command)
    }
}

private struct ExpandedWorktreeRow: View {
    let projectID: UUID
    let worktree: Worktree
    let selected: Bool
    let onSelect: () -> Void
    let onRename: (String) -> Void
    let onRemove: (() -> Void)?

    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @FocusState private var renameFieldFocused: Bool

    private var displayName: String {
        if worktree.isPrimary, worktree.name.isEmpty { return "main" }
        return worktree.name
    }

    private var branchLabel: String? {
        guard !worktree.isPrimary else { return nil }
        guard let branch = worktree.branch, !branch.isEmpty else { return nil }
        guard branch.caseInsensitiveCompare(displayName) != .orderedSame else { return nil }
        return branch
    }

    var body: some View {
        Group {
            if isRenaming {
                HStack(spacing: UIMetrics.spacing3) {
                    leadingIndicator

                    TextField("", text: $renameText)
                        .textFieldStyle(.plain)
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                        .foregroundStyle(MuxyTheme.fg)
                        .focused($renameFieldFocused)
                        .onSubmit { commitRename() }
                        .onExitCommand { cancelRename() }

                    Spacer(minLength: UIMetrics.spacing1)
                }
                .padding(.horizontal, UIMetrics.spacing4)
                .padding(.vertical, UIMetrics.scaled(7))
                .background(rowBackground, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
                .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
                .onHover { hovered = $0 }
            } else {
                ProjectItem(
                    title: displayName,
                    color: itemColor,
                    onTap: onSelect,
                    trailing: { _ in EmptyView() }
                )
                .onHover { hovered = $0 }
            }
        }
        .contextMenu {
            if worktree.isPrimary {
                Text("Primary worktree").font(.system(size: UIMetrics.fontFootnote))
            } else if let onRemove {
                Button("Rename") { startRename() }
                Divider()
                Button("Remove", role: .destructive, action: onRemove)
            } else {
                Button("Rename") { startRename() }
                Divider()
                Text("External worktree").font(.system(size: UIMetrics.fontFootnote))
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(worktreeAccessibilityLabel)
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityAddTraits(.isButton)
    }

    private var worktreeAccessibilityLabel: String {
        var label = displayName
        if worktree.isPrimary { label += ", primary" }
        if let branch = branchLabel { label += ", branch: \(branch)" }
        return label
    }

    @ViewBuilder
    private var leadingIndicator: some View {
        let unread = NotificationStore.shared.unreadCount(for: projectID, worktreeID: worktree.id)
        ZStack {
            if unread > 0 {
                Circle().fill(MuxyTheme.accent).frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
            } else if selected {
                Circle().fill(MuxyTheme.accent.opacity(0.4)).frame(width: UIMetrics.scaled(5), height: UIMetrics.scaled(5))
            }
        }
        .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
    }

    private var itemColor: Color {
        if selected { return MuxyTheme.accent }
        return MuxyTheme.fg
    }

    private var rowBackground: AnyShapeStyle {
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }

    private func startRename() {
        renameText = worktree.name
        isRenaming = true
        renameFieldFocused = true
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespaces)
        if !trimmed.isEmpty { onRename(trimmed) }
        isRenaming = false
    }

    private func cancelRename() {
        isRenaming = false
    }
}

private struct ExpandedNewWorktreeButton: View {
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "plus")
                    .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                Text("New Worktree")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                Spacer()
            }
            .padding(.leading, ProjectItemLayout.leadingPadding)
            .padding(.trailing, ProjectItemLayout.trailingPadding)
            .padding(.vertical, UIMetrics.scaled(5))
            .background(hovered ? MuxyTheme.hover : Color.clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel("New Worktree")
    }
}

private struct ExpandedRenamePopover: View {
    @Binding var text: String
    let onCommit: () -> Void
    let onCancel: () -> Void
    @FocusState private var isFocused: Bool

    var body: some View {
        VStack(spacing: UIMetrics.spacing4) {
            Text("Rename Project")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            TextField("Project name", text: $text)
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

private struct IdentifiableExpandedImage: Identifiable {
    let id = UUID()
    let image: NSImage
}
