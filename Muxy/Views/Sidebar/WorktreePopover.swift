import AppKit
import SwiftUI

struct WorktreePopover: View {
    let project: Project
    let isGitRepo: Bool
    let onDismiss: () -> Void
    let onRequestCreate: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var searchText = ""

    private var worktrees: [Worktree] {
        worktreeStore.list(for: project.id)
    }

    private var filteredWorktrees: [Worktree] {
        guard !searchText.isEmpty else { return worktrees }
        return worktrees.filter { worktree in
            worktree.name.localizedCaseInsensitiveContains(searchText)
                || (worktree.branch?.localizedCaseInsensitiveContains(searchText) ?? false)
        }
    }

    private var activeWorktreeID: UUID? {
        appState.activeWorktreeID[project.id]
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(MuxyTheme.border.opacity(0.55))
            if worktrees.count > 4 {
                searchField
                Divider().overlay(MuxyTheme.border.opacity(0.55))
            }
            if filteredWorktrees.isEmpty {
                emptyResults
            } else {
                worktreeList
            }
            if isGitRepo {
                Divider().overlay(MuxyTheme.border.opacity(0.55))
                newWorktreeButton
            }
        }
        .frame(width: 300)
        .frame(maxHeight: 420)
        .background(MuxyTheme.bg)
    }

    private var header: some View {
        HStack(spacing: 6) {
            Text("WORKTREES")
                .font(.system(size: 10, weight: .bold))
                .tracking(0.6)
                .foregroundStyle(MuxyTheme.fgDim)
            Text("·")
                .font(.system(size: 10))
                .foregroundStyle(MuxyTheme.fgDim)
            Text(project.name)
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.top, 10)
        .padding(.bottom, 8)
    }

    private var searchField: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
            ZStack(alignment: .leading) {
                if searchText.isEmpty {
                    Text("Search worktrees…")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                TextField("", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fg)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private var emptyResults: some View {
        Text("No matches")
            .font(.system(size: 12))
            .foregroundStyle(MuxyTheme.fgMuted)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 24)
    }

    private var worktreeList: some View {
        ScrollView {
            LazyVStack(spacing: 2) {
                ForEach(filteredWorktrees) { worktree in
                    WorktreePopoverRow(
                        worktree: worktree,
                        selected: worktree.id == activeWorktreeID,
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
                        onRemove: worktree.isPrimary ? nil : {
                            Task { await requestRemove(worktree: worktree) }
                        }
                    )
                }
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 6)
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

    private var newWorktreeButton: some View {
        Button(action: onRequestCreate) {
            HStack(spacing: 8) {
                Image(systemName: "plus.square.dashed")
                    .font(.system(size: 12))
                Text("New Worktree…")
                    .font(.system(size: 12, weight: .medium))
                Spacer()
            }
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

private struct WorktreePopoverRow: View {
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
                if let branch = worktree.branch, !branch.isEmpty, !isRenaming {
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
            if let onRemove {
                Button("Rename") { startRename() }
                Divider()
                Button("Remove", role: .destructive, action: onRemove)
            } else {
                Text("Primary worktree").font(.system(size: 11))
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
