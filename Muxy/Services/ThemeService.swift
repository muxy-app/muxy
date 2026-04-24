import AppKit
import Foundation
import MuxyShared

struct ThemePreview: Identifiable {
    let name: String
    let background: NSColor
    let foreground: NSColor
    let palette: [NSColor]
    var id: String { name }
}

@MainActor @Observable
final class ThemeService {
    static let shared = ThemeService()
    nonisolated static let defaultThemeName = "Muxy"
    nonisolated static let pinnedThemeNames: Set<String> = ["Muxy", "Muxy Light"]

    @ObservationIgnored private let config: MuxyConfig
    @ObservationIgnored private let ghostty: GhosttyService
    @ObservationIgnored private var cachedColors: CachedThemeColors?

    private struct CachedThemeColors {
        let name: String
        let fg: UInt32
        let bg: UInt32
        let palette: [UInt32]
    }

    init(config: MuxyConfig = .shared, ghostty: GhosttyService = .shared) {
        self.config = config
        self.ghostty = ghostty
    }

    func loadThemes() async -> [ThemePreview] {
        await Task.detached { Self.discoverThemes() }.value
    }

    func currentThemeName() -> String? {
        config.configValue(for: "theme")?.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
    }

    func currentThemeColors() -> DeviceThemeEventDTO? {
        guard let name = currentThemeName() else { return nil }
        if let cached = cachedColors, cached.name == name {
            return DeviceThemeEventDTO(fg: cached.fg, bg: cached.bg, palette: cached.palette)
        }
        for dir in Self.themeDirectories() {
            let path = dir + "/" + name
            guard FileManager.default.fileExists(atPath: path),
                  let theme = Self.parseThemeFile(atPath: path, name: name)
            else { continue }
            let fg = Self.rgb(from: theme.foreground)
            let bg = Self.rgb(from: theme.background)
            let palette = currentPalette()
            cachedColors = CachedThemeColors(name: name, fg: fg, bg: bg, palette: palette)
            return DeviceThemeEventDTO(fg: fg, bg: bg, palette: palette)
        }
        return nil
    }

    private func currentPalette() -> [UInt32] {
        (0 ..< 16).map { index in
            guard let color = ghostty.paletteColor(at: index) else { return 0 }
            return Self.rgb(from: color)
        }
    }

    nonisolated private static func rgb(from color: NSColor) -> UInt32 {
        let srgb = color.usingColorSpace(.sRGB) ?? color
        let r = UInt32((srgb.redComponent * 255).rounded()) & 0xFF
        let g = UInt32((srgb.greenComponent * 255).rounded()) & 0xFF
        let b = UInt32((srgb.blueComponent * 255).rounded()) & 0xFF
        return (r << 16) | (g << 8) | b
    }

    func applyDefaultThemeIfNeeded() {
        guard currentThemeName() == nil else { return }
        applyTheme(Self.defaultThemeName)
    }

    func applyTheme(_ name: String) {
        let sanitized = name.filter { $0 != "\"" && $0 != "\n" && $0 != "\r" }
        config.updateConfigValue("theme", value: "\"\(sanitized)\"")
        cachedColors = nil
        ghostty.reloadConfig()
        NotificationCenter.default.post(name: .themeDidChange, object: nil)
    }

    nonisolated private static func discoverThemes() -> [ThemePreview] {
        var themesByName: [String: ThemePreview] = [:]

        for dir in themeDirectories() {
            guard let files = try? FileManager.default.contentsOfDirectory(atPath: dir) else { continue }
            for file in files {
                guard let theme = parseThemeFile(atPath: dir + "/" + file, name: file) else { continue }
                themesByName[theme.name] = theme
            }
        }

        return themesByName.values.sorted {
            let pinned0 = pinnedThemeNames.contains($0.name)
            let pinned1 = pinnedThemeNames.contains($1.name)
            if pinned0 != pinned1 { return pinned0 }
            if pinned0, pinned1 { return $0.name < $1.name }
            return $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending
        }
    }

    nonisolated private static func themeDirectories() -> [String] {
        var dirs: [String] = []
        if let resourcesDir = getenv("GHOSTTY_RESOURCES_DIR").map({ String(cString: $0) }) {
            dirs.append(resourcesDir + "/themes")
        }

        let appBundlePaths = [
            "/Applications/Ghostty.app/Contents/Resources/ghostty/themes",
            NSHomeDirectory() + "/Applications/Ghostty.app/Contents/Resources/ghostty/themes",
        ]
        for path in appBundlePaths where !dirs.contains(path) {
            dirs.append(path)
        }

        dirs.append(NSHomeDirectory() + "/.config/ghostty/themes")

        if let bundledThemes = Bundle.appResources.resourceURL?.appendingPathComponent("themes").path {
            dirs.append(bundledThemes)
        }

        return dirs
    }

    nonisolated private static func parseThemeFile(atPath path: String, name: String) -> ThemePreview? {
        guard let content = try? String(contentsOfFile: path, encoding: .utf8) else { return nil }
        var bg: NSColor?
        var fg: NSColor?
        var palette: [Int: NSColor] = [:]
        for line in content.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("background"), !trimmed.hasPrefix("background-") {
                bg = extractColor(from: trimmed)
            } else if trimmed.hasPrefix("foreground"), !trimmed.hasPrefix("foreground-") {
                fg = extractColor(from: trimmed)
            } else if trimmed.hasPrefix("palette") {
                parsePaletteEntry(trimmed, into: &palette)
            }
        }
        guard let bg, let fg else { return nil }
        let sortedPalette = (0 ..< 16).compactMap { palette[$0] }
        return ThemePreview(name: name, background: bg, foreground: fg, palette: sortedPalette)
    }

    nonisolated private static func parsePaletteEntry(_ line: String, into palette: inout [Int: NSColor]) {
        guard let eqIndex = line.firstIndex(of: "=") else { return }
        let value = line[line.index(after: eqIndex)...].trimmingCharacters(in: .whitespaces)
        guard let eqIndex2 = value.firstIndex(of: "=") else { return }
        guard let index = Int(value[..<eqIndex2]) else { return }
        guard index >= 0, index < 16 else { return }
        guard let color = parseHex(String(value[value.index(after: eqIndex2)...])) else { return }
        palette[index] = color
    }

    nonisolated private static func extractColor(from line: String) -> NSColor? {
        guard let eqIndex = line.firstIndex(of: "=") else { return nil }
        let value = line[line.index(after: eqIndex)...].trimmingCharacters(in: .whitespaces)
        return parseHex(value)
    }

    nonisolated private static func parseHex(_ hex: String) -> NSColor? {
        var h = hex
        if h.hasPrefix("#") { h = String(h.dropFirst()) }
        guard h.count == 6, let val = UInt32(h, radix: 16) else { return nil }
        return NSColor(
            srgbRed: CGFloat((val >> 16) & 0xFF) / 255,
            green: CGFloat((val >> 8) & 0xFF) / 255,
            blue: CGFloat(val & 0xFF) / 255,
            alpha: 1
        )
    }
}
