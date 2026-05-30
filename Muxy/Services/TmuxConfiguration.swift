import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "TmuxConfiguration")

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
