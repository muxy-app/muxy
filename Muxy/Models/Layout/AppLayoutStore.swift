import Foundation

@MainActor
@Observable
final class AppLayoutStore {
    static let shared = AppLayoutStore()

    private let defaults: UserDefaults
    private let storageKey: String
    private(set) var layout: AppLayout

    init(defaults: UserDefaults = .standard, storageKey: String = AppLayout.storageKey) {
        self.defaults = defaults
        self.storageKey = storageKey
        let stored = defaults.string(forKey: storageKey)
        layout = stored.flatMap(AppLayout.init(rawValue:)) ?? AppLayout.defaultValue
    }

    var provider: any AppLayoutProviding { layout.provider }

    func set(_ layout: AppLayout) {
        guard self.layout != layout else { return }
        self.layout = layout
        defaults.set(layout.rawValue, forKey: storageKey)
    }

    func toggle() {
        guard let index = AppLayout.allCases.firstIndex(of: layout) else {
            set(AppLayout.defaultValue)
            return
        }
        set(AppLayout.allCases[(index + 1) % AppLayout.allCases.count])
    }
}
