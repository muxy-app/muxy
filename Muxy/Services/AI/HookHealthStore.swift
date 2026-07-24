import Foundation

enum HookInstallState: Equatable {
    case installed
    case notInstalled
    case cliMissing
    case conflict(String)
    case error(String)
}

struct HookHealth: Equatable {
    var installState: HookInstallState = .notInstalled
    var lastVerifiedAt: Date?
    var lastRepairedAt: Date?
    var lastEventAt: Date?
    var lastError: String?
}

@MainActor
@Observable
final class HookHealthStore {
    static let shared = HookHealthStore()

    private(set) var health: [String: HookHealth] = [:]

    private let now: () -> Date

    init(now: @escaping () -> Date = { Date() }) {
        self.now = now
    }

    func health(for providerID: String) -> HookHealth {
        health[providerID] ?? HookHealth()
    }

    func noteVerified(providerID: String, state: HookInstallState) {
        var entry = health[providerID] ?? HookHealth()
        entry.installState = state
        entry.lastVerifiedAt = now()
        entry.lastError = Self.errorMessage(for: state)
        health[providerID] = entry
    }

    func noteRepaired(providerID: String, state: HookInstallState) {
        var entry = health[providerID] ?? HookHealth()
        entry.installState = state
        entry.lastVerifiedAt = now()
        entry.lastRepairedAt = now()
        entry.lastError = Self.errorMessage(for: state)
        health[providerID] = entry
    }

    func noteEvent(providerID: String) {
        var entry = health[providerID] ?? HookHealth()
        entry.lastEventAt = now()
        health[providerID] = entry
    }

    func reset(providerID: String) {
        health[providerID] = HookHealth()
    }

    private static func errorMessage(for state: HookInstallState) -> String? {
        switch state {
        case let .conflict(message): message
        case let .error(message): message
        case .installed,
             .notInstalled,
             .cliMissing: nil
        }
    }
}
