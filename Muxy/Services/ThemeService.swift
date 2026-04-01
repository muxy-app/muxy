import Foundation
import AppKit

@MainActor @Observable
final class ThemeService {
    static let shared = ThemeService()

    @ObservationIgnored private let config: MuxyConfig
    @ObservationIgnored private let ghostty: GhosttyService

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

    func applyTheme(_ name: String) {
        let sanitized = name.filter { $0 != "\"" && $0 != "\n" && $0 != "\r" }
        config.updateConfigValue("theme", value: "\"\(sanitized)\"")
        ghostty.reloadConfig()
    }

    private nonisolated static func discoverThemes() -> [ThemePreview] {
        var themesByName: [String: ThemePreview] = [:]

        for dir in themeDirectories() {
            guard let files = try? FileManager.default.contentsOfDirectory(atPath: dir) else { continue }
            for file in files {
                guard let theme = parseThemeFile(atPath: dir + "/" + file, name: file) else { continue }
                themesByName[theme.name] = theme
            }
        }

        return themesByName.values.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    private nonisolated static func themeDirectories() -> [String] {
        var dirs: [String] = []
        if let resourcesDir = getenv("GHOSTTY_RESOURCES_DIR").map({ String(cString: $0) }) {
            dirs.append(resourcesDir + "/themes")
        }
        dirs.append(NSHomeDirectory() + "/.config/ghostty/themes")
        return dirs
    }

    private nonisolated static func parseThemeFile(atPath path: String, name: String) -> ThemePreview? {
        guard let content = try? String(contentsOfFile: path, encoding: .utf8) else { return nil }
        var bg: NSColor?
        var fg: NSColor?
        var palette: [Int: NSColor] = [:]
        for line in content.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("background") && !trimmed.hasPrefix("background-") {
                bg = extractColor(from: trimmed)
            } else if trimmed.hasPrefix("foreground") && !trimmed.hasPrefix("foreground-") {
                fg = extractColor(from: trimmed)
            } else if trimmed.hasPrefix("palette") {
                parsePaletteEntry(trimmed, into: &palette)
            }
        }
        guard let bg, let fg else { return nil }
        let sortedPalette = (0..<16).compactMap { palette[$0] }
        return ThemePreview(name: name, background: bg, foreground: fg, palette: sortedPalette)
    }

    private nonisolated static func parsePaletteEntry(_ line: String, into palette: inout [Int: NSColor]) {
        guard let eqIndex = line.firstIndex(of: "=") else { return }
        let value = line[line.index(after: eqIndex)...].trimmingCharacters(in: .whitespaces)
        guard let eqIndex2 = value.firstIndex(of: "=") else { return }
        guard let index = Int(value[..<eqIndex2]) else { return }
        guard index >= 0 && index < 16 else { return }
        guard let color = parseHex(String(value[value.index(after: eqIndex2)...])) else { return }
        palette[index] = color
    }

    private nonisolated static func extractColor(from line: String) -> NSColor? {
        guard let eqIndex = line.firstIndex(of: "=") else { return nil }
        let value = line[line.index(after: eqIndex)...].trimmingCharacters(in: .whitespaces)
        return parseHex(value)
    }

    private nonisolated static func parseHex(_ hex: String) -> NSColor? {
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
