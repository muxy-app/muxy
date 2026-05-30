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

    static func cleanupStaleSessions() {
        guard let tmux = findBinary() else { return }
        let socket = socketName

        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + 5) {
            let process = Process()
            let pipe = Pipe()
            process.executableURL = URL(fileURLWithPath: tmux)
            process.arguments = ["-L", socket, "list-sessions", "-F", "#{session_name} #{session_attached}"]
            process.standardOutput = pipe
            process.standardError = FileHandle.nullDevice

            do {
                try process.run()
                process.waitUntilExit()
            } catch {
                logger.error("tmux list-sessions failed: \(error.localizedDescription)")
                return
            }

            guard process.terminationStatus == 0 else { return }
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            guard let output = String(data: data, encoding: .utf8) else { return }

            let staleSessions = output
                .split(separator: "\n")
                .map(String.init)
                .filter { $0.hasPrefix(sessionPrefix) }
                .filter { line in
                    let parts = line.split(separator: " ")
                    guard parts.count == 2 else { return false }
                    return parts[1] == "0"
                }
                .map { line in String(line.split(separator: " ")[0]) }

            guard !staleSessions.isEmpty else { return }

            logger.info("Cleaning up \(staleSessions.count) stale tmux session(s)")

            for session in staleSessions {
                let kill = Process()
                kill.executableURL = URL(fileURLWithPath: tmux)
                kill.arguments = ["-L", socket, "kill-session", "-t", session]
                kill.standardOutput = FileHandle.nullDevice
                kill.standardError = FileHandle.nullDevice
                try? kill.run()
                kill.waitUntilExit()
            }
        }
    }
}
