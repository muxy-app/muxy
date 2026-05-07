import AppKit
import SwiftUI

struct PullRequestsListView: View {
    @Bindable var state: VCSTabState
    let onCheckout: (GitRepositoryService.PRListItem) -> Void

    var body: some View {
        VStack(spacing: 0) {
            controlsBar
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            content
        }
    }

    private var controlsBar: some View {
        HStack(spacing: UIMetrics.spacing3) {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgDim)
                TextField("Search", text: $state.pullRequestSearchQuery)
                    .textFieldStyle(.plain)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fg)
                if !state.pullRequestSearchQuery.isEmpty {
                    Button {
                        state.pullRequestSearchQuery = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: UIMetrics.fontCaption))
                            .foregroundStyle(MuxyTheme.fgDim)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .frame(height: UIMetrics.scaled(22))
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))

            Menu {
                ForEach([
                    GitRepositoryService.PRListFilter.open,
                    .closed,
                    .merged,
                    .all,
                ], id: \.self) { option in
                    Button {
                        state.setPullRequestStateFilter(option)
                    } label: {
                        if state.pullRequestStateFilter == option {
                            Label(filterLabel(option), systemImage: "checkmark")
                        } else {
                            Text(filterLabel(option))
                        }
                    }
                }
            } label: {
                HStack(spacing: UIMetrics.scaled(3)) {
                    Text(filterLabel(state.pullRequestStateFilter))
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    Image(systemName: "chevron.down")
                        .font(.system(size: UIMetrics.fontMicro, weight: .bold))
                }
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.horizontal, UIMetrics.spacing3)
                .frame(height: UIMetrics.scaled(22))
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .padding(.vertical, UIMetrics.spacing3)
    }

    private func filterLabel(_ filter: GitRepositoryService.PRListFilter) -> String {
        switch filter {
        case .open: "Open"
        case .closed: "Closed"
        case .merged: "Merged"
        case .all: "All"
        }
    }

    @ViewBuilder
    private var content: some View {
        if !state.isGhInstalled {
            emptyState(
                icon: "exclamationmark.triangle",
                text: "GitHub CLI (gh) is not installed.\nInstall with: brew install gh"
            )
        } else if state.isLoadingPullRequests, state.pullRequests.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let error = state.pullRequestsLastError, state.pullRequests.isEmpty {
            emptyState(icon: "exclamationmark.triangle", text: error)
        } else if state.pullRequestsLastFetched == nil {
            unfetchedState
        } else if state.filteredPullRequests.isEmpty {
            emptyState(
                icon: "tray",
                text: state.pullRequestSearchQuery.isEmpty ? "No pull requests" : "No matches"
            )
        } else {
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(state.filteredPullRequests) { pr in
                        PullRequestRow(
                            pr: pr,
                            isCheckingOut: state.checkingOutPRNumber == pr.number,
                            onCheckout: { onCheckout(pr) }
                        )
                        Rectangle().fill(MuxyTheme.border).frame(height: 1)
                    }
                }
            }
        }
    }

    private var unfetchedState: some View {
        VStack(spacing: UIMetrics.spacing4) {
            Text("Pull requests not synced yet")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            Button {
                state.loadPullRequests()
            } label: {
                HStack(spacing: UIMetrics.spacing2) {
                    Image(systemName: "arrow.clockwise")
                        .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                    Text("Sync now")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                }
                .foregroundStyle(MuxyTheme.fg)
                .padding(.horizontal, UIMetrics.spacing5)
                .frame(height: UIMetrics.controlMedium)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
                .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusMD).stroke(MuxyTheme.border, lineWidth: 1))
            }
            .buttonStyle(.plain)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func emptyState(icon: String, text: String) -> some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: icon)
                .font(.system(size: UIMetrics.fontTitleLarge))
                .foregroundStyle(MuxyTheme.fgDim)
            Text(text)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(UIMetrics.spacing8)
    }
}

struct PullRequestRow: View {
    let pr: GitRepositoryService.PRListItem
    let isCheckingOut: Bool
    let onCheckout: () -> Void

