import Foundation

enum SidebarCollapsedStyle: String, CaseIterable, Identifiable {
    case hidden
    case icons

    var id: String { rawValue }

    var title: String {
        switch self {
        case .hidden: "Hidden"
        case .icons: "Icons"
        }
    }

    private static let key = "muxy.sidebarCollapsedStyle"

    static var current: SidebarCollapsedStyle {
        guard let raw = UserDefaults.standard.string(forKey: key),
              let mode = SidebarCollapsedStyle(rawValue: raw)
        else { return .icons }
        return mode
    }
}

enum SidebarExpandedStyle: String, CaseIterable, Identifiable {
    case icons
    case wide

    var id: String { rawValue }

    var title: String {
        switch self {
        case .icons: "Icons"
        case .wide: "Wide"
        }
    }

    private static let key = "muxy.sidebarExpandedStyle"

    static var current: SidebarExpandedStyle {
        guard let raw = UserDefaults.standard.string(forKey: key),
              let mode = SidebarExpandedStyle(rawValue: raw)
        else { return .wide }
        return mode
    }
}
