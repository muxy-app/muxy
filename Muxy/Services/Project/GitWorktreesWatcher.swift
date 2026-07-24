import CoreServices
import Foundation

final class GitWorktreesWatcher: @unchecked Sendable {
    private let queue = DispatchQueue(label: "app.muxy.worktrees-watcher", qos: .utility)
    private var stream: FSEventStreamRef?
    private var debounceWork: DispatchWorkItem?
    private var handler: (@Sendable () -> Void)?

    init?(repoPath: String, handler: @escaping @Sendable () -> Void) {
        guard let gitDirectory = Self.resolveGitDirectory(forRepoPath: repoPath) else { return nil }

        self.handler = handler

        var context = FSEventStreamContext()
        context.info = Unmanaged.passUnretained(self).toOpaque()

        let paths = [gitDirectory] as CFArray
        guard let stream = FSEventStreamCreate(
            nil,
            { _, clientInfo, numEvents, eventPaths, eventFlags, _ in
                guard let clientInfo, numEvents > 0 else { return }
                let watcher = Unmanaged<GitWorktreesWatcher>.fromOpaque(clientInfo).takeUnretainedValue()
                guard let paths = Unmanaged<CFArray>.fromOpaque(eventPaths).takeUnretainedValue() as? [String]
                else { return }
                let flags = Array(UnsafeBufferPointer(start: eventFlags, count: numEvents))

                let isRelevant = zip(paths, flags).contains { path, flag in
                    GitWorktreesWatcher.isRelevantChange(path: path, flag: flag)
                }
                guard isRelevant else { return }

                watcher.scheduleRefresh()
            },
            &context,
            paths,
            FSEventStreamEventId(kFSEventStreamEventIdSinceNow),
            0.3,
            FSEventStreamCreateFlags(kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagUseCFTypes)
        )
        else { return nil }

        self.stream = stream
        FSEventStreamSetDispatchQueue(stream, queue)
        FSEventStreamStart(stream)
    }

    deinit {
        handler = nil
        debounceWork?.cancel()
        guard let stream else { return }
        FSEventStreamStop(stream)
        FSEventStreamInvalidate(stream)
        FSEventStreamRelease(stream)
    }

    static func isRelevantChange(path: String, flag: FSEventStreamEventFlags) -> Bool {
        isWorktreeStructuralChange(path: path, flag: flag) || isHeadReferenceChange(path: path)
    }

    static func isHeadReferenceChange(path: String) -> Bool {
        guard path.hasSuffix("/HEAD") else { return false }
        return !path.contains("/logs/")
    }

    static func isWorktreeStructuralChange(path: String, flag: FSEventStreamEventFlags) -> Bool {
        guard path.contains("/worktrees/") else { return false }
        guard flag & FSEventStreamEventFlags(kFSEventStreamEventFlagItemIsDir) != 0 else { return false }
        let structuralMask = FSEventStreamEventFlags(
            kFSEventStreamEventFlagItemCreated
                | kFSEventStreamEventFlagItemRemoved
                | kFSEventStreamEventFlagItemRenamed
        )
        return flag & structuralMask != 0
    }

    static func resolveGitDirectory(forRepoPath repoPath: String) -> String? {
        guard let worktreeGitDirectory = resolveWorktreeGitDirectory(forRepoPath: repoPath) else { return nil }
        let dotGitDirectory = URL(fileURLWithPath: repoPath)
            .appendingPathComponent(".git")
            .standardizedFileURL
            .path
        guard worktreeGitDirectory != dotGitDirectory else { return worktreeGitDirectory }
        guard let range = worktreeGitDirectory.range(of: "/worktrees/") else { return worktreeGitDirectory }
        return String(worktreeGitDirectory[..<range.lowerBound])
    }

    static func resolveWorktreeGitDirectory(forRepoPath repoPath: String) -> String? {
        let manager = FileManager.default
        let repoURL = URL(fileURLWithPath: repoPath).standardizedFileURL
        let dotGitURL = repoURL.appendingPathComponent(".git")
        let dotGit = dotGitURL.path
        var isDirectory: ObjCBool = false
        guard manager.fileExists(atPath: dotGit, isDirectory: &isDirectory) else { return nil }
        if isDirectory.boolValue {
            return dotGit
        }

        guard let contents = try? String(contentsOfFile: dotGit, encoding: .utf8) else { return nil }
        guard let line = contents
            .split(whereSeparator: \.isNewline)
            .map({ $0.trimmingCharacters(in: .whitespaces) })
            .first(where: { $0.hasPrefix("gitdir:") })
        else { return nil }

        let target = line.dropFirst("gitdir:".count).trimmingCharacters(in: .whitespaces)
        let targetURL = URL(fileURLWithPath: target, relativeTo: repoURL).standardizedFileURL
        return targetURL.path
    }

    private func scheduleRefresh() {
        debounceWork?.cancel()
        let work = DispatchWorkItem { [weak self] in
            self?.handler?()
        }
        debounceWork = work
        queue.asyncAfter(deadline: .now() + 0.3, execute: work)
    }
}
