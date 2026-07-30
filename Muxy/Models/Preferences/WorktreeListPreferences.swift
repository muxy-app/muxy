import Foundation

enum WorktreeListPreferences {
    static let showUnreadIndicatorKey = "muxy.worktrees.showUnreadIndicator"
    static let defaultShowUnreadIndicator = true

    static let orderByMRUKey = "muxy.worktrees.orderByMRU"
    static let defaultOrderByMRU = true

    static let groupWorktreesKey = "muxy.worktrees.groupWorktrees"
    static let defaultGroupWorktrees = false

    static var isGroupingEnabled: Bool {
        let defaults = UserDefaults.standard
        guard defaults.object(forKey: groupWorktreesKey) != nil else { return defaultGroupWorktrees }
        return defaults.bool(forKey: groupWorktreesKey)
    }
}
