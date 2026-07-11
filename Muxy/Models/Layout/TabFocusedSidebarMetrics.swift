import Foundation

@MainActor
enum TabFocusedSidebarMetrics {
    static var rowOuterInset: CGFloat { UIMetrics.spacing3 }
    static var rowHorizontalInset: CGFloat { UIMetrics.spacing3 }
    static var sectionHorizontalInset: CGFloat { rowOuterInset + rowHorizontalInset }
    static var rowCornerRadius: CGFloat { UIMetrics.radiusLG }
    static var projectRowHeight: CGFloat { UIMetrics.scaled(34) }
    static var tabRowHeight: CGFloat { UIMetrics.scaled(30) }
    static var tabRowIndent: CGFloat { UIMetrics.spacing9 }
    static var tabContentLeadingInset: CGFloat { UIMetrics.spacing3 }
    static var tabGuideLeading: CGFloat { rowOuterInset + rowHorizontalInset + UIMetrics.iconXL / 2 }
    static var activeRailWidth: CGFloat { UIMetrics.scaled(3) }
    static var controlSlot: CGFloat { UIMetrics.scaled(20) }
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
