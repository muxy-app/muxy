import SwiftUI

struct TabFocusedBranchPopover: View {
    let summary: GitRepositorySummary
    let branches: [String]
    let isLoadingBranches: Bool
    let isMutatingBranches: Bool
    let branchBeingDeleted: String?
    let isRepositoryInteractionDisabled: Bool
    let onSwitch: (String) -> Void
    let onCreate: (String) async -> Bool
    let onDelete: (String) async -> Bool

    @State private var pendingDeletion: String?
    @State private var isCreatingBranch = false
    @State private var newBranchName = ""
    @State private var isSubmittingNewBranch = false
    @State private var isNewBranchButtonHovered = false
    @State private var searchFocusRequest = 0
    @FocusState private var isNewBranchFieldFocused: Bool

    private struct BranchItem: Identifiable {
        let name: String
        var id: String { name }
    }

    private var items: [BranchItem] {
        branches.map { BranchItem(name: $0) }
    }

    private var trimmedNewBranchName: String {
        newBranchName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var isInteractionDisabled: Bool {
        isMutatingBranches || isRepositoryInteractionDisabled
    }

    private var canCreateBranch: Bool {
        !trimmedNewBranchName.isEmpty
            && !branches.contains(trimmedNewBranchName)
            && !isInteractionDisabled
            && !isSubmittingNewBranch
            && pendingDeletion == nil
    }

    var body: some View {
        VStack(spacing: 0) {
            branchPicker
            Divider().overlay(MuxyTheme.border)
            branchCreationFooter
        }
        .frame(width: UIMetrics.scaled(300), height: UIMetrics.scaled(400))
        .background(MuxyTheme.bg)
        .onChange(of: branches) { _, branches in
            if let pendingDeletion, !branches.contains(pendingDeletion) {
                self.pendingDeletion = nil
            }
        }
        .onDisappear {
            pendingDeletion = nil
            isCreatingBranch = false
        }
    }

    @ViewBuilder
    private var branchPicker: some View {
        if isLoadingBranches, branches.isEmpty {
            ProgressView()
                .controlSize(.small)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            SearchableListPicker(
                items: items,
                filterKey: { $0.name },
                placeholder: "Search branches…",
                emptyLabel: "No branches",
                selectsRowOnTap: false,
                isSearchDisabled: branchBeingDeleted != nil,
                searchFocusRequest: searchFocusRequest,
                onSearchChange: { _ in cancelDeletion() },
                onEscape: cancelDeletion,
                onSelect: select,
                row: { item, isHighlighted in
                    BranchPopoverRow(
                        name: item.name,
                        isSelected: item.name == summary.branch,
                        isHighlighted: isHighlighted,
                        isDeletionPending: pendingDeletion == item.name,
                        isDeleting: branchBeingDeleted == item.name,
                        isInteractionDisabled: isInteractionDisabled,
                        hasActiveInlineAction: pendingDeletion != nil || isCreatingBranch,
                        onSelect: { select(item) },
                        onRequestDelete: { requestDeletion(item.name) },
                        onConfirmDelete: { confirmDeletion(item.name) },
                        onCancelDelete: cancelDeletion
                    )
                }
            )
        }
    }

    @ViewBuilder
    private var branchCreationFooter: some View {
        if isCreatingBranch {
            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                HStack(spacing: UIMetrics.spacing2) {
                    Image(systemName: "arrow.triangle.branch")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.accent)
                    Text("New branch")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                }
                HStack(spacing: UIMetrics.spacing2) {
                    TextField("feature/name", text: $newBranchName)
                        .textFieldStyle(.plain)
                        .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fg)
                        .padding(.horizontal, UIMetrics.spacing3)
                        .frame(height: UIMetrics.controlSmall)
                        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                        .focused($isNewBranchFieldFocused)
                        .disabled(isInteractionDisabled || isSubmittingNewBranch)
                        .onSubmit(createBranch)
                        .onExitCommand(perform: cancelBranchCreation)
                    Button("Cancel", action: cancelBranchCreation)
                        .buttonStyle(.plain)
                        .font(.system(size: UIMetrics.fontXS, weight: .medium))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .disabled(isSubmittingNewBranch)
                        .keyboardShortcut(.cancelAction)
                    Button(action: createBranch) {
                        Group {
                            if isSubmittingNewBranch {
                                ProgressView().controlSize(.mini)
                            } else {
                                Text("Create")
                            }
                        }
                        .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                        .foregroundStyle(canCreateBranch ? MuxyTheme.accent : MuxyTheme.fgDim)
                    }
                    .buttonStyle(.plain)
                    .disabled(!canCreateBranch)
                    .keyboardShortcut(.defaultAction)
                    .accessibilityLabel("Create and switch to branch")
                }
            }
            .padding(.horizontal, UIMetrics.spacing5)
            .padding(.vertical, UIMetrics.spacing4)
        } else {
            Button(action: beginBranchCreation) {
                HStack(spacing: UIMetrics.spacing3) {
                    Image(systemName: "plus")
                        .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                        .frame(width: UIMetrics.iconSM)
                    Text("New branch")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    Spacer(minLength: UIMetrics.spacing3)
                }
                .foregroundStyle(isInteractionDisabled ? MuxyTheme.fgDim : MuxyTheme.accent)
                .padding(.horizontal, UIMetrics.spacing4)
                .frame(height: UIMetrics.controlMedium)
                .background(
                    isNewBranchButtonHovered && !isInteractionDisabled ? MuxyTheme.hover : .clear,
                    in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                )
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(isInteractionDisabled || pendingDeletion != nil)
            .onHover { isNewBranchButtonHovered = $0 }
            .help("Create a branch from the current HEAD")
            .accessibilityLabel("Create new branch")
            .padding(UIMetrics.spacing3)
        }
    }

    private func select(_ item: BranchItem) {
        guard item.name != summary.branch,
              pendingDeletion == nil,
              !isCreatingBranch,
              !isInteractionDisabled
        else { return }
        onSwitch(item.name)
    }

    private func requestDeletion(_ branch: String) {
        guard branch != summary.branch,
              !isCreatingBranch,
              !isInteractionDisabled
        else { return }
        pendingDeletion = branch
    }

    private func cancelDeletion() {
        guard branchBeingDeleted == nil, pendingDeletion != nil else { return }
        pendingDeletion = nil
        requestSearchFocus()
    }

    private func confirmDeletion(_ branch: String) {
        guard pendingDeletion == branch, !isInteractionDisabled else { return }
        Task {
            _ = await onDelete(branch)
            if pendingDeletion == branch {
                pendingDeletion = nil
                requestSearchFocus()
            }
        }
    }

    private func beginBranchCreation() {
        guard !isInteractionDisabled, pendingDeletion == nil else { return }
        newBranchName = ""
        isCreatingBranch = true
        Task { @MainActor in
            await Task.yield()
            isNewBranchFieldFocused = true
        }
    }

    private func cancelBranchCreation() {
        guard !isSubmittingNewBranch else { return }
        isCreatingBranch = false
        newBranchName = ""
        requestSearchFocus()
    }

    private func createBranch() {
        guard canCreateBranch else { return }
        let branch = trimmedNewBranchName
        isSubmittingNewBranch = true
        Task {
            let created = await onCreate(branch)
            isSubmittingNewBranch = false
            guard created else {
                isNewBranchFieldFocused = true
                return
            }
            newBranchName = ""
            isCreatingBranch = false
            requestSearchFocus()
        }
    }

    private func requestSearchFocus() {
        searchFocusRequest &+= 1
    }
}

