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

    static let storageKey = "muxy.sidebarCollapsedStyle"
    static let defaultValue: SidebarCollapsedStyle = .icons

    static var current: SidebarCollapsedStyle {
        guard let raw = UserDefaults.standard.string(forKey: storageKey),
              let mode = SidebarCollapsedStyle(rawValue: raw)
        else { return defaultValue }
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

    static let storageKey = "muxy.sidebarExpandedStyle"
    static let defaultValue: SidebarExpandedStyle = .wide

    static var current: SidebarExpandedStyle {
        guard let raw = UserDefaults.standard.string(forKey: storageKey),
              let mode = SidebarExpandedStyle(rawValue: raw)
        else { return defaultValue }
        return mode
    }
}
