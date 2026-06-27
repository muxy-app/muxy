import Foundation

@MainActor
@Observable
final class AppLayoutStore {
    static let shared = AppLayoutStore()

    private(set) var layout: AppLayout

    init() {
        let stored = UserDefaults.standard.string(forKey: AppLayout.storageKey)
        layout = stored.flatMap(AppLayout.init(rawValue:)) ?? AppLayout.defaultValue
    }

    var provider: any AppLayoutProviding { layout.provider }

    func set(_ layout: AppLayout) {
        guard self.layout != layout else { return }
        self.layout = layout
        UserDefaults.standard.set(layout.rawValue, forKey: AppLayout.storageKey)
    }

    func toggle() {
        set(layout == .projectFocused ? .tabFocused : .projectFocused)
    }
}
