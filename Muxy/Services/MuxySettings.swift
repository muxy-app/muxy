import Foundation

/// Centralized app-level settings backed by UserDefaults via @AppStorage.
/// Keys are namespaced under "muxy." to avoid collisions.
enum MuxySettings {
    static let quickTerminalWidthFractionKey = "muxy.quickTerminal.widthFraction"
    static let quickTerminalHeightFractionKey = "muxy.quickTerminal.heightFraction"
    static let hideTabBarWhenSingleTabKey = "muxy.hideTabBarWhenSingleTab"
    static let windowBackgroundOpacityKey = "muxy.window.backgroundOpacity"
    static let windowBackgroundBlurKey = "muxy.window.backgroundBlur"

    static let defaultQuickTerminalWidthFraction: Double = 0.6
    static let defaultQuickTerminalHeightFraction: Double = 0.5
    static let defaultHideTabBarWhenSingleTab: Bool = false
    static let defaultWindowBackgroundOpacity: Double = 1.0
    static let defaultWindowBackgroundBlur: Bool = false
}
