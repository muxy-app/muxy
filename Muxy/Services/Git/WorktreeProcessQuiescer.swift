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
    typealias BoundedRemover = @Sendable (_ path: String, _ timeout: TimeInterval) async throws -> Void

    struct DirectoryIdentity: Equatable, Sendable {
        let device: UInt64
        let inode: UInt64
    }

    private struct ProcessMatcher {
        let path: String
        let identity: DirectoryIdentity
        let workingDirectory: @Sendable (pid_t) -> String?

        func matching(_ candidates: [pid_t]) throws -> [pid_t] {
            try validate()
            let matches = WorktreeProcessQuiescer.matchingProcessIDs(
                using: path,
                candidates: candidates,
                workingDirectory: workingDirectory
            )
            try validate()
            return matches
        }

        func validate() throws {
            guard WorktreeProcessQuiescer.directoryIdentity(at: path) == identity else {
                throw WorktreeProcessQuiescerError.directoryChanged(recoveryPath: nil)
            }
        }
    }

    static func quiesce(
        path: String,
        matching expectedIdentity: DirectoryIdentity,
        timeout: TimeInterval,
        processIDs: @escaping @Sendable () -> [pid_t] = { ProcSampling.listAllPIDs() },
        workingDirectory: @escaping @Sendable (pid_t) -> String? = ProcessArgumentsInspector.workingDirectory,
        sendSignal: @escaping @Sendable (pid_t, Int32) -> Int32 = { kill($0, $1) }
    ) async throws {
        let deadline = OperationDeadline(timeout: timeout)
        let matcher = ProcessMatcher(path: path, identity: expectedIdentity, workingDirectory: workingDirectory)
        var matchedProcessIDs = try matcher.matching(processIDs())
        guard !matchedProcessIDs.isEmpty else { return }

        try signal(
            SIGTERM,
            processIDs: matchedProcessIDs,
            matcher: matcher,
            deadline: deadline,
            sendSignal: sendSignal
        )
        try await sleep(for: 0.3, deadline: deadline)

        matchedProcessIDs = try matcher.matching(processIDs())
        guard !matchedProcessIDs.isEmpty else { return }

        try signal(
            SIGKILL,
            processIDs: matchedProcessIDs,
            matcher: matcher,
            deadline: deadline,
            sendSignal: sendSignal
        )
        for _ in 0 ..< 10 {
            try await sleep(for: 0.05, deadline: deadline)
            matchedProcessIDs = try matcher.matching(processIDs())
            guard !matchedProcessIDs.isEmpty else { return }
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

    static func removeDirectory(
        at path: String,
        matching expectedIdentity: DirectoryIdentity,
        timeout: TimeInterval,
        boundedRemover: @escaping BoundedRemover = { path, timeout in
            try await LocalFileOps().removeItem(at: path, timeout: timeout)
        }
    ) async throws {
        let source = URL(fileURLWithPath: path)
        let quarantine = source.deletingLastPathComponent().appendingPathComponent(
            "\(quarantinePrefix)\(UUID().uuidString)",
            isDirectory: true
        )
        guard directoryIdentity(at: source.path) == expectedIdentity else {
            throw WorktreeProcessQuiescerError.directoryChanged(recoveryPath: nil)
        }
        try await GitProcessRunner.offMainThrowing {
            guard directoryIdentity(at: source.path) == expectedIdentity else {
                throw WorktreeProcessQuiescerError.directoryChanged(recoveryPath: nil)
            }
            try FileManager.default.moveItem(at: source, to: quarantine)
        }
        guard directoryIdentity(at: quarantine.path) == expectedIdentity else {
            try await restore(quarantine, to: source)
            throw WorktreeProcessQuiescerError.directoryChanged(recoveryPath: nil)
        }
        do {
            try await boundedRemover(quarantine.path, timeout)
        } catch {
            guard await LocalFileOps().exists(at: quarantine.path) else { return }
            try await restore(quarantine, to: source)
            throw error
        }
    }

    private static func signal(
        _ value: Int32,
        processIDs: [pid_t],
        matcher: ProcessMatcher,
        deadline: OperationDeadline,
        sendSignal: (pid_t, Int32) -> Int32
    ) throws {
        let currentMatches = try Set(matcher.matching(processIDs))
        let identifiers = String(describing: currentMatches.sorted())
        logger.info(
            "Signal \(value), pids \(identifiers, privacy: .public), worktree \(matcher.path, privacy: .private(mask: .hash))"
        )
        for processID in processIDs where currentMatches.contains(processID) {
            try matcher.validate()
            _ = try deadline.remaining()
            _ = sendSignal(processID, value)
        }
    }

    private static func restore(_ quarantine: URL, to source: URL) async throws {
        do {
            try await GitProcessRunner.offMainThrowing {
                guard !FileManager.default.fileExists(atPath: source.path),
                      (try? FileManager.default.destinationOfSymbolicLink(atPath: source.path)) == nil
                else {
                    throw WorktreeProcessQuiescerError.directoryChanged(recoveryPath: quarantine.path)
                }
                try FileManager.default.moveItem(at: quarantine, to: source)
            }
        } catch {
            throw WorktreeProcessQuiescerError.directoryChanged(recoveryPath: quarantine.path)
        }
    }

    private static func sleep(for duration: TimeInterval, deadline: OperationDeadline) async throws {
        let remaining = try deadline.remaining()
        try await Task.sleep(for: .seconds(min(duration, remaining)))
    }
}
