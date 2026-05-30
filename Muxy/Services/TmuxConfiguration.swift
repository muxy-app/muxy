import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "TmuxConfiguration")

/// Manages tmux binary discovery and session naming for Low Memory Mode.
///
/// Low Memory Mode reduces RAM by offloading hidden terminal surfaces to tmux sessions.
/// Shell state persists across workspace switches without keeping Ghostty surfaces in memory.
///
/// Requires tmux 3.3+ installed via Homebrew (`brew install tmux`).
/// When tmux is absent, the feature degrades gracefully — the settings toggle is disabled
/// and all terminals use the standard Ghostty rendering path.
enum TmuxConfiguration {
    static let socketName = "muxy"
    static let sessionPrefix = "muxy-"

    private static let binarySearchPaths = [
        "/opt/homebrew/bin/tmux",
        "/usr/local/bin/tmux",
        "/opt/local/bin/tmux",
        "/usr/bin/tmux",
    ]

    nonisolated(unsafe) private static var cachedBinary: String?

    static func findBinary() -> String? {
        if let cached = cachedBinary, FileManager.default.isExecutableFile(atPath: cached) {
            return cached
        }
        cachedBinary = nil
        let found = binarySearchPaths.first { FileManager.default.isExecutableFile(atPath: $0) }
        if let found { cachedBinary = found }
        return found
    }

    static func sessionName(for paneID: UUID) -> String {
        "\(sessionPrefix)\(paneID.uuidString.prefix(8))"
    }

    static func lowMemoryModeEnabled() -> Bool {
        findBinary() != nil &&
            UserDefaults.standard.bool(forKey: GeneralSettingsKeys.lowMemoryMode)
    }
}
