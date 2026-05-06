import AppKit
import SwiftUI

struct CommitHistoryView: View {
    @Bindable var state: VCSTabState
    @State private var branchNameInput = ""
    @State private var tagNameInput = ""
    @State private var pendingBranchHash: String?
    @State private var pendingTagHash: String?

    var body: some View {
        ScrollView {
            historyContent
        }
    }

    @ViewBuilder
    private var historyContent: some View {
        if state.isLoadingCommits, state.commits.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity)
                .padding(UIMetrics.spacing8)
        } else if state.commits.isEmpty {
            Text("No commits")
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(maxWidth: .infinity)
                .padding(.vertical, UIMetrics.spacing8)
        } else {
            commitList
        }
    }

    private var commitList: some View {
        LazyVStack(spacing: 0) {
            ForEach(state.commits) { commit in
                CommitRow(
                    commit: commit,
                    currentBranch: state.branchName,
                    onCheckout: { state.switchBranch($0) },
                    onCheckoutDetached: { state.checkoutDetached($0) },
                    onCherryPick: { state.cherryPick($0) },
                    onRevert: { state.revert($0, subject: $1) },
                    onCreateBranch: { pendingBranchHash = $0 },
                    onCreateTag: { pendingTagHash = $0 }
                )
            }

            if state.hasMoreCommits {
                Button {
                    state.loadMoreCommits()
                } label: {
                    if state.isLoadingCommits {
                        ProgressView()
                            .controlSize(.small)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, UIMetrics.spacing5)
                    } else {
                        Text("Load more")
                            .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                            .foregroundStyle(MuxyTheme.accent)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, UIMetrics.spacing5)
                    }
                }
                .buttonStyle(.plain)
            }
        }
        .sheet(item: Binding(
            get: { pendingBranchHash.map { NamePrompt(hash: $0) } },
            set: { pendingBranchHash = $0?.hash }
        )) { prompt in
            NameInputSheet(
                title: "Create Branch",
                placeholder: "Branch name",
                actionTitle: "Create",
                name: $branchNameInput,
                onSubmit: {
                    let trimmedName = branchNameInput.trimmingCharacters(in: .whitespacesAndNewlines)
                    state.createBranch(name: trimmedName, from: prompt.hash)
                    branchNameInput = ""
                    pendingBranchHash = nil
                },
                onCancel: {
                    branchNameInput = ""
                    pendingBranchHash = nil
                }
            )
        }
        .sheet(item: Binding(
            get: { pendingTagHash.map { NamePrompt(hash: $0) } },
            set: { pendingTagHash = $0?.hash }
        )) { prompt in
            NameInputSheet(
                title: "Create Tag",
                placeholder: "Tag name",
                actionTitle: "Create",
                name: $tagNameInput,
                onSubmit: {
                    let trimmedName = tagNameInput.trimmingCharacters(in: .whitespacesAndNewlines)
                    state.createTag(name: trimmedName, at: prompt.hash)
                    tagNameInput = ""
                    pendingTagHash = nil
                },
                onCancel: {
                    tagNameInput = ""
                    pendingTagHash = nil
                }
            )
        }
    }
}

private struct NamePrompt: Identifiable {
    let hash: String
    var id: String { hash }
}

private struct CommitRow: View {
    let commit: GitCommit
    let currentBranch: String?
    let onCheckout: (String) -> Void
    let onCheckoutDetached: (String) -> Void
    let onCherryPick: (String) -> Void
    let onRevert: (String, String) -> Void
    let onCreateBranch: (String) -> Void
    let onCreateTag: (String) -> Void
    @State private var hovered = false

    private var dotColor: Color {
        if commit.isMerge {
            return MuxyTheme.accent
        }
        if commit.refs.contains(where: { $0.kind == .localBranch }) {
            return MuxyTheme.accent
        }
        if commit.refs.contains(where: { $0.kind == .tag }) {
            return MuxyTheme.diffRemoveFg
        }
        return MuxyTheme.fgMuted
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing4) {
            commitDot

            VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                Text(commit.subject)
                    .font(.system(size: UIMetrics.fontBody, weight: .regular))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.tail)

                HStack(spacing: UIMetrics.spacing3) {
                    if !commit.refs.isEmpty {
                        refBadges
                    }

                    Text(commit.authorName)
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .lineLimit(1)

                    Text(relativeDate(commit.authorDate))
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
            }

            Spacer(minLength: 0)

