import Foundation
import GhosttyKit

enum GhosttyDefaultsConfig {
    static func defaultsURL(bundle: Bundle) -> URL? {
        guard let resourceURL = bundle.resourceURL else { return nil }
        let url = resourceURL.appendingPathComponent("ghostty-overrides/muxy-defaults.conf")
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        return url
    }

    static func load(into config: ghostty_config_t) {
        guard let url = defaultsURL(bundle: .appResources) else { return }
        url.path.withCString { ghostty_config_load_file(config, $0) }
    }
}
