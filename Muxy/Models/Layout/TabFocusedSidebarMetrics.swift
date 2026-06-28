import Foundation

@MainActor
enum TabFocusedSidebarMetrics {
    static var rowHorizontalInset: CGFloat { UIMetrics.spacing4 }
    static var controlSlot: CGFloat { UIMetrics.scaled(18) }
}

enum TabFocusedSidebarPreferences {
    static func projectExpandedKey(_ projectID: UUID) -> String {
        "muxy.tabFocused.projectExpanded.\(projectID.uuidString)"
    }

    static func groupByWorktreeKey(_ projectID: UUID) -> String {
        "muxy.tabFocused.groupByWorktree.\(projectID.uuidString)"
    }

    static let focusModeKey = "muxy.tabFocused.focusMode"
}
