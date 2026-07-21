import Foundation
import GhosttyKit
import os

private let notchTerminalGhosttyLogger = Logger(subsystem: "app.muxy", category: "NotchTerminalGhosttyConfig")

@MainActor
struct GhosttyConfigOverlayLoader {
    let clone: (ghostty_config_t) -> ghostty_config_t?
    let loadFile: (ghostty_config_t, String) -> Void

    func load(base: ghostty_config_t, overridesFilePath: String) -> ghostty_config_t? {
        guard let config = clone(base) else { return nil }
        loadFile(config, overridesFilePath)
        return config
    }

    static let live = GhosttyConfigOverlayLoader(
        clone: { ghostty_config_clone($0) },
        loadFile: { config, file in
            file.withCString { ghostty_config_load_file(config, $0) }
        }
    )
}

enum NotchTerminalGhosttyConfig {
    @MainActor
    static func apply(_: NotchTerminalAppearance, to surface: ghostty_surface_t) {
        guard let config = makeConfiguration() else { return }
        defer { ghostty_config_free(config) }

        ghostty_surface_update_config(surface, config)
    }

    static func configText() -> String {
        "background-opacity = 0.00\nbackground-blur = false\n"
    }

    @MainActor
    private static func makeConfiguration() -> ghostty_config_t? {
        guard let base = GhosttyService.shared.config else { return nil }
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-notch-terminal-\(UUID().uuidString).conf")
        defer { try? FileManager.default.removeItem(at: url) }
        do {
            try Data(configText().utf8).write(to: url, options: .atomic)
        } catch {
            notchTerminalGhosttyLogger.error("Failed to write the notch terminal config: \(error.localizedDescription)")
            return nil
        }
        return GhosttyConfigOverlayLoader.live.load(base: base, overridesFilePath: url.path)
    }
}
