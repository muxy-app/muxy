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

    @MainActor
    static func statusLine(for health: HookHealth, now: Date = Date()) -> String {
        switch health.installState {
        case .installed:
            healthyLine(for: health, now: now)
        case .cliMissing:
            L10n.string("CLI not installed")
        case let .conflict(message):
            message
        case let .error(message):
            message
        case .notInstalled:
            L10n.string("Not installed")
        }
    }

    @MainActor
    private static func healthyLine(for health: HookHealth, now: Date) -> String {
        if let repairedAt = health.lastRepairedAt, wasJustRepaired(repairedAt, verifiedAt: health.lastVerifiedAt) {
            return L10n.string("Config overwritten — repaired \(relative(from: repairedAt, now: now))")
        }
        guard let eventAt = health.lastEventAt else {
            return L10n.string("Hook healthy")
        }
        return L10n.string("Hook healthy · last event \(relative(from: eventAt, now: now))")
    }

    private static func wasJustRepaired(_ repairedAt: Date, verifiedAt: Date?) -> Bool {
        guard let verifiedAt else { return true }
        return repairedAt >= verifiedAt.addingTimeInterval(-0.001)
    }

    @MainActor
    static func relative(from date: Date, now: Date = Date()) -> String {
        let interval = max(0, now.timeIntervalSince(date))
        guard interval >= 60 else { return L10n.string("just now") }
        let minutes = Int(interval / 60)
        guard minutes >= 60 else { return L10n.string("\(minutes) min ago") }
        let hours = minutes / 60
        guard hours >= 24 else { return L10n.string("\(hours) hr ago") }
        return L10n.string("\(hours / 24) d ago")
    }
}
