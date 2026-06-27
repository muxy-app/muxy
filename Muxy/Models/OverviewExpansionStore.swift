import Foundation

@MainActor
@Observable
final class OverviewExpansionStore {
    static let shared = OverviewExpansionStore()

    private var expanded: [UUID: Bool] = [:]

    func isExpanded(_ projectID: UUID, default defaultValue: Bool) -> Bool {
        if let value = expanded[projectID] { return value }
        let key = OverviewSidebarPreferences.projectExpandedKey(projectID)
        if let stored = UserDefaults.standard.object(forKey: key) as? Bool {
            expanded[projectID] = stored
            return stored
        }
        return defaultValue
    }

    func set(_ projectID: UUID, expanded value: Bool) {
        expanded[projectID] = value
        UserDefaults.standard.set(value, forKey: OverviewSidebarPreferences.projectExpandedKey(projectID))
    }

    func isExpandedPersisted(_ projectID: UUID) -> Bool {
        if let value = expanded[projectID] { return value }
        return UserDefaults.standard.bool(forKey: OverviewSidebarPreferences.projectExpandedKey(projectID))
    }

    private var groupByWorktree: [UUID: Bool] = [:]

    func isGroupedByWorktree(_ projectID: UUID) -> Bool {
        if let value = groupByWorktree[projectID] { return value }
        let stored = UserDefaults.standard.bool(forKey: OverviewSidebarPreferences.groupByWorktreeKey(projectID))
        groupByWorktree[projectID] = stored
        return stored
    }

    func setGroupedByWorktree(_ projectID: UUID, grouped value: Bool) {
        groupByWorktree[projectID] = value
        UserDefaults.standard.set(value, forKey: OverviewSidebarPreferences.groupByWorktreeKey(projectID))
    }
}
