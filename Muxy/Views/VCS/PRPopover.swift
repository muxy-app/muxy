import SwiftUI

struct PRPopover: View {
    @Bindable var state: VCSTabState
    let info: PRInfo
    let onMerge: (PRMergeMethod) -> Void
    let onClose: () -> Void
    let onOpenInBrowser: () -> Void
    let onRefresh: () -> Void

    @State private var mergeMethod: PRMergeMethod = .squash

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: stateIcon)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(stateColor)
                VStack(alignment: .leading, spacing: 1) {
                    Text("Pull Request #\(info.number)")
                        .font(.system(size: 12, weight: .semibold))
                    Text(stateLabel)
                        .font(.system(size: 10))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
                Spacer(minLength: 0)
                Button {
                    onRefresh()
                } label: {
                    Image(systemName: "arrow.clockwise")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .frame(width: 20, height: 20)
                }
                .buttonStyle(.plain)
                .help("Refresh")
            }

            VStack(alignment: .leading, spacing: 4) {
                infoRow(label: "Base", value: info.baseBranch)
                if let mergeable = info.mergeable {
                    infoRow(
                        label: "Mergeable",
                        value: mergeable ? "Yes" : "Conflicts",
                        valueColor: mergeable ? MuxyTheme.diffAddFg : MuxyTheme.diffRemoveFg
                    )
                }
                checksRow
            }

            Divider()

            Button(action: onOpenInBrowser) {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.up.right.square")
                        .font(.system(size: 11, weight: .semibold))
                    Text("Open on GitHub")
                        .font(.system(size: 11, weight: .medium))
                    Spacer(minLength: 0)
                }
                .foregroundStyle(MuxyTheme.fg)
                .padding(.horizontal, 8)
                .padding(.vertical, 6)
                .frame(maxWidth: .infinity)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)

            if info.state == .open {
                Picker("Method", selection: $mergeMethod) {
                    ForEach(PRMergeMethod.allCases) { method in
                        Text(method.shortLabel).tag(method)
                    }
                }
                .pickerStyle(.segmented)
                .labelsHidden()

                Button { onMerge(mergeMethod) } label: {
                    HStack(spacing: 6) {
                        if state.isMergingPullRequest {
                            ProgressView().controlSize(.mini)
                        } else {
                            Image(systemName: "arrow.triangle.merge")
                                .font(.system(size: 11, weight: .bold))
                        }
                        Text(state.isMergingPullRequest ? "Merging…" : mergeMethod.label)
                            .font(.system(size: 11, weight: .medium))
                        Spacer(minLength: 0)
                    }
                    .foregroundStyle(mergeDisabled ? MuxyTheme.fgDim : MuxyTheme.bg)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .frame(maxWidth: .infinity)
                    .background(
                        mergeDisabled ? MuxyTheme.surface : MuxyTheme.accent,
                        in: RoundedRectangle(cornerRadius: 5)
                    )
                }
                .buttonStyle(.plain)
                .disabled(mergeDisabled)
                .help(mergeHelp)

                Button(action: onClose) {
                    HStack(spacing: 6) {
                        if state.isClosingPullRequest {
                            ProgressView().controlSize(.mini)
                        } else {
                            Image(systemName: "xmark.circle")
                                .font(.system(size: 11, weight: .semibold))
                        }
                        Text("Close PR")
                            .font(.system(size: 11, weight: .medium))
                        Spacer(minLength: 0)
                    }
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .frame(maxWidth: .infinity)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 5))
                }
                .buttonStyle(.plain)
                .disabled(state.isClosingPullRequest)
            }
        }
        .padding(12)
        .frame(width: 260)
        .task(id: info.number) {
            await pollLoop()
        }
    }

    private func pollLoop() async {
        var intervalSeconds: UInt64 = 5
        let maxIntervalSeconds: UInt64 = 60
        while !Task.isCancelled {
            do {
                try await Task.sleep(nanoseconds: intervalSeconds * 1_000_000_000)
            } catch {
                return
            }
            guard state.pullRequestInfo?.state == .open,
                  !state.isMergingPullRequest,
                  !state.isClosingPullRequest
            else { return }
            onRefresh()
            intervalSeconds = min(intervalSeconds * 2, maxIntervalSeconds)
        }
    }

    private var mergeDisabled: Bool {
        if state.isMergingPullRequest { return true }
        if info.mergeable == false { return true }
        return false
    }

    private var mergeHelp: String {
        if info.mergeable == false { return "This PR has conflicts and cannot be merged." }
        if info.checks.status == .failure { return "Checks are failing. You will be asked to confirm before merging." }
        if info.checks.status == .pending { return "Checks are still running. You will be asked to confirm before merging." }
        return "Merge PR #\(info.number)"
    }

    @ViewBuilder
    private var checksRow: some View {
        switch info.checks.status {
        case .none:
            EmptyView()
        case .success:
            infoRow(
                label: "Checks",
                value: "\(info.checks.passing)/\(info.checks.total) passing",
                valueColor: MuxyTheme.diffAddFg
            )
        case .pending:
            infoRow(
                label: "Checks",
                value: "\(info.checks.pending) running",
                valueColor: MuxyTheme.fgMuted
            )
        case .failure:
            infoRow(
                label: "Checks",
                value: "\(info.checks.failing) failing",
                valueColor: MuxyTheme.diffRemoveFg
            )
        }
    }

    private var stateIcon: String {
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

    private var stateColor: Color {
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

    private var stateLabel: String {
        switch info.state {
        case .open: info.isDraft ? "Draft · Open" : "Open"
        case .merged: "Merged"
        case .closed: "Closed"
        }
    }

    private func infoRow(label: String, value: String, valueColor: Color = MuxyTheme.fg) -> some View {
        HStack(spacing: 6) {
            Text(label)
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 70, alignment: .leading)
            Text(value)
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundStyle(valueColor)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer(minLength: 0)
        }
    }
}
