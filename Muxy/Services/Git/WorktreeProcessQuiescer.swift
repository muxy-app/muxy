import Darwin
import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "WorktreeProcessQuiescer")

enum WorktreeProcessQuiescerError: LocalizedError {
    case processesStillRunning
    case directoryChanged(recoveryPath: String?)

    var errorDescription: String? {
        switch self {
        case .processesStillRunning:
            "Processes using this worktree could not be stopped."
        case let .directoryChanged(recoveryPath):
            if let recoveryPath {
                "The worktree directory changed during removal. Its files were preserved at \"\(recoveryPath)\"."
            } else {
                "The worktree directory changed during removal."
            }
        }
    }
}

enum WorktreeProcessQuiescer {
    static let quarantinePrefix = ".muxy-removal-"

    struct DirectoryIdentity: Equatable, Sendable {
        let device: UInt64
        let inode: UInt64
    }

    static func quiesce(path: String, timeout: TimeInterval) async throws {
        let deadline = OperationDeadline(timeout: timeout)
        var processIDs = matchingProcessIDs(using: path)
        guard !processIDs.isEmpty else { return }

        signal(SIGTERM, processIDs: processIDs, path: path)
        try await sleep(for: 0.3, deadline: deadline)

        processIDs = matchingProcessIDs(using: path)
        guard !processIDs.isEmpty else { return }

        signal(SIGKILL, processIDs: processIDs, path: path)
        for _ in 0 ..< 10 {
            try await sleep(for: 0.05, deadline: deadline)
            processIDs = matchingProcessIDs(using: path)
            guard !processIDs.isEmpty else { return }
        }

        throw WorktreeProcessQuiescerError.processesStillRunning
    }

    static func matchingProcessIDs(
        using path: String,
        candidates: [pid_t] = ProcSampling.listAllPIDs(),
        currentProcessID: pid_t = getpid(),
        workingDirectory: (pid_t) -> String? = ProcessArgumentsInspector.workingDirectory
    ) -> [pid_t] {
        let root = canonicalPath(path)
        let prefix = root.hasSuffix("/") ? root : root + "/"
        return candidates.filter { processID in
            guard processID != currentProcessID,
                  let directory = workingDirectory(processID)
            else { return false }
            let canonicalDirectory = canonicalPath(directory)
            return canonicalDirectory == root || canonicalDirectory.hasPrefix(prefix)
        }
    }

    private static func canonicalPath(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.resolvingSymlinksInPath().path
    }

    static func directoryIdentity(at path: String) -> DirectoryIdentity? {
        var info = stat()
        guard lstat(path, &info) == 0, info.st_mode & S_IFMT == S_IFDIR else { return nil }
        return DirectoryIdentity(device: UInt64(info.st_dev), inode: UInt64(info.st_ino))
    }

    static func removeDirectory(at path: String, matching expectedIdentity: DirectoryIdentity) throws {
        let source = URL(fileURLWithPath: path)
        let quarantine = source.deletingLastPathComponent().appendingPathComponent(
            "\(quarantinePrefix)\(UUID().uuidString)",
            isDirectory: true
        )
        try FileManager.default.moveItem(at: source, to: quarantine)
        guard directoryIdentity(at: quarantine.path) == expectedIdentity else {
            if !FileManager.default.fileExists(atPath: source.path) {
                do {
                    try FileManager.default.moveItem(at: quarantine, to: source)
                } catch {
                    throw WorktreeProcessQuiescerError.directoryChanged(recoveryPath: quarantine.path)
                }
                throw WorktreeProcessQuiescerError.directoryChanged(recoveryPath: nil)
            }
            throw WorktreeProcessQuiescerError.directoryChanged(recoveryPath: quarantine.path)
        }
        try FileManager.default.removeItem(at: quarantine)
    }

    private static func signal(_ value: Int32, processIDs: [pid_t], path: String) {
        let currentMatches = Set(matchingProcessIDs(using: path, candidates: processIDs))
        let identifiers = String(describing: currentMatches.sorted())
        logger.info(
            "Signal \(value), pids \(identifiers, privacy: .public), worktree \(path, privacy: .private(mask: .hash))"
        )
        for processID in processIDs where currentMatches.contains(processID) {
            kill(processID, value)
        }
    }

    private static func sleep(for duration: TimeInterval, deadline: OperationDeadline) async throws {
        let remaining = try deadline.remaining()
        try await Task.sleep(for: .seconds(min(duration, remaining)))
    }
}
