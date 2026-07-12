import Foundation

@MainActor
@Observable
final class TabFocusedSidebarState {
    static let shared = TabFocusedSidebarState()

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        focusMode = defaults.bool(forKey: TabFocusedSidebarPreferences.focusModeKey)
    }

    private var expanded: [UUID: Bool] = [:]

    func isExpanded(_ projectID: UUID, default defaultValue: Bool) -> Bool {
        if let value = expanded[projectID] { return value }
        let key = TabFocusedSidebarPreferences.projectExpandedKey(projectID)
        if let stored = defaults.object(forKey: key) as? Bool {
            expanded[projectID] = stored
            return stored
        }
        return defaultValue
    }

    func set(_ projectID: UUID, expanded value: Bool) {
        expanded[projectID] = value
        defaults.set(value, forKey: TabFocusedSidebarPreferences.projectExpandedKey(projectID))
    }

    func isExpandedPersisted(_ projectID: UUID) -> Bool {
        if let value = expanded[projectID] { return value }
        return defaults.bool(forKey: TabFocusedSidebarPreferences.projectExpandedKey(projectID))
    }

    var focusMode: Bool {
        didSet {
            guard focusMode != oldValue else { return }
            defaults.set(focusMode, forKey: TabFocusedSidebarPreferences.focusModeKey)
        }
    }
}