private struct BranchPopoverRow: View {
    let name: String
    let isSelected: Bool
    let isHighlighted: Bool
    let isDeletionPending: Bool
    let isDeleting: Bool
    let isInteractionDisabled: Bool
    let hasActiveInlineAction: Bool
    let onSelect: () -> Void
    let onRequestDelete: () -> Void
    let onConfirmDelete: () -> Void
    let onCancelDelete: () -> Void

    @State private var isHovered = false

    var body: some View {
        Group {
            if isDeletionPending {
                deletionConfirmation
            } else {
                branchContent
            }
        }
        .padding(.horizontal, UIMetrics.spacing3)
        .frame(height: isDeletionPending ? UIMetrics.scaled(34) : UIMetrics.controlMedium)
        .background(rowBackground, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .padding(.horizontal, UIMetrics.spacing3)
        .padding(.vertical, UIMetrics.spacing1)
        .onHover { isHovered = $0 }
    }

    private var branchContent: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Button(action: onSelect) {
                HStack(spacing: UIMetrics.spacing3) {
                    Image(systemName: "arrow.triangle.branch")
                        .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                        .foregroundStyle(isSelected ? MuxyTheme.accent : MuxyTheme.fgMuted)
                        .frame(width: UIMetrics.iconSM)
                    Text(name)
                        .font(.system(
                            size: UIMetrics.fontFootnote,
                            weight: isSelected ? .semibold : .regular,
                            design: .monospaced
                        ))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer(minLength: UIMetrics.spacing1)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(isInteractionDisabled || hasActiveInlineAction || isSelected)
            .accessibilityLabel(isSelected ? "Current branch \(name)" : "Switch to branch \(name)")

            trailingAccessory
        }
    }

    @ViewBuilder
    private var trailingAccessory: some View {
        if isSelected {
            Image(systemName: "checkmark")
                .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
                .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                .accessibilityHidden(true)
        } else {
            Button(action: onRequestDelete) {
                Image(systemName: "trash")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                    .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .opacity(showsDeleteAction ? 1 : 0)
            .allowsHitTesting(showsDeleteAction)
            .disabled(isInteractionDisabled || hasActiveInlineAction)
            .help("Delete branch \(name)")
            .accessibilityLabel("Delete branch \(name)")
        }
    }

    private var deletionConfirmation: some View {
        HStack(spacing: UIMetrics.spacing2) {
            VStack(alignment: .leading, spacing: 0) {
                Text("Delete \(name)?")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Text("Unmerged commits may be permanently lost")
                    .font(.system(size: UIMetrics.fontXS))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Spacer(minLength: UIMetrics.spacing1)
            Button("Cancel", action: onCancelDelete)
                .buttonStyle(.plain)
                .font(.system(size: UIMetrics.fontXS, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
                .disabled(isDeleting)
                .keyboardShortcut(.cancelAction)
            Button(action: onConfirmDelete) {
                HStack(spacing: UIMetrics.spacing2) {
                    if isDeleting {
                        ProgressView().controlSize(.mini)
                    } else {
                        Image(systemName: "trash")
                    }
                    Text(isDeleting ? "Deleting…" : "Delete")
                }
                .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                .foregroundStyle(MuxyTheme.diffRemoveFg)
                .frame(minWidth: UIMetrics.scaled(38))
            }
            .buttonStyle(.plain)
            .disabled(isDeleting || isInteractionDisabled)
            .help("Permanently delete branch \(name). Unmerged commits may be lost.")
            .accessibilityLabel("Permanently delete branch \(name). Unmerged commits may be lost.")
        }
        .accessibilityElement(children: .contain)
    }

    private var showsDeleteAction: Bool {
        !isSelected
            && !isInteractionDisabled
            && !hasActiveInlineAction
            && (isHovered || isHighlighted)
    }

    private var rowBackground: AnyShapeStyle {
        if isDeletionPending { return AnyShapeStyle(MuxyTheme.diffRemoveBg) }
        if isHovered { return AnyShapeStyle(MuxyTheme.hover) }
        if isSelected { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if isHighlighted { return AnyShapeStyle(MuxyTheme.surface) }
        return AnyShapeStyle(Color.clear)
    }
}
