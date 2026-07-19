import CoreServices
import Foundation

final class HookConfigWatcher: @unchecked Sendable {
    private let queue = DispatchQueue(label: "app.muxy.hook-config-watcher", qos: .utility)
    private var stream: FSEventStreamRef?
    private let watchedPaths: Set<String>
    private let trigger: DebouncedTrigger

    init?(
        configPaths: [String],
        debounceInterval: TimeInterval = 0.4,
        handler: @escaping @Sendable () -> Void
    ) {
        let directories = Set(configPaths.map { ($0 as NSString).deletingLastPathComponent }).filter { !$0.isEmpty }
        guard !directories.isEmpty else { return nil }

        watchedPaths = Set(configPaths)
        trigger = DebouncedTrigger(
            interval: debounceInterval,
            scheduler: QueueDebounceScheduler(queue: queue),
            handler: handler
        )

        var context = FSEventStreamContext()
        context.info = Unmanaged.passUnretained(self).toOpaque()

        let paths = Array(directories) as CFArray
        guard let stream = FSEventStreamCreate(
            nil,
            { _, clientInfo, numEvents, eventPaths, _, _ in
                guard let clientInfo, numEvents > 0 else { return }
                let watcher = Unmanaged<HookConfigWatcher>.fromOpaque(clientInfo).takeUnretainedValue()
                guard let paths = Unmanaged<CFArray>.fromOpaque(eventPaths).takeUnretainedValue() as? [String]
                else { return }
                watcher.handleEvents(paths: paths)
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
        guard let stream else { return }
        FSEventStreamStop(stream)
        FSEventStreamInvalidate(stream)
        FSEventStreamRelease(stream)
    }

    static func isRelevantChange(path: String, watchedPaths: Set<String>) -> Bool {
        guard !path.hasSuffix(".muxy-backup") else { return false }
        return watchedPaths.contains { watched in
            path == watched || path.hasPrefix(watched)
        }
    }

    private func handleEvents(paths: [String]) {
        let relevant = paths.contains { Self.isRelevantChange(path: $0, watchedPaths: watchedPaths) }
        guard relevant else { return }
        trigger.signal()
    }
}
