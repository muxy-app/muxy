import Foundation

@MainActor
@Observable
final class TabFocusedSidebarState {
    static let shared = TabFocusedSidebarState()

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
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

    private var groupByWorktree: [UUID: Bool] = [:]

    func isGroupedByWorktree(_ projectID: UUID) -> Bool {
        if let value = groupByWorktree[projectID] { return value }
        let stored = defaults.bool(forKey: TabFocusedSidebarPreferences.groupByWorktreeKey(projectID))
        groupByWorktree[projectID] = stored
        return stored
    }

    func setGroupedByWorktree(_ projectID: UUID, grouped value: Bool) {
        groupByWorktree[projectID] = value
        defaults.set(value, forKey: TabFocusedSidebarPreferences.groupByWorktreeKey(projectID))
    }
}
