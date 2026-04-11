import SwiftUI

struct ProjectRow: View {
    let project: Project
    let shortcutIndex: Int?
    let isAnyDragging: Bool
    let onSelect: () -> Void
    let onRemove: () -> Void
    let onRename: (String) -> Void

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore

    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @FocusState private var renameFieldFocused: Bool
    @State private var showWorktreePopover = false
    @State private var isGitRepo = false
    @State private var showCreateWorktreeSheet = false

    private var isActive: Bool {
        appState.activeProjectID == project.id
    }

    private var worktrees: [Worktree] {
        worktreeStore.list(for: project.id)
    }

    private var activeWorktree: Worktree? {
        guard isActive, let id = appState.activeWorktreeID[project.id] else { return nil }
        return worktrees.first { $0.id == id }
    }

    private var showWorktreeTrigger: Bool {
        isActive
    }

    private var triggerLabel: String {
        guard let activeWorktree else { return "main" }
        if activeWorktree.isPrimary {
            if let branch = activeWorktree.branch, !branch.isEmpty { return branch }
            return "main"
        }
        return activeWorktree.name
    }

    var body: some View {
        HStack(spacing: 8) {
            label
            Spacer(minLength: 0)
            trailingAccessory
        }
        .padding(.horizontal, 10)
        .frame(height: 28)
        .background(background, in: RoundedRectangle(cornerRadius: 6))
        .overlay(alignment: .leading) {
            if isActive {
                UnevenRoundedRectangle(
                    topLeadingRadius: 6,
                    bottomLeadingRadius: 6,
                    bottomTrailingRadius: 0,
                    topTrailingRadius: 0
                )
                .fill(MuxyTheme.accent)
                .frame(width: 3)
            }
        }
        .contentShape(RoundedRectangle(cornerRadius: 6))
        .onHover { hovering in
            guard !isAnyDragging else { return }
            hovered = hovering
        }
        .onChange(of: isAnyDragging) { _, dragging in
            if dragging { hovered = false }
        }
        .onTapGesture {
            guard !isAnyDragging, !isRenaming else { return }
            onSelect()
        }
        .task(id: project.path) {
            isGitRepo = await GitWorktreeService.shared.isGitRepository(project.path)
        }
        .contextMenu {
            Button("Rename Project") { startRename() }
            if isGitRepo {
                Divider()
                Button("New Worktree…") { showCreateWorktreeSheet = true }
                if worktrees.count > 1 {
                    Button("Switch Worktree…") { showWorktreePopover = true }
                }
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
    }

    @ViewBuilder
    private var label: some View {
        if isRenaming {
            TextField("", text: $renameText)
                .textFieldStyle(.plain)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
                .focused($renameFieldFocused)
                .onSubmit { commitRename() }
                .onExitCommand { cancelRename() }
        } else {
            Text(project.name)
                .font(.system(size: 12, weight: isActive ? .semibold : .medium))
                .foregroundStyle(nameColor)
                .lineLimit(1)
                .truncationMode(.middle)
        }
    }

    @ViewBuilder
    private var trailingAccessory: some View {
        if showShortcutBadge, let shortcutIndex,
           let action = ShortcutAction.projectAction(for: shortcutIndex)
        {
            ShortcutBadge(label: KeyBindingStore.shared.combo(for: action).displayString)
        } else if showWorktreeTrigger, !isRenaming {
            Button {
                showWorktreePopover = true
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "arrow.triangle.branch")
                        .font(.system(size: 9, weight: .semibold))
                    Text(triggerLabel)
                        .font(.system(size: 10, weight: .medium))
                        .lineLimit(1)
                        .fixedSize(horizontal: true, vertical: false)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 8, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                .foregroundStyle(MuxyTheme.fg.opacity(0.85))
                .padding(.horizontal, 6)
                .padding(.vertical, 3)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
                .contentShape(RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)
            .help("Switch Worktree")
            .popover(isPresented: $showWorktreePopover, arrowEdge: .trailing) {
                WorktreePopover(
                    project: project,
                    isGitRepo: isGitRepo,
                    onDismiss: { showWorktreePopover = false },
                    onRequestCreate: {
                        showWorktreePopover = false
                        showCreateWorktreeSheet = true
                    }
                )
                .environment(appState)
                .environment(worktreeStore)
            }
        }
    }

    private var background: AnyShapeStyle {
        if isActive { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }

    private var nameColor: Color {
        if isActive { return MuxyTheme.fg }
        if hovered { return MuxyTheme.fg }
        return MuxyTheme.fgMuted
    }

    private var showShortcutBadge: Bool {
        guard let shortcutIndex,
              let action = ShortcutAction.projectAction(for: shortcutIndex)
        else { return false }
        return ModifierKeyMonitor.shared.isHolding(
            modifiers: KeyBindingStore.shared.combo(for: action).modifiers
        )
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
        renameFieldFocused = true
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
}
