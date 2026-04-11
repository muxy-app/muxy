import CoreServices
import Foundation

final class GitDirectoryWatcher: @unchecked Sendable {
    private let queue = DispatchQueue(label: "app.muxy.git-watcher", qos: .utility)
    private var stream: FSEventStreamRef?
    private var debounceWork: DispatchWorkItem?
    private var handler: (@Sendable () -> Void)?

    init?(directoryPath: String, handler: @escaping @Sendable () -> Void) {
        let gitPath = (directoryPath as NSString).appendingPathComponent(".git")
        guard FileManager.default.fileExists(atPath: gitPath) else { return nil }

        self.handler = handler

        var context = FSEventStreamContext()
        context.info = Unmanaged.passUnretained(self).toOpaque()

        let paths = [directoryPath] as CFArray
        guard let stream = FSEventStreamCreate(
            nil,
            { _, clientInfo, numEvents, eventPaths, eventFlags, _ in
                guard let clientInfo, numEvents > 0 else { return }
                let watcher = Unmanaged<GitDirectoryWatcher>.fromOpaque(clientInfo).takeUnretainedValue()
                guard let paths = Unmanaged<CFArray>.fromOpaque(eventPaths).takeUnretainedValue() as? [String]
                else { return }
                let flags = Array(UnsafeBufferPointer(start: eventFlags, count: numEvents))

                let dominated = zip(paths, flags).allSatisfy { path, flag in
                    let isGitInternal = path.contains("/.git/")
                    let isLockFile = path.hasSuffix(".lock")
                    return isGitInternal && isLockFile || flag & UInt32(kFSEventStreamEventFlagItemIsDir) != 0 && isGitInternal
                }
                guard !dominated else { return }

                watcher.scheduleRefresh()
            },
            &context,
            paths,
            FSEventStreamEventId(kFSEventStreamEventIdSinceNow),
            1.5,
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

    private func scheduleRefresh() {
        debounceWork?.cancel()
        let work = DispatchWorkItem { [weak self] in
            self?.handler?()
        }
        debounceWork = work
        queue.asyncAfter(deadline: .now() + 1.0, execute: work)
    }
}
