import Foundation

@MainActor
enum OverviewSidebarLayout {
    static var defaultWidth: CGFloat { UIMetrics.scaled(260) }
    static var minWidth: CGFloat { UIMetrics.scaled(220) }
    static var maxWidth: CGFloat { UIMetrics.scaled(420) }

    static func clampWidth(_ value: CGFloat) -> CGFloat {
        min(max(value, minWidth), maxWidth)
    }
}

enum OverviewSidebarPreferences {
    static let visibleKey = "muxy.overviewSidebarVisible"
    static let widthKey = "muxy.overviewSidebarWidth"
    static let projectSectionExpandedKey = "muxy.overviewSection.project"
    static let gitSectionExpandedKey = "muxy.overviewSection.git"
    static let worktreesSectionExpandedKey = "muxy.overviewSection.worktrees"
    static let tabsSectionExpandedKey = "muxy.overviewSection.tabs"
}
