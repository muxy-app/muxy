import Foundation

@MainActor
enum TabFocusedSidebarMetrics {
    static var rowOuterInset: CGFloat { UIMetrics.spacing3 }
    static var rowVerticalPadding: CGFloat { UIMetrics.spacing1 }
    static var rowHorizontalInset: CGFloat { UIMetrics.spacing3 }
    static var rowCornerRadius: CGFloat { UIMetrics.radiusLG }
    static var rowHeight: CGFloat { UIMetrics.scaled(32) }
    static var folderIconSize: CGFloat { UIMetrics.iconLG }
    static var iconTitleGap: CGFloat { UIMetrics.spacing3 }
    static var tabContentLeadingInset: CGFloat { rowHorizontalInset + folderIconSize + iconTitleGap }
    static var paneTreeLeadingInset: CGFloat { tabContentLeadingInset + UIMetrics.iconSM + UIMetrics.spacing2 }
    static var paneRowHeight: CGFloat { UIMetrics.scaled(26) }
    static var activeRailWidth: CGFloat { UIMetrics.scaled(3) }
    static var controlSlot: CGFloat { UIMetrics.scaled(20) }
}

enum TabFocusedSidebarPreferences {
    static func projectExpandedKey(_ projectID: UUID) -> String {
        "muxy.tabFocused.projectExpanded.\(projectID.uuidString)"
    }

    static let focusModeKey = "muxy.tabFocused.focusMode"
}
