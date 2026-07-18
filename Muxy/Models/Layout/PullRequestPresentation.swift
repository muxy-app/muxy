import SwiftUI

enum RepositoryToolbarPresentation {
    enum WorktreeRemovalState: Equatable {
        case hidden
        case available
        case preparing
        case removing
    }

    static func branchLabel(summary: GitRepositorySummary?, worktree: Worktree?) -> String {
        if let summary {
            return summary.displayBranch
        }
        guard let branch = worktree?.branch, !branch.isEmpty else { return "Branch" }
        return branch
    }

    static func worktreeRemovalState(
        worktree: Worktree?,
        isPreparing: Bool,
        isRemoving: Bool
    ) -> WorktreeRemovalState {
        guard let worktree, worktree.canBeRemoved else { return .hidden }
        if isRemoving {
            return .removing
        }
        return isPreparing ? .preparing : .available
    }
}

struct PRMergeabilityPresentation: Equatable {
    enum Tone {
        case positive
        case negative
        case warning
        case muted
    }

    let text: String
    let tone: Tone

    @MainActor var color: Color {
        switch tone {
        case .positive: MuxyTheme.diffAddFg
        case .negative: MuxyTheme.diffRemoveFg
        case .warning: MuxyTheme.warning
        case .muted: MuxyTheme.fgMuted
        }
    }

    static func make(info: GitRepositoryService.PRInfo) -> PRMergeabilityPresentation? {
        if info.isDraft {
            return PRMergeabilityPresentation(text: "Draft", tone: .muted)
        }
        return switch info.mergeStateStatus {
        case .dirty:
            PRMergeabilityPresentation(text: "Conflicts", tone: .negative)
        case .behind:
            PRMergeabilityPresentation(text: "Behind base", tone: .negative)
        case .blocked:
            PRMergeabilityPresentation(text: "Blocked", tone: .negative)
        case .draft:
            PRMergeabilityPresentation(text: "Draft", tone: .muted)
        case .clean,
             .hasHooks:
            PRMergeabilityPresentation(text: "Ready", tone: .positive)
        case .unstable:
            unstablePresentation(checks: info.checks)
        case .unknown:
            unknownPresentation(mergeable: info.mergeable)
        }
    }

    private static func unstablePresentation(
        checks: GitRepositoryService.PRChecks
    ) -> PRMergeabilityPresentation {
        switch checks.status {
        case .failure:
            PRMergeabilityPresentation(text: "Checks failing", tone: .warning)
        case .pending:
            PRMergeabilityPresentation(text: "Checks running", tone: .warning)
        case .none,
             .success:
            PRMergeabilityPresentation(text: "Ready", tone: .positive)
        }
    }

    private static func unknownPresentation(mergeable: Bool?) -> PRMergeabilityPresentation? {
        switch mergeable {
        case true: PRMergeabilityPresentation(text: "Ready", tone: .positive)
        case false: PRMergeabilityPresentation(text: "Conflicts", tone: .negative)
        default: nil
        }
    }
}

struct PRMergeAvailability: Equatable {
    let isEnabled: Bool
    let help: String

    static func make(info: GitRepositoryService.PRInfo) -> PRMergeAvailability {
        guard info.state == .open else {
            return PRMergeAvailability(isEnabled: false, help: "Only open pull requests can be merged.")
        }
        guard !info.isDraft else {
            return PRMergeAvailability(
                isEnabled: false,
                help: "Mark this pull request ready for review before merging."
            )
        }
        guard info.mergeable != false else {
            return PRMergeAvailability(isEnabled: false, help: "This PR has conflicts and cannot be merged.")
        }
        switch info.mergeStateStatus {
        case .dirty:
            return PRMergeAvailability(isEnabled: false, help: "This PR has conflicts and cannot be merged.")
        case .behind:
            return PRMergeAvailability(
                isEnabled: false,
                help: "This branch is out of date with the base branch. Update it before merging."
            )
        case .blocked:
            return PRMergeAvailability(
                isEnabled: false,
                help: "Merging is blocked by branch protection, required reviews, or checks."
            )
        case .draft:
            return PRMergeAvailability(
                isEnabled: false,
                help: "Mark this pull request ready for review before merging."
            )
        case .clean,
             .hasHooks,
             .unstable,
             .unknown:
            return PRMergeAvailability(isEnabled: true, help: confirmationHelp(info: info))
        }
    }

    private static func confirmationHelp(info: GitRepositoryService.PRInfo) -> String {
        switch info.checks.status {
        case .failure: "Checks are failing. Click to start the five-second merge confirmation."
        case .pending: "Checks are still running. Click to start the five-second merge confirmation."
        case .none,
             .success: "Start the five-second confirmation to merge PR #\(info.number)."
        }
    }
}

struct PullRequestActionConfirmation: Equatable {
    struct Context: Equatable {
        let repositoryID: String
        let branch: String
        let headOID: String?
        let pullRequest: GitRepositoryService.PRInfo
    }

    enum Kind: Equatable {
        case merge(GitRepositoryService.PRMergeMethod)
        case close
    }

    struct Pending: Equatable {
        let id: UUID
        let kind: Kind

        init(id: UUID = UUID(), kind: Kind) {
            self.id = id
            self.kind = kind
        }
    }

    struct State: Equatable {
        private(set) var pending: Pending?

        mutating func arm(_ kind: Kind) -> Pending {
            let pending = Pending(kind: kind)
            self.pending = pending
            return pending
        }

        mutating func cancel() {
            pending = nil
        }
    }

    enum Activation: Equatable {
        case arm(Kind)
        case confirm
    }

    static let duration: TimeInterval = 5

    static func activation(pending: Pending?, requested: Kind) -> Activation {
        guard pending?.kind == requested else { return .arm(requested) }
        return .confirm
    }
}

enum PullRequestPresentation {
    static func symbol(for info: GitRepositoryService.PRInfo) -> String {
        if info.state == .open {
            switch info.checks.status {
            case .failure: return "xmark.octagon.fill"
            case .pending: return "clock"
            case .none,
                 .success: break
            }
        }
        switch info.state {
        case .open: return info.isDraft ? "pencil.circle" : "arrow.triangle.pull"
        case .merged: return "checkmark.circle.fill"
        case .closed: return "xmark.circle"
        }
    }

    @MainActor
    static func color(for info: GitRepositoryService.PRInfo) -> Color {
        if info.state == .open {
            switch info.checks.status {
            case .failure: return MuxyTheme.diffRemoveFg
            case .pending: return MuxyTheme.warning
            case .none,
                 .success: break
            }
        }
        switch info.state {
        case .open: return info.isDraft ? MuxyTheme.fgMuted : MuxyTheme.diffAddFg
        case .merged: return MuxyTheme.accent
        case .closed: return MuxyTheme.diffRemoveFg
        }
    }

    static func stateLabel(for info: GitRepositoryService.PRInfo) -> String {
        switch info.state {
        case .open: info.isDraft ? "Draft · Open" : "Open"
        case .merged: "Merged"
        case .closed: "Closed"
        }
    }

    static func checksLabel(for checks: GitRepositoryService.PRChecks) -> String? {
        switch checks.status {
        case .none: nil
        case .success: "\(checks.passing)/\(checks.total) passing"
        case .pending: "\(checks.pending) running"
        case .failure: "\(checks.failing) failing"
        }
    }
}
