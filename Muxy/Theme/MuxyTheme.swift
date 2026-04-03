import AppKit
import SwiftUI

enum MuxyTheme {
    @MainActor static var bg: Color { Color(nsColor: GhosttyService.shared.backgroundColor) }
    @MainActor static var nsBg: NSColor { GhosttyService.shared.backgroundColor }
    @MainActor static var fg: Color { Color(nsColor: GhosttyService.shared.foregroundColor) }
    @MainActor static var fgMuted: Color {
        Color(nsColor: GhosttyService.shared.foregroundColor.withAlphaComponent(0.65))
    }

    @MainActor static var fgDim: Color {
        Color(nsColor: GhosttyService.shared.foregroundColor.withAlphaComponent(0.4))
    }

    @MainActor static var surface: Color {
        Color(nsColor: GhosttyService.shared.foregroundColor.withAlphaComponent(0.08))
    }

    @MainActor static var border: Color {
        Color(nsColor: GhosttyService.shared.foregroundColor.withAlphaComponent(0.12))
    }

    @MainActor static var hover: Color {
        Color(nsColor: GhosttyService.shared.foregroundColor.withAlphaComponent(0.06))
    }

    @MainActor static var accent: Color { Color(nsColor: GhosttyService.shared.accentColor) }
    @MainActor static var accentSoft: Color {
        Color(nsColor: GhosttyService.shared.accentColor.withAlphaComponent(0.1))
    }

    @MainActor static var terminalBg: Color {
        Color(nsColor: GhosttyService.shared.backgroundColor.withAlphaComponent(GhosttyService.shared.backgroundOpacity))
    }
}
