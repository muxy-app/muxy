import CoreGraphics
import SwiftUI

@MainActor
enum UIMetrics {
    static var fontMicro: CGFloat { scaled(8) }
    static var fontXS: CGFloat { scaled(9) }
    static var fontCaption: CGFloat { scaled(10) }
    static var fontFootnote: CGFloat { scaled(11) }
    static var fontBody: CGFloat { scaled(12) }
    static var fontEmphasis: CGFloat { scaled(13) }
    static var fontHeadline: CGFloat { scaled(14) }
    static var fontTitle: CGFloat { scaled(15) }
    static var fontTitleLarge: CGFloat { scaled(16) }
    static var fontDisplay: CGFloat { scaled(20) }
    static var fontHero: CGFloat { scaled(24) }
    static var fontMega: CGFloat { scaled(28) }

    static var spacing1: CGFloat { scaled(2) }
    static var spacing2: CGFloat { scaled(4) }
    static var spacing3: CGFloat { scaled(6) }
    static var spacing4: CGFloat { scaled(8) }
    static var spacing5: CGFloat { scaled(10) }
    static var spacing6: CGFloat { scaled(12) }
    static var spacing7: CGFloat { scaled(16) }
    static var spacing8: CGFloat { scaled(20) }
    static var spacing9: CGFloat { scaled(24) }
    static var spacing10: CGFloat { scaled(32) }

    static var iconXS: CGFloat { scaled(10) }
    static var iconSM: CGFloat { scaled(12) }
    static var iconMD: CGFloat { scaled(14) }
    static var iconLG: CGFloat { scaled(16) }
    static var iconXL: CGFloat { scaled(20) }
    static var iconXXL: CGFloat { scaled(28) }

    static var controlSmall: CGFloat { scaled(20) }
    static var controlMedium: CGFloat { scaled(24) }
    static var controlLarge: CGFloat { scaled(32) }

    static var radiusSM: CGFloat { scaled(4) }
    static var radiusMD: CGFloat { scaled(6) }
    static var radiusLG: CGFloat { scaled(8) }
    static var radiusXL: CGFloat { scaled(10) }

    static var sidebarCollapsedWidth: CGFloat { scaled(44) }
    static var sidebarExpandedWidth: CGFloat { scaled(220) }

    static var tabBarHeight: CGFloat { scaled(28) }
    static var headerHeight: CGFloat { scaled(36) }

    static func scaled(_ value: CGFloat) -> CGFloat {
        value * UIScale.shared.multiplier
    }
}
