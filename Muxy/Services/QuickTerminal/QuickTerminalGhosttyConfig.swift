import Foundation
import GhosttyKit

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

enum QuickTerminalGhosttyConfig {
    @MainActor
    static func apply(to surface: ghostty_surface_t) {
        guard let config = makeConfiguration() else { return }
        defer { ghostty_config_free(config) }

        ghostty_surface_update_config(surface, config)
    }

    static func overridesURL(bundle: Bundle = .module) -> URL? {
        guard let resourceURL = bundle.resourceURL else { return nil }
        let url = resourceURL.appendingPathComponent("ghostty/quick-terminal.conf")
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        return url
    }

    @MainActor
    private static func makeConfiguration() -> ghostty_config_t? {
        guard let base = GhosttyService.shared.config,
              let url = overridesURL()
        else { return nil }
        return GhosttyConfigOverlayLoader.live.load(base: base, overridesFilePath: url.path)
    }
}
