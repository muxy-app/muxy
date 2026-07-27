import Foundation

enum TabFocusedSidebarRowTap: Equatable {
    case toggleExpansion
    case activateRow

    static func resolve(content: TabFocusedSidebarContent, isActive: Bool) -> TabFocusedSidebarRowTap {
        guard content == .agents, !isActive else { return .toggleExpansion }
        return .activateRow
    }
}
