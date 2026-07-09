import Foundation

enum AppBackgroundStyle: String, CaseIterable, Identifiable {
    case solid
    case vibrant

    static let storageKey = "muxy.appBackgroundStyle"
    static let defaultValue = AppBackgroundStyle.vibrant

    var id: String { rawValue }

    var title: String {
        switch self {
        case .solid: "Solid"
        case .vibrant: "Vibrant"
        }
    }

    static func resolve(_ rawValue: String?) -> AppBackgroundStyle {
        guard let rawValue, let style = AppBackgroundStyle(rawValue: rawValue) else { return defaultValue }
        return style
    }
}

enum AppSidebarVibrancyPolicy {
    static func isActive(
        style: AppBackgroundStyle,
        reduceTransparency: Bool,
        increaseContrast: Bool,
        isFullScreen: Bool
    ) -> Bool {
        style == .vibrant && !reduceTransparency && !increaseContrast && !isFullScreen
    }
}
