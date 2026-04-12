import SwiftUI

struct PRPill: View {
    @Bindable var state: VCSTabState
    let onRequestCreate: () -> Void
    let onRequestMerge: (PRInfo, PRMergeMethod) -> Void
    let onRequestClose: (PRInfo) -> Void

    @State private var showPRPopover = false

    var body: some View {
        switch state.prLaunchState {
        case .hidden:
            EmptyView()
        case .ghMissing:
            ghMissingPill
        case .canCreate:
            createPRPill
        case let .hasPR(info):
            hasPRPill(info: info)
        }
    }

    private var ghMissingPill: some View {
        pillContainer(
            icon: "exclamationmark.triangle",
            text: "Install gh",
            tint: MuxyTheme.fgMuted,
            disabled: true
        ) {}
            .help("Install GitHub CLI to create pull requests: brew install gh")
    }

    private var createPRPill: some View {
        pillContainer(
            icon: "arrow.triangle.pull",
            text: "Create PR",
            tint: MuxyTheme.accent,
            disabled: state.isOpeningPullRequest,
            action: onRequestCreate
        )
        .help("Create a pull request")
    }

    private func hasPRPill(info: PRInfo) -> some View {
        Button {
            showPRPopover = true
        } label: {
            HStack(spacing: 4) {
                Image(systemName: prStateIcon(info))
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(prStateColor(info))
                Text("PR #\(info.number)")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg.opacity(0.85))
                Image(systemName: "chevron.down")
                    .font(.system(size: 8, weight: .bold))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
            .overlay(RoundedRectangle(cornerRadius: 5).stroke(prStateColor(info).opacity(0.35), lineWidth: 1))
            .contentShape(RoundedRectangle(cornerRadius: 5))
        }
        .buttonStyle(.plain)
        .help("Pull request #\(info.number)")
        .popover(isPresented: $showPRPopover, arrowEdge: .top) {
            PRPopover(
                state: state,
                info: info,
                onMerge: { method in
                    let needsConfirmation = state.hasAnyChanges
                        || info.checks.status == .failure
                        || info.checks.status == .pending
                    if needsConfirmation {
                        showPRPopover = false
                    }
                    onRequestMerge(info, method)
                },
                onClose: {
                    showPRPopover = false
                    onRequestClose(info)
                },
                onOpenInBrowser: {
                    showPRPopover = false
                    if let url = URL(string: info.url) {
                        NSWorkspace.shared.open(url)
                    }
                },
                onRefresh: {
                    state.refreshPullRequest()
                }
            )
        }
        .onChange(of: state.pullRequestInfo?.number) { _, number in
            if number == nil, showPRPopover {
                showPRPopover = false
            }
        }
    }

    private func prStateIcon(_ info: PRInfo) -> String {
        if info.state == .open {
            switch info.checks.status {
            case .failure: return "xmark.octagon.fill"
            case .pending: return "clock"
            default: break
            }
        }
        switch info.state {
        case .open: return info.isDraft ? "pencil.circle" : "arrow.triangle.pull"
        case .merged: return "checkmark.circle.fill"
        case .closed: return "xmark.circle"
        }
    }

    private func prStateColor(_ info: PRInfo) -> Color {
        if info.state == .open {
            switch info.checks.status {
            case .failure: return MuxyTheme.diffRemoveFg
            case .pending: return MuxyTheme.fgMuted
            default: break
            }
        }
        switch info.state {
        case .open: return info.isDraft ? MuxyTheme.fgMuted : MuxyTheme.diffAddFg
        case .merged: return MuxyTheme.accent
        case .closed: return MuxyTheme.diffRemoveFg
        }
    }

    private func pillContainer(
        icon: String,
        text: String,
        tint: Color,
        disabled: Bool,
        action: @escaping () -> Void = {}
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 4) {
                Image(systemName: icon)
                    .font(.system(size: 9, weight: .bold))
                Text(text)
                    .font(.system(size: 10, weight: .semibold))
            }
            .foregroundStyle(tint)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
            .overlay(RoundedRectangle(cornerRadius: 5).stroke(tint.opacity(0.35), lineWidth: 1))
            .contentShape(RoundedRectangle(cornerRadius: 5))
        }
        .buttonStyle(.plain)
        .disabled(disabled)
    }
}
