import AppKit
import Foundation
import MuxyShared

enum ThemeAppearance: Hashable {
    case light
    case dark
}

struct ThemePreview: Identifiable {
    let name: String
    let background: NSColor
    let foreground: NSColor
    let palette: [NSColor]
    var id: String { name }
}

struct ThemeSelection: Equatable {
    let rawValue: String
    let darkName: String?
    let lightName: String?
    let fallbackName: String?

    var displayName: String {
        if let darkName, let lightName {
            return "Dark: \(darkName), Light: \(lightName)"
        }
        return fallbackName ?? rawValue
    }

    func resolvedName(isDark: Bool) -> String? {
        if isDark, let darkName { return darkName }
        if !isDark, let lightName { return lightName }
        return fallbackName ?? darkName ?? lightName
    }
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
        currentThemeSelection()?.displayName
    }

    func currentThemeSelection() -> ThemeSelection? {
        guard let value = config.configValue(for: "theme") else { return nil }
        return Self.parseThemeSelection(value)
    }

    func currentLightThemeName() -> String? {
        guard let selection = currentThemeSelection() else { return nil }
        return selection.lightName ?? selection.fallbackName
    }

    func currentDarkThemeName() -> String? {
        guard let selection = currentThemeSelection() else { return nil }
        return selection.darkName ?? selection.fallbackName
    }

    func activeThemeName() -> String? {
        currentThemeSelection()?.resolvedName(isDark: Self.isCurrentAppearanceDark())
    }

    func activeThemePreview() -> ThemePreview? {
        guard let name = activeThemeName() else { return nil }
        return Self.themePreview(named: name)
    }

    func activeAppearance() -> ThemeAppearance {
        Self.isCurrentAppearanceDark() ? .dark : .light
    }

    func activeThemePreview(for appearance: ThemeAppearance) -> ThemePreview? {
        guard let name = currentThemeSelection()?.resolvedName(isDark: appearance == .dark) else { return nil }
        return Self.themePreview(named: name)
    }

    func currentThemeColors() -> DeviceThemeEventDTO? {
        guard let name = activeThemeName() else { return nil }
        if let cached = cachedColors, cached.name == name {
            return DeviceThemeEventDTO(fg: cached.fg, bg: cached.bg, palette: cached.palette)
        }
        guard let theme = Self.themePreview(named: name) else { return nil }
        let fg = Self.rgb(from: theme.foreground)
        let bg = Self.rgb(from: theme.background)
        let palette = theme.palette.count == 16
            ? theme.palette.map(Self.rgb(from:))
            : currentPalette()
        cachedColors = CachedThemeColors(name: name, fg: fg, bg: bg, palette: palette)
        return DeviceThemeEventDTO(fg: fg, bg: bg, palette: palette)
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

    func migrateToPairedThemeIfNeeded() {
        guard let selection = currentThemeSelection() else { return }
        if selection.darkName != nil, selection.lightName != nil { return }
        let unified = selection.darkName ?? selection.lightName ?? selection.fallbackName ?? Self.defaultThemeName
        applyTheme(dark: selection.darkName ?? unified, light: selection.lightName ?? unified)
    }

    func applyLightTheme(_ name: String) {
        let dark = currentDarkThemeName() ?? name
        applyTheme(dark: dark, light: name)
    }

    func applyDarkTheme(_ name: String) {
        let light = currentLightThemeName() ?? name
        applyTheme(dark: name, light: light)
    }

    func applyTheme(_ name: String) {
        let sanitized = sanitizedThemeName(name)
        config.updateConfigValue("theme", value: "\"\(sanitized)\"")
        cachedColors = nil
        ghostty.reloadConfig()
        NotificationCenter.default.post(name: .themeDidChange, object: nil)
    }

    func applyTheme(dark darkName: String, light lightName: String) {
        let dark = sanitizedThemeName(darkName)
        let light = sanitizedThemeName(lightName)
        config.updateConfigValue("theme", value: "dark:\"\(dark)\",light:\"\(light)\"")
        cachedColors = nil
        ghostty.reloadConfig()
        NotificationCenter.default.post(name: .themeDidChange, object: nil)
    }

    private func sanitizedThemeName(_ name: String) -> String {
        name.filter { $0 != "\"" && $0 != "\n" && $0 != "\r" }
    }

    nonisolated static func parseThemeSelection(_ value: String) -> ThemeSelection {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        let unquoted = unquote(trimmed)
        let entries = splitThemeEntries(unquoted)
        var darkName: String?
        var lightName: String?
        var fallbackParts: [String] = []

        for entry in entries {
            let parts = entry.split(separator: ":", maxSplits: 1).map(String.init)
            guard parts.count == 2 else {
                fallbackParts.append(entry)
                continue
            }
            let key = parts[0].trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            let name = unquote(parts[1].trimmingCharacters(in: .whitespacesAndNewlines))
            if key == "dark" {
                darkName = name
            } else if key == "light" {
                lightName = name
            } else {
                fallbackParts.append(entry)
            }
        }

        let fallback = fallbackParts.isEmpty
            ? (darkName == nil && lightName == nil ? unquoted : nil)
            : fallbackParts.joined(separator: ",")
        return ThemeSelection(
            rawValue: trimmed,
            darkName: darkName,
            lightName: lightName,
            fallbackName: fallback
        )
    }

    nonisolated private static func splitThemeEntries(_ value: String) -> [String] {
        var entries: [String] = []
        var current = ""
        var isQuoted = false
        for char in value {
            if char == "\"" {
                isQuoted.toggle()
                current.append(char)
            } else if char == ",", !isQuoted {
                let entry = current.trimmingCharacters(in: .whitespacesAndNewlines)
                if !entry.isEmpty { entries.append(entry) }
                current = ""
            } else {
                current.append(char)
            }
        }
        let entry = current.trimmingCharacters(in: .whitespacesAndNewlines)
        if !entry.isEmpty { entries.append(entry) }
        return entries
    }

    nonisolated private static func unquote(_ value: String) -> String {
        guard value.count >= 2, value.first == "\"", value.last == "\"" else { return value }
        return String(value.dropFirst().dropLast())
    }

    static func isCurrentAppearanceDark() -> Bool {
        isDarkAppearance(
            userInterfaceStyle: UserDefaults.standard.string(forKey: "AppleInterfaceStyle"),
            effectiveAppearance: NSApp?.effectiveAppearance
        )
    }

    nonisolated static func isDarkAppearance(
        userInterfaceStyle: String?,
        effectiveAppearance: NSAppearance?
    ) -> Bool {
        if let userInterfaceStyle {
            return userInterfaceStyle.localizedCaseInsensitiveCompare("dark") == .orderedSame
        }
        guard let effectiveAppearance else { return false }
        return effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
    }

    nonisolated private static func themePreview(named name: String) -> ThemePreview? {
        for dir in themeDirectories() {
            let path = dir + "/" + name
            guard FileManager.default.fileExists(atPath: path),
                  let theme = parseThemeFile(atPath: path, name: name)
            else { continue }
            return theme
        }
        return nil
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
        if let bundled = Bundle.appResources.resourceURL?.appendingPathComponent("ghostty/themes").path {
            dirs.append(bundled)
        }
        dirs.append(NSHomeDirectory() + "/.config/ghostty/themes")
        if let extraThemes = Bundle.appResources.resourceURL?.appendingPathComponent("themes").path {
            dirs.append(extraThemes)
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
