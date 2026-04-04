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

    @MainActor static var diffAddFg: Color {
        Color(nsColor: nsDiffAdd)
    }

    @MainActor static var diffRemoveFg: Color {
        Color(nsColor: nsDiffRemove)
    }

    @MainActor static var diffHunkFg: Color {
        Color(nsColor: nsDiffHunk)
    }

    @MainActor static var diffAddBg: Color {
        Color(nsColor: nsDiffAdd.withAlphaComponent(0.16))
    }

    @MainActor static var diffRemoveBg: Color {
        Color(nsColor: nsDiffRemove.withAlphaComponent(0.16))
    }

    @MainActor static var diffHunkBg: Color {
        Color(nsColor: nsDiffHunk.withAlphaComponent(0.1))
    }

    @MainActor static var nsDiffAdd: NSColor {
        GhosttyService.shared.paletteColor(at: 2) ?? NSColor.systemGreen
    }

    @MainActor static var nsDiffRemove: NSColor {
        GhosttyService.shared.paletteColor(at: 1) ?? NSColor.systemRed
    }

    @MainActor static var nsDiffHunk: NSColor {
        GhosttyService.shared.paletteColor(at: 6) ?? GhosttyService.shared.accentColor
    }

    @MainActor static var nsDiffString: NSColor {
        GhosttyService.shared.paletteColor(at: 2) ?? NSColor.systemGreen
    }

    @MainActor static var nsDiffNumber: NSColor {
        GhosttyService.shared.paletteColor(at: 3) ?? NSColor.systemYellow
    }

    @MainActor static var nsDiffComment: NSColor {
        GhosttyService.shared.paletteColor(at: 8) ?? GhosttyService.shared.foregroundColor.withAlphaComponent(0.5)
    }
}
