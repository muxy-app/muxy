import Foundation

enum AppLayout: String, CaseIterable, Identifiable {
    case projectFocused
    case tabFocused

    var id: String { rawValue }

    var title: String {
        switch self {
        case .projectFocused: "Project Focused"
        case .tabFocused: "Tab Focused"
        }
    }

    static let storageKey = "muxy.appLayout"
    static let defaultValue: AppLayout = .projectFocused

    var provider: any AppLayoutProviding {
        switch self {
        case .projectFocused: ProjectFocusedLayout()
        case .tabFocused: TabFocusedLayout()
        }
    }
}
