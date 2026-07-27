import Foundation

@MainActor
@Observable
final class AppLayoutStore {
    static let shared = AppLayoutStore()

    private let defaults: UserDefaults
    private(set) var layout: AppLayout

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        let stored = defaults.string(forKey: AppLayout.storageKey)
        layout = stored.flatMap(AppLayout.init(rawValue:)) ?? AppLayout.defaultValue
    }

    var provider: any AppLayoutProviding { layout.provider }

    func set(_ layout: AppLayout) {
        guard self.layout != layout else { return }
        self.layout = layout
        defaults.set(layout.rawValue, forKey: AppLayout.storageKey)
    }

    func toggle() {
        guard let index = AppLayout.allCases.firstIndex(of: layout) else {
            set(AppLayout.defaultValue)
            return
        }
        set(AppLayout.allCases[(index + 1) % AppLayout.allCases.count])
    }
}
