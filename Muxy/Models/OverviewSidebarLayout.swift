import Foundation

@MainActor
enum OverviewSidebarLayout {
    static var defaultWidth: CGFloat { UIMetrics.scaled(260) }
    static var minWidth: CGFloat { UIMetrics.scaled(220) }
    static var maxWidth: CGFloat { UIMetrics.scaled(420) }

    static var rowHorizontalInset: CGFloat { UIMetrics.spacing4 }
    static var controlSlot: CGFloat { UIMetrics.scaled(18) }

    static func clampWidth(_ value: CGFloat) -> CGFloat {
        min(max(value, minWidth), maxWidth)
    }
}

enum OverviewSidebarPreferences {
    static let visibleKey = "muxy.overviewSidebarVisible"
    static let widthKey = "muxy.overviewSidebarWidth"

    static func projectExpandedKey(_ projectID: UUID) -> String {
        "muxy.overviewProjectExpanded.\(projectID.uuidString)"
    }

    static func groupByWorktreeKey(_ projectID: UUID) -> String {
        "muxy.overviewGroupByWorktree.\(projectID.uuidString)"
    }

    static var isVisible: Bool {
        UserDefaults.standard.bool(forKey: visibleKey)
    }
}
