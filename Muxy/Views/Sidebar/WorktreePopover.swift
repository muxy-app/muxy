import AppKit
import SwiftUI

struct WorktreePopover: View {
    let project: Project
    let isGitRepo: Bool
    let onDismiss: () -> Void
    let onRequestCreate: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore

    @State private var isRefreshing = false

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
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
            }
        )
    }

    private var footerActions: [PopoverFooterAction] {
        guard isGitRepo else { return [] }
        return [
            PopoverFooterAction(
                title: "Refresh Worktrees",
                icon: "arrow.clockwise",
                isBusy: isRefreshing,
                action: {
                    Task {
                        await WorktreeRefreshHelper.refresh(
                            project: project,
                            appState: appState,
                            worktreeStore: worktreeStore,
                            isRefreshing: $isRefreshing
                        )
                    }
                }
            ),
            PopoverFooterAction(
                title: "New Worktree…",
                icon: "plus.square.dashed",
                action: onRequestCreate
            ),
        ]
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
        HStack(spacing: 10) {
            indicator
            VStack(alignment: .leading, spacing: 1) {
                if isRenaming {
                    TextField("", text: $renameText)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                        .focused($renameFieldFocused)
                        .onSubmit { commitRename() }
                        .onExitCommand { cancelRename() }
                } else {
                    HStack(spacing: 6) {
                        Text(displayName)
                            .font(.system(size: 12, weight: selected ? .semibold : .medium))
                            .foregroundStyle(selected ? MuxyTheme.fg : MuxyTheme.fg.opacity(0.9))
                            .lineLimit(1)
                            .truncationMode(.middle)
                        if worktree.isPrimary {
                            Text("PRIMARY")
                                .font(.system(size: 8, weight: .bold))
                                .tracking(0.5)
                                .foregroundStyle(MuxyTheme.fgDim)
                                .padding(.horizontal, 4)
                                .padding(.vertical, 1)
                                .background(MuxyTheme.surface, in: Capsule())
                        }
                    }
                }
                if let branch = branchSubtitle, !isRenaming {
                    Text(branch)
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            Spacer(minLength: 4)
            if selected {
                Image(systemName: "checkmark")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(MuxyTheme.accent)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(rowBackground, in: RoundedRectangle(cornerRadius: 6))
        .contentShape(RoundedRectangle(cornerRadius: 6))
        .onHover { hovered = $0 }
        .onTapGesture {
            guard !isRenaming else { return }
            onSelect()
        }
        .contextMenu {
            if worktree.isPrimary {
                Text("Primary worktree").font(.system(size: 11))
            } else if let onRemove {
                Button("Rename") { startRename() }
                Divider()
                Button("Remove", role: .destructive, action: onRemove)
            } else {
                Button("Rename") { startRename() }
                Divider()
                Text("External worktree").font(.system(size: 11))
            }
        }
    }

    private var indicator: some View {
        ZStack {
            Circle()
                .fill(selected ? MuxyTheme.accent : MuxyTheme.fgDim.opacity(0.35))
                .frame(width: 7, height: 7)
        }
        .frame(width: 10)
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
