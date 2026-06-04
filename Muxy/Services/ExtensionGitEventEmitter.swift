import Foundation

final class ExtensionGitEventEmitter: @unchecked Sendable {
    static let shared = ExtensionGitEventEmitter()

    private let window: TimeInterval = 0.3
    private let lock = NSLock()
    private var lastEmitted: [String: TimeInterval] = [:]

    static func emit(projectPath: String) {
        shared.emit(projectPath: projectPath)
    }

    func emit(projectPath: String) {
        let now = ProcessInfo.processInfo.systemUptime
        guard shouldEmit(projectPath: projectPath, now: now) else { return }

        Task {
            let state = await Self.gitState(repoPath: projectPath)
            NotificationSocketServer.shared.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.gitChanged,
                payload: [
                    "projectPath": projectPath,
                    "branch": state.branch,
                    "hasChanges": state.hasChanges ? "true" : "false",
                ]
            ))
        }
    }

    func shouldEmit(projectPath: String, now: TimeInterval) -> Bool {
        lock.lock()
        defer { lock.unlock() }

        lastEmitted = lastEmitted.filter { now - $0.value < window }
        if let last = lastEmitted[projectPath], now - last < window { return false }
        lastEmitted[projectPath] = now
        return true
    }

    private static func gitState(repoPath: String) async -> (branch: String, hasChanges: Bool) {
        let result = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["status", "--porcelain=1", "--branch"]
        )
        guard let result, result.status == 0 else { return ("", false) }
        return parseStatus(result.stdout)
    }

    static func parseStatus(_ output: String) -> (branch: String, hasChanges: Bool) {
        let lines = output.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        var branch = ""
        var hasChanges = false
        for line in lines where !line.isEmpty {
            if line.hasPrefix("## ") {
                branch = parseBranch(line)
                continue
            }
            hasChanges = true
        }
        return (branch, hasChanges)
    }

    private static func parseBranch(_ line: String) -> String {
        let header = line.dropFirst(3)
        let beforeTracking = header.split(separator: " ", maxSplits: 1).first.map(String.init) ?? ""
        return beforeTracking.split(separator: ".", maxSplits: 1).first.map(String.init) ?? beforeTracking
    }
}
