import SwiftUI

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
        switch info.mergeStateStatus {
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
            PRMergeabilityPresentation(text: "Yes", tone: .positive)
        case .unstable:
            unstablePresentation(checks: info.checks)
        case .unknown:
            unknownPresentation(mergeable: info.mergeable)
        }
    }

    private static func unstablePresentation(checks: GitRepositoryService.PRChecks) -> PRMergeabilityPresentation {
        switch checks.status {
        case .failure:
            PRMergeabilityPresentation(text: "Yes (checks failing)", tone: .warning)
        case .pending:
            PRMergeabilityPresentation(text: "Yes (checks running)", tone: .positive)
        case .none,
             .success:
            PRMergeabilityPresentation(text: "Yes", tone: .positive)
        }
    }

    private static func unknownPresentation(mergeable: Bool?) -> PRMergeabilityPresentation? {
        switch mergeable {
        case true: PRMergeabilityPresentation(text: "Yes", tone: .positive)
        case false: PRMergeabilityPresentation(text: "Conflicts", tone: .negative)
        default: nil
        }
    }
}