    @State private var hovered = false

    var body: some View {
        HStack(spacing: UIMetrics.spacing4) {
            stateBadge
            VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                HStack(spacing: UIMetrics.spacing3) {
                    Text(pr.title)
                        .font(.system(size: UIMetrics.fontBody, weight: .medium))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                        .truncationMode(.tail)
                    Text("#\(pr.number)")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                HStack(spacing: UIMetrics.spacing2) {
                    Text(pr.author)
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Text("•")
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgDim)
                    Image(systemName: "arrow.triangle.branch")
                        .font(.system(size: UIMetrics.fontXS))
                        .foregroundStyle(MuxyTheme.fgDim)
                    Text("\(pr.headBranch) → \(pr.baseBranch)")
                        .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            Spacer(minLength: 0)
            checksBadge
            if hovered || isCheckingOut {
                checkoutButton
            }
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .frame(height: UIMetrics.scaled(44))
        .background(hovered ? MuxyTheme.surface : MuxyTheme.bg)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .onTapGesture(perform: onCheckout)
        .help("Checkout PR #\(pr.number)")
    }

    @ViewBuilder
    private var stateBadge: some View {
        let (symbol, color) = stateAppearance
        Image(systemName: symbol)
            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
            .foregroundStyle(color)
            .frame(width: UIMetrics.iconMD)
    }

    private var stateAppearance: (String, Color) {
        if pr.isDraft { return ("circle.dotted", MuxyTheme.fgMuted) }
        switch pr.state {
        case .open: return ("arrow.triangle.pull", MuxyTheme.diffAddFg)
        case .merged: return ("checkmark.circle.fill", MuxyTheme.accent)
        case .closed: return ("xmark.circle.fill", MuxyTheme.diffRemoveFg)
        }
    }

    @ViewBuilder
    private var checksBadge: some View {
        switch pr.checks.status {
        case .none:
            EmptyView()
        case .pending:
            Image(systemName: "clock")
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)
        case .success:
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.diffAddFg)
        case .failure:
            Image(systemName: "xmark.octagon.fill")
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.diffRemoveFg)
        }
    }

    private var checkoutButton: some View {
        Button(action: onCheckout) {
            HStack(spacing: UIMetrics.scaled(3)) {
                if isCheckingOut {
                    ProgressView().controlSize(.mini)
                } else {
                    Image(systemName: "arrow.down.to.line")
                        .font(.system(size: UIMetrics.fontXS, weight: .bold))
                }
                Text("Checkout")
                    .font(.system(size: UIMetrics.fontCaption, weight: .medium))
            }
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, UIMetrics.spacing4)
            .frame(height: UIMetrics.scaled(22))
            .background(MuxyTheme.bg, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .disabled(isCheckingOut)
    }
}

struct PullRequestsAutoSyncMenu: View {
    @Bindable var state: VCSTabState

    private static let options: [(minutes: Int, label: String)] = [
        (0, "Off"),
        (5, "Every 5 minutes"),
        (15, "Every 15 minutes"),
        (30, "Every 30 minutes"),
        (60, "Every hour"),
    ]

    var body: some View {
        Menu {
            ForEach(Self.options, id: \.minutes) { option in
                Button {
                    state.setPullRequestAutoSyncMinutes(option.minutes)
                } label: {
                    if state.pullRequestAutoSyncMinutes == option.minutes {
                        Label(option.label, systemImage: "checkmark")
                    } else {
                        Text(option.label)
                    }
                }
            }
        } label: {
            Image(systemName: state.pullRequestAutoSyncMinutes > 0 ? "clock.fill" : "clock")
                .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                .foregroundStyle(state.pullRequestAutoSyncMinutes > 0 ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .help(autoSyncHelp)
    }

    private var autoSyncHelp: String {
        if state.pullRequestAutoSyncMinutes == 0 { return "Auto-sync: off" }
        return "Auto-sync: every \(state.pullRequestAutoSyncMinutes) minute\(state.pullRequestAutoSyncMinutes == 1 ? "" : "s")"
    }
}
