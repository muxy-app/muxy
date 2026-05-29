import SwiftUI

struct WorktreePopover: View {
    let project: Project
    let isGitRepo: Bool
    let onDismiss: () -> Void
    let onRequestCreate: () -> Void
    let onRequestRemove: (Worktree) -> Void
    var fixedSize: Bool = true

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var pendingRemoval: WorktreeRemovalConfirmation?

    private var worktrees: [Worktree] {
        worktreeStore.list(for: project.id)
    }

    private var activeWorktreeID: UUID? {
        appState.activeWorktreeID[project.id]
    }

    var body: some View {
        PopoverPicker(
            items: worktrees,
            filterKey: { worktree in
                worktree.name + " " + (worktree.branch ?? "")
            },
            searchPlaceholder: "Search worktrees…",
            emptyLabel: "No matches",
            footerActions: footerActions,
            fixedSize: fixedSize,
            onSelect: { worktree in
                appState.selectWorktree(projectID: project.id, worktree: worktree)
                onDismiss()
            },
            row: { worktree, isHighlighted in
                WorktreePopoverRow(
                    worktree: worktree,
                    selected: worktree.id == activeWorktreeID,
                    isHighlighted: isHighlighted,
                    onSelect: {
                        appState.selectWorktree(projectID: project.id, worktree: worktree)
                        onDismiss()
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
                .padding(.horizontal, UIMetrics.spacing3)
                .padding(.vertical, UIMetrics.spacing1)
            }
        )
        .alert(
            pendingRemoval?.title ?? "",
            isPresented: removalAlertBinding,
            presenting: pendingRemoval
        ) { confirmation in
            Button("Remove", role: .destructive) {
                onRequestRemove(confirmation.worktree)
                pendingRemoval = nil
            }
            .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) {
                pendingRemoval = nil
            }
            .keyboardShortcut(.cancelAction)
        } message: { confirmation in
            Text(confirmation.message)
        }
    }

    private var footerActions: [PopoverFooterAction] {
        guard isGitRepo else { return [] }
        return [
            PopoverFooterAction(
                title: "New Worktree…",
                icon: "plus.square.dashed",
                action: onRequestCreate
            ),
        ]
    }

    @MainActor
    private func requestRemove(worktree: Worktree) async {
        let hasChanges = await GitWorktreeService.shared.hasUncommittedChanges(worktreePath: worktree.path)
        pendingRemoval = WorktreeRemovalConfirmation(
            worktree: worktree,
            hasUncommittedChanges: hasChanges
        )
    }

    private var removalAlertBinding: Binding<Bool> {
        Binding(
            get: { pendingRemoval != nil },
            set: { newValue in
                if !newValue {
                    pendingRemoval = nil
                }
            }
        )
    }
}

private struct WorktreePopoverRow: View {
    let worktree: Worktree
    let selected: Bool
    let isHighlighted: Bool
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

    private var branchSubtitle: String? {
        guard let branch = worktree.branch, !branch.isEmpty else { return nil }
        guard branch.caseInsensitiveCompare(displayName) != .orderedSame else { return nil }
        return branch
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing5) {
            indicator
            VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
                if isRenaming {
                    TextField("", text: $renameText)
                        .textFieldStyle(.plain)
                        .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                        .focused($renameFieldFocused)
                        .onSubmit { commitRename() }
                        .onExitCommand { cancelRename() }
                } else {
                    HStack(spacing: UIMetrics.spacing3) {
                        Text(displayName)
                            .font(.system(size: UIMetrics.fontBody, weight: selected ? .semibold : .medium))
                            .foregroundStyle(selected ? MuxyTheme.fg : MuxyTheme.fg.opacity(0.9))
                            .lineLimit(1)
                            .truncationMode(.middle)
                        if worktree.isPrimary {
                            Text("PRIMARY")
                                .font(.system(size: UIMetrics.fontMicro, weight: .bold))
                                .tracking(0.5)
                                .foregroundStyle(MuxyTheme.fgDim)
                                .padding(.horizontal, UIMetrics.spacing2)
                                .padding(.vertical, UIMetrics.scaled(1))
                                .background(MuxyTheme.surface, in: Capsule())
                        }
                    }
                }
                if let branch = branchSubtitle, !isRenaming {
                    Text(branch)
                        .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            Spacer(minLength: 4)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.scaled(7))
        .background(rowBackground, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .onHover { hovered = $0 }
        .onTapGesture {
            guard !isRenaming else { return }
            onSelect()
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
            }
        }
    }

    private var indicator: some View {
        ZStack {
            Circle()
                .fill(selected ? MuxyTheme.accent : MuxyTheme.fgDim.opacity(0.35))
                .frame(width: UIMetrics.scaled(7), height: UIMetrics.scaled(7))
        }
        .frame(width: UIMetrics.scaled(10))
    }

    private var rowBackground: AnyShapeStyle {
        if selected { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if isHighlighted { return AnyShapeStyle(MuxyTheme.surface) }
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
