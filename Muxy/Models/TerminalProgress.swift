import Foundation

struct TerminalProgress: Equatable {
    enum Kind: Equatable {
        case set
        case error
        case indeterminate
        case paused
    }

    let kind: Kind
    let percent: Int?

    static func clamping(kind: Kind, percent: Int?) -> TerminalProgress {
        let normalized = percent.map { max(0, min(100, $0)) }
        return TerminalProgress(kind: kind, percent: normalized)
    }
}