            if hovered {
                Text(commit.shortHash)
                    .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .padding(.trailing, UIMetrics.spacing1)
            }
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .frame(height: UIMetrics.scaled(40))
        .background(hovered ? MuxyTheme.hover : .clear)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .contextMenu { contextMenuItems }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(commitAccessibilityLabel)
    }

    private var commitAccessibilityLabel: String {
        var parts = [commit.subject]
        parts.append("by \(commit.authorName)")
        parts.append(relativeDate(commit.authorDate))
        if commit.isMerge { parts.append("merge commit") }
        let refNames = commit.refs.map(\.name)
        if !refNames.isEmpty { parts.append("refs: \(refNames.joined(separator: ", "))") }
        parts.append(commit.shortHash)
        return parts.joined(separator: ", ")
    }

    private var commitDot: some View {
        Circle()
            .fill(commit.isMerge ? .clear : dotColor)
            .stroke(dotColor, lineWidth: commit.isMerge ? 1.5 : 0)
            .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
    }

    private var refBadges: some View {
        ForEach(Array(commit.refs.enumerated()), id: \.offset) { _, ref in
            refBadge(ref)
        }
    }

    private func refBadge(_ ref: GitRef) -> some View {
        let color: Color = switch ref.kind {
        case .head,
             .localBranch:
            MuxyTheme.accent
        case .remoteBranch:
            MuxyTheme.diffAddFg
        case .tag:
            MuxyTheme.diffRemoveFg
        }

        let icon = switch ref.kind {
        case .head,
             .localBranch:
            "arrow.triangle.branch"
        case .remoteBranch:
            "cloud"
        case .tag:
            "tag"
        }

        return HStack(spacing: UIMetrics.spacing1) {
            Image(systemName: icon)
                .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
            Text(ref.name)
                .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                .lineLimit(1)
        }
        .foregroundStyle(color)
        .padding(.horizontal, UIMetrics.scaled(5))
        .padding(.vertical, UIMetrics.scaled(1))
        .background(color.opacity(0.12), in: RoundedRectangle(cornerRadius: UIMetrics.scaled(3)))
    }

    @ViewBuilder
    private var contextMenuItems: some View {
        Button("Copy Commit Hash") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(commit.hash, forType: .string)
        }

        Button("Copy Commit Message") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(commit.subject, forType: .string)
        }

        Divider()

        if let localBranch = commit.refs.first(where: { $0.kind == .localBranch }),
           localBranch.name != currentBranch
        {
            Button("Checkout \(localBranch.name)") {
                onCheckout(localBranch.name)
            }
        }

        Button("Checkout (Detached)") {
            onCheckoutDetached(commit.hash)
        }

        Divider()

        Button("Create Branch...") {
            onCreateBranch(commit.hash)
        }

        Button("Create Tag...") {
            onCreateTag(commit.hash)
        }

        Divider()

        Button("Cherry Pick") {
            onCherryPick(commit.hash)
        }

        Button("Revert Commit") {
            onRevert(commit.hash, commit.subject)
        }
    }
}

private func relativeDate(_ date: Date) -> String {
    let now = Date()
    let interval = now.timeIntervalSince(date)

    guard interval > 0 else { return "just now" }

    let minute: TimeInterval = 60
    let hour: TimeInterval = 3600
    let day: TimeInterval = 86400
    let week: TimeInterval = 604_800
    let month: TimeInterval = 2_592_000
    let year: TimeInterval = 31_536_000

    if interval < minute {
        return "just now"
    } else if interval < hour {
        let m = Int(interval / minute)
        return "\(m)m ago"
    } else if interval < day {
        let h = Int(interval / hour)
        return "\(h)h ago"
    } else if interval < week {
        let d = Int(interval / day)
        return "\(d)d ago"
    } else if interval < month {
        let w = Int(interval / week)
        return "\(w)w ago"
    } else if interval < year {
        let m = Int(interval / month)
        return "\(m)mo ago"
    } else {
        let y = Int(interval / year)
        return "\(y)y ago"
    }
}

private struct NameInputSheet: View {
    let title: String
    let placeholder: String
    let actionTitle: String
    @Binding var name: String
    let onSubmit: () -> Void
    let onCancel: () -> Void

    private var isValid: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        VStack(spacing: UIMetrics.spacing7) {
            Text(title)
                .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            TextField(placeholder, text: $name)
                .textFieldStyle(.roundedBorder)
                .onSubmit {
                    guard isValid else { return }
                    onSubmit()
                }

            HStack(spacing: UIMetrics.spacing4) {
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)

                Button(actionTitle) {
                    onSubmit()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!isValid)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(300))
    }
}
