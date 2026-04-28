import AppKit
import SwiftUI

enum MuxyTheme {
    @MainActor static var bg: Color { snapshot.bg }
    @MainActor static var nsBg: NSColor { snapshot.nsBg }
    @MainActor static var fg: Color { snapshot.fg }
    @MainActor static var fgMuted: Color { snapshot.fgMuted }
    @MainActor static var fgDim: Color { snapshot.fgDim }
    @MainActor static var surface: Color { snapshot.surface }
    @MainActor static var border: Color { snapshot.border }
    @MainActor static var hover: Color { snapshot.hover }

    @MainActor static var accent: Color { snapshot.accent }
    @MainActor static var accentSoft: Color { snapshot.accentSoft }
    @MainActor static var warning: Color { snapshot.warning }

    @MainActor static var diffAddFg: Color { snapshot.diffAddFg }
    @MainActor static var diffRemoveFg: Color { snapshot.diffRemoveFg }
    @MainActor static var diffHunkFg: Color { snapshot.diffHunkFg }
    @MainActor static var diffAddBg: Color { snapshot.diffAddBg }
    @MainActor static var diffRemoveBg: Color { snapshot.diffRemoveBg }
    @MainActor static var diffHunkBg: Color { snapshot.diffHunkBg }

    @MainActor static var nsDiffAdd: NSColor { snapshot.nsDiffAdd }
    @MainActor static var nsDiffRemove: NSColor { snapshot.nsDiffRemove }
    @MainActor static var nsDiffHunk: NSColor { snapshot.nsDiffHunk }
    @MainActor static var nsDiffString: NSColor { snapshot.nsDiffString }
    @MainActor static var nsDiffNumber: NSColor { snapshot.nsDiffNumber }
    @MainActor static var nsDiffComment: NSColor { snapshot.nsDiffComment }

    @MainActor static var colorScheme: ColorScheme { snapshot.colorScheme }

    @MainActor private static var cachedVersion: Int = -1
    @MainActor private static var cachedSnapshot: Snapshot?

    @MainActor private static var snapshot: Snapshot {
        let version = GhosttyService.shared.configVersion
        if let cached = cachedSnapshot, cachedVersion == version {
            return cached
        }
        let newSnapshot = Snapshot(from: GhosttyService.shared)
        cachedSnapshot = newSnapshot
        cachedVersion = version
        return newSnapshot
    }
}

extension MuxyTheme {
    struct Snapshot {
        let nsBg: NSColor
        let bg: Color
        let fg: Color
        let fgMuted: Color
        let fgDim: Color
        let surface: Color
        let border: Color
        let hover: Color
        let accent: Color
        let accentSoft: Color
        let warning: Color
        let diffAddFg: Color
        let diffRemoveFg: Color
        let diffHunkFg: Color
        let diffAddBg: Color
        let diffRemoveBg: Color
        let diffHunkBg: Color
        let nsDiffAdd: NSColor
        let nsDiffRemove: NSColor
        let nsDiffHunk: NSColor
        let nsDiffString: NSColor
        let nsDiffNumber: NSColor
        let nsDiffComment: NSColor
        let colorScheme: ColorScheme

        @MainActor
        init(from service: GhosttyService) {
            let bgColor = service.backgroundColor
            let fgColor = service.foregroundColor
            let accentColor = service.accentColor

            nsBg = bgColor
            bg = Color(nsColor: bgColor)
            fg = Color(nsColor: fgColor)
            fgMuted = Color(nsColor: fgColor.withAlphaComponent(0.65))
            fgDim = Color(nsColor: fgColor.withAlphaComponent(0.4))
            surface = Color(nsColor: fgColor.withAlphaComponent(0.08))
            border = Color(nsColor: fgColor.withAlphaComponent(0.12))
            hover = Color(nsColor: fgColor.withAlphaComponent(0.06))
            accent = Color(nsColor: accentColor)
            accentSoft = Color(nsColor: accentColor.withAlphaComponent(0.1))
            warning = Color(nsColor: service.paletteColor(at: 3) ?? NSColor.systemYellow)

            let addColor = service.paletteColor(at: 2) ?? NSColor.systemGreen
            let removeColor = service.paletteColor(at: 1) ?? NSColor.systemRed
            let hunkColor = service.paletteColor(at: 6) ?? accentColor

            nsDiffAdd = addColor
            nsDiffRemove = removeColor
            nsDiffHunk = hunkColor
            nsDiffString = service.paletteColor(at: 2) ?? NSColor.systemGreen
            nsDiffNumber = service.paletteColor(at: 3) ?? NSColor.systemYellow
            nsDiffComment = service.paletteColor(at: 8) ?? fgColor.withAlphaComponent(0.5)

            diffAddFg = Color(nsColor: addColor)
            diffRemoveFg = Color(nsColor: removeColor)
            diffHunkFg = Color(nsColor: hunkColor)
            diffAddBg = Color(nsColor: addColor.withAlphaComponent(0.16))
            diffRemoveBg = Color(nsColor: removeColor.withAlphaComponent(0.16))
            diffHunkBg = Color(nsColor: hunkColor.withAlphaComponent(0.1))

            let srgb = bgColor.usingColorSpace(.sRGB)
            let luminance: CGFloat = if let srgb {
                0.2126 * srgb.redComponent + 0.7152 * srgb.greenComponent + 0.0722 * srgb.blueComponent
            } else {
                0
            }
            colorScheme = luminance > 0.5 ? .light : .dark
        }
    }
}
