import Foundation

enum HookHealthDot: Equatable {
    case healthy
    case warning
    case error
    case idle
}

enum HookHealthPresenter {
    static let staleEventThreshold: TimeInterval = 24 * 60 * 60

    static func dot(for health: HookHealth, now: Date = Date()) -> HookHealthDot {
        switch health.installState {
        case .installed: installedDot(for: health, now: now)
        case .cliMissing: .warning
        case .conflict,
             .error: .error
        case .notInstalled: .idle
        }
    }

    private static func installedDot(for health: HookHealth, now: Date) -> HookHealthDot {
        guard let eventAt = health.lastEventAt else { return .healthy }
        guard now.timeIntervalSince(eventAt) < staleEventThreshold else { return .warning }
        return .healthy
    }

    static func statusLine(for health: HookHealth, now: Date = Date()) -> String {
        switch health.installState {
        case .installed:
            healthyLine(for: health, now: now)
        case .cliMissing:
            "CLI not installed"
        case let .conflict(message):
            message
        case let .error(message):
            message
        case .notInstalled:
            "Not installed"
        }
    }

    private static func healthyLine(for health: HookHealth, now: Date) -> String {
        if let repairedAt = health.lastRepairedAt, wasJustRepaired(repairedAt, verifiedAt: health.lastVerifiedAt) {
            return "Config overwritten — repaired \(relative(from: repairedAt, now: now))"
        }
        guard let eventAt = health.lastEventAt else {
            return "Hook healthy"
        }
        return "Hook healthy · last event \(relative(from: eventAt, now: now))"
    }

    private static func wasJustRepaired(_ repairedAt: Date, verifiedAt: Date?) -> Bool {
        guard let verifiedAt else { return true }
        return repairedAt >= verifiedAt.addingTimeInterval(-0.001)
    }

    static func relative(from date: Date, now: Date = Date()) -> String {
        let interval = max(0, now.timeIntervalSince(date))
        guard interval >= 60 else { return "just now" }
        let minutes = Int(interval / 60)
        guard minutes >= 60 else { return "\(minutes) min ago" }
        let hours = minutes / 60
        guard hours >= 24 else { return "\(hours) hr ago" }
        return "\(hours / 24) d ago"
    }
}
