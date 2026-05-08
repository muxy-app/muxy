import CoreServices
import Foundation

final class EditorFileWatcher: @unchecked Sendable {
    private let queue = DispatchQueue(label: "app.muxy.editor-file-watcher", qos: .utility)
    private let filePath: String
    private let debounceInterval: TimeInterval
    private var stream: FSEventStreamRef?
    private var debounceWork: DispatchWorkItem?
    private var handler: (@Sendable () -> Void)?

    init?(filePath: String, debounceInterval: TimeInterval = 0.3, handler: @escaping @Sendable () -> Void) {
        self.filePath = filePath
        self.debounceInterval = debounceInterval
        self.handler = handler

        let directory = (filePath as NSString).deletingLastPathComponent
        guard !directory.isEmpty else { return nil }

        var context = FSEventStreamContext()
        context.info = Unmanaged.passUnretained(self).toOpaque()

        let paths = [directory] as CFArray
        guard let stream = FSEventStreamCreate(
            nil,
            { _, clientInfo, numEvents, eventPaths, _, _ in
                guard let clientInfo, numEvents > 0 else { return }
                let watcher = Unmanaged<EditorFileWatcher>.fromOpaque(clientInfo).takeUnretainedValue()
                guard let paths = Unmanaged<CFArray>.fromOpaque(eventPaths).takeUnretainedValue() as? [String]
                else { return }
                let target = (watcher.filePath as NSString).resolvingSymlinksInPath
                let matched = paths.contains { ($0 as NSString).resolvingSymlinksInPath == target }
                guard matched else { return }
                watcher.scheduleRefresh()
            },
            &context,
            paths,
            FSEventStreamEventId(kFSEventStreamEventIdSinceNow),
            0,
            FSEventStreamCreateFlags(kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagUseCFTypes)
        )
        else { return nil }

        self.stream = stream
        FSEventStreamSetDispatchQueue(stream, queue)
        FSEventStreamStart(stream)
    }

    deinit {
        if let stream {
            FSEventStreamStop(stream)
            FSEventStreamInvalidate(stream)
            FSEventStreamRelease(stream)
        }
        debounceWork?.cancel()
        handler = nil
    }

    private func scheduleRefresh() {
        debounceWork?.cancel()
        let work = DispatchWorkItem { [weak self] in
            self?.handler?()
        }
        debounceWork = work
        queue.asyncAfter(deadline: .now() + debounceInterval, execute: work)
    }
}
