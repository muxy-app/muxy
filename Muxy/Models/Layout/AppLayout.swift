import Foundation

enum AppLayout: String, CaseIterable, Identifiable {
    case projectFocused
    case tabFocused
    case agentsFocused

    var id: String { rawValue }

    var title: String {
        switch self {
        case .projectFocused: "Project Focused"
        case .tabFocused: "Tab Focused"
        case .agentsFocused: "Agents Focused"
        }
    }

    var supportsGroupedWorktrees: Bool {
        self != .projectFocused
    }

    static let storageKey = "muxy.appLayout"
    static let defaultValue: AppLayout = .projectFocused

    var provider: any AppLayoutProviding {
        switch self {
        case .projectFocused: ProjectFocusedLayout()
        case .tabFocused: TabFocusedLayout()
        case .agentsFocused: AgentsFocusedLayout()
        }
    }
}
