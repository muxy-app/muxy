import SwiftUI

struct WorktreeBranchPicker: View {
    let project: Project
    let isGitRepo: Bool
    let currentBranch: String?
    let branches: [String]
    let isLoadingBranches: Bool
    let activeWorktree: Worktree?
    let onSelectBranch: (String) -> Void
    let onRefreshBranches: () -> Void
    let onCreateBranch: () -> Void
    let onDeleteBranch: (String) -> Void
    let onRequestCreateWorktree: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore

    @State private var showPopover = false
    @State private var segment: Segment = .worktrees

    enum Segment: String, CaseIterable, Identifiable {
        case worktrees
        case branches
        var id: String { rawValue }
        var title: String {
            switch self {
            case .worktrees: "Worktrees"
            case .branches: "Branches"
            }
        }
    }

    private var worktreeLabel: String {
        guard let worktree = activeWorktree else { return "default" }
        if worktree.isPrimary {
            return worktree.name.isEmpty ? "default" : worktree.name
        }
        return worktree.name
    }

    private var branchLabel: String {
        currentBranch ?? "detached"
    }

    var body: some View {
        Button(action: open) {
            HStack(spacing: 4) {
                Image(systemName: "square.stack.3d.up")
                    .font(.system(size: 9, weight: .semibold))
                Text(worktreeLabel)
                    .font(.system(size: 10, weight: .medium))
                    .lineLimit(1)
                    .truncationMode(.tail)
                Image(systemName: "chevron.right")
                    .font(.system(size: 7, weight: .bold))
                    .foregroundStyle(MuxyTheme.fgDim)
                Image(systemName: "arrow.triangle.branch")
                    .font(.system(size: 9, weight: .semibold))
                Text(branchLabel)
                    .font(.system(size: 10, weight: .medium))
                    .lineLimit(1)
                    .truncationMode(.tail)
                Image(systemName: "chevron.down")
                    .font(.system(size: 8, weight: .bold))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
            .frame(maxWidth: 160, alignment: .leading)
            .foregroundStyle(MuxyTheme.fg.opacity(0.85))
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
            .contentShape(RoundedRectangle(cornerRadius: 5))
        }
        .buttonStyle(.plain)
        .help("\(worktreeLabel) › \(branchLabel)")
        .accessibilityLabel("Worktree \(worktreeLabel), Branch \(branchLabel)")
        .accessibilityHint("Opens worktree and branch picker")
        .popover(isPresented: $showPopover, arrowEdge: .top) {
            popoverContent
        }
    }

    private func open() {
        if segment == .branches {
            onRefreshBranches()
        }
        showPopover = true
    }

    private var segmentedHeader: some View {
        HStack(spacing: 0) {
            ForEach(Segment.allCases) { item in
                segmentButton(item)
            }
        }
        .frame(height: 32)
    }

    private func segmentButton(_ item: Segment) -> some View {
        let isActive = segment == item
        return Button {
            segment = item
            if item == .branches {
                onRefreshBranches()
            }
        } label: {
            Text(item.title)
                .font(.system(size: 11, weight: isActive ? .semibold : .medium))
                .foregroundStyle(isActive ? MuxyTheme.fg : MuxyTheme.fgDim)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .overlay(alignment: .bottom) {
                    Rectangle()
                        .fill(isActive ? MuxyTheme.accent : Color.clear)
                        .frame(height: 2)
                }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var popoverContent: some View {
        VStack(spacing: 0) {
            segmentedHeader
            Divider().overlay(MuxyTheme.border.opacity(0.55))

            Group {
                switch segment {
                case .worktrees:
                    WorktreePopover(
                        project: project,
                        isGitRepo: isGitRepo,
                        onDismiss: { showPopover = false },
                        onRequestCreate: {
                            showPopover = false
                            onRequestCreateWorktree()
                        },
                        fixedSize: false
                    )
                case .branches:
                    BranchPickerContent(
                        currentBranch: currentBranch,
                        branches: branches,
                        isLoading: isLoadingBranches,
                        fixedSize: false,
                        onSelect: { branch in
                            showPopover = false
                            onSelectBranch(branch)
                        },
                        onCreateBranch: {
                            showPopover = false
                            onCreateBranch()
                        },
                        onDeleteBranch: { branch in
                            showPopover = false
                            onDeleteBranch(branch)
                        }
                    )
                }
            }
        }
        .frame(width: 320, height: 460)
        .background(MuxyTheme.bg)
    }
}
