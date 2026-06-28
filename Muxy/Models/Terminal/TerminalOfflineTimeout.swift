import Foundation

enum TerminalOfflineTimeout: String, CaseIterable, Identifiable {
    case tenSeconds = "10 seconds"
    case thirtySeconds = "30 seconds"
    case oneMinute = "1 minute"
    case twoMinutes = "2 minutes"
    case fiveMinutes = "5 minutes"
    case tenMinutes = "10 minutes"
    case fifteenMinutes = "15 minutes"
    case thirtyMinutes = "30 minutes"

    var id: String { rawValue }

    var seconds: TimeInterval {
        switch self {
        case .tenSeconds: 10
        case .thirtySeconds: 30
        case .oneMinute: 60
        case .twoMinutes: 120
        case .fiveMinutes: 300
        case .tenMinutes: 600
        case .fifteenMinutes: 900
        case .thirtyMinutes: 1800
        }
    }

    static func closest(to seconds: TimeInterval) -> TerminalOfflineTimeout {
        allCases.min(by: { abs($0.seconds - seconds) < abs($1.seconds - seconds) }) ?? .fiveMinutes
    }
}
