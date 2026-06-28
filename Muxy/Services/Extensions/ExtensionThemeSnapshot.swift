import AppKit
import Foundation

@MainActor
enum ExtensionThemeSnapshot {
    static func current() -> [String: String] {
        var values: [String: String] = [:]
        let accentNS = NSColor(MuxyTheme.accent)
        values["background"] = hex(MuxyTheme.nsBg)
        values["foreground"] = hex(MuxyTheme.nsFg)
        values["foregroundMuted"] = hex(MuxyTheme.nsFgMuted)
        values["surface"] = hex(MuxyTheme.nsFg, alpha: 0.08)
        values["border"] = hex(MuxyTheme.nsFg, alpha: 0.12)
        values["hover"] = hex(MuxyTheme.nsFg, alpha: 0.06)
        values["accent"] = hex(accentNS)
        values["accentSoft"] = hex(accentNS, alpha: 0.1)
        values["diffAdd"] = hex(MuxyTheme.nsDiffAdd)
        values["diffRemove"] = hex(MuxyTheme.nsDiffRemove)
        values["diffHunk"] = hex(MuxyTheme.nsDiffHunk)
        values["colorScheme"] = MuxyTheme.colorScheme == .dark ? "dark" : "light"
        values["topbarHeight"] = "\(Int(UIMetrics.titleBarHeight.rounded()))px"
        return values
    }

    private static func hex(_ color: NSColor, alpha: CGFloat? = nil) -> String {
        let resolved = color.usingColorSpace(.sRGB) ?? color
        let r = Int(round(resolved.redComponent * 255))
        let g = Int(round(resolved.greenComponent * 255))
        let b = Int(round(resolved.blueComponent * 255))
        let a = alpha ?? resolved.alphaComponent
        if a >= 0.999 {
            return String(format: "#%02x%02x%02x", r, g, b)
        }
        let aByte = Int(round(a * 255))
        return String(format: "#%02x%02x%02x%02x", r, g, b, aByte)
    }
}
