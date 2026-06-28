import Foundation
import GhosttyKit
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy", category: "ClientThemeApplier")

enum ClientThemeApplier {
    @MainActor
    static func apply(_ theme: ClientThemeDTO, to surface: ghostty_surface_t) {
        guard let base = GhosttyService.shared.config,
              let config = ghostty_config_clone(base)
        else { return }
        defer { ghostty_config_free(config) }

        guard loadColors(theme, into: config) else { return }
        ghostty_config_finalize(config)
        ghostty_surface_update_config(surface, config)
    }

    @MainActor
    static func revert(_ surface: ghostty_surface_t) {
        guard let base = GhosttyService.shared.config else { return }
        ghostty_surface_update_config(surface, base)
    }

    private static func loadColors(_ theme: ClientThemeDTO, into config: ghostty_config_t) -> Bool {
        let contents = configText(for: theme)
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-client-theme-\(UUID().uuidString).conf")
        defer { try? FileManager.default.removeItem(at: url) }
        do {
            try Data(contents.utf8).write(to: url, options: .atomic)
        } catch {
            logger.error("Failed to write client theme config: \(error)")
            return false
        }
        url.path.withCString { ghostty_config_load_file(config, $0) }
        return true
    }

    static func configText(for theme: ClientThemeDTO) -> String {
        var lines: [String] = []
        for (index, color) in theme.palette.prefix(ClientThemeDTO.paletteLimit).enumerated() {
            lines.append("palette = \(index)=\(hex(color))")
        }
        lines.append("background = \(hex(theme.bg))")
        lines.append("foreground = \(hex(theme.fg))")
        appendColor(theme.cursorColor, key: "cursor-color", to: &lines)
        appendColor(theme.cursorText, key: "cursor-text", to: &lines)
        appendColor(theme.selectionBackground, key: "selection-background", to: &lines)
        appendColor(theme.selectionForeground, key: "selection-foreground", to: &lines)
        return lines.joined(separator: "\n") + "\n"
    }

    private static func appendColor(_ value: UInt32?, key: String, to lines: inout [String]) {
        guard let value else { return }
        lines.append("\(key) = \(hex(value))")
    }

    private static func hex(_ value: UInt32) -> String {
        let r = (value >> 16) & 0xFF
        let g = (value >> 8) & 0xFF
        let b = value & 0xFF
        return String(format: "#%02x%02x%02x", r, g, b)
    }
}
