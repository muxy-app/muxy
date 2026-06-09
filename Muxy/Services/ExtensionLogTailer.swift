import Darwin
import Foundation

enum ExtensionLogUpdate {
    case reset([String])
    case append([String])
}

@MainActor
final class ExtensionLogTailer {
    static let maxBufferedLines = 1000

    private let url: URL
    private var source: DispatchSourceFileSystemObject?
    private var readOffset: UInt64 = 0
    private var partial: String = ""
    private let onUpdate: @MainActor (ExtensionLogUpdate) -> Void

    init(url: URL, onUpdate: @escaping @MainActor (ExtensionLogUpdate) -> Void) {
        self.url = url
        self.onUpdate = onUpdate
    }

    deinit {
        source?.cancel()
    }

    func start() {
        ensureFileExists()
        loadInitialLines()
        startWatching()
    }

    func stop() {
        source?.cancel()
        source = nil
    }

    func clear() {
        try? Data().write(to: url, options: [.atomic])
        readOffset = 0
        partial = ""
        onUpdate(.reset([]))
    }

    private func ensureFileExists() {
        let directory = url.deletingLastPathComponent()
        if !FileManager.default.fileExists(atPath: directory.path) {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        if !FileManager.default.fileExists(atPath: url.path) {
            FileManager.default.createFile(atPath: url.path, contents: nil)
        }
    }

    private func loadInitialLines() {
        let lines = ExtensionLogTail.read(url: url, maxLines: Self.maxBufferedLines)
        onUpdate(.reset(lines))
        if let attributes = try? FileManager.default.attributesOfItem(atPath: url.path),
           let size = attributes[.size] as? UInt64
        {
            readOffset = size
        }
    }

    private func startWatching() {
        let fd = open(url.path, O_EVTONLY)
        guard fd >= 0 else { return }

        let dispatchSource = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd,
            eventMask: [.write, .extend, .delete, .rename],
            queue: .main
        )
        dispatchSource.setEventHandler { [weak self] in
            self?.handleFileEvent(dispatchSource.data)
        }
        dispatchSource.setCancelHandler {
            close(fd)
        }
        dispatchSource.resume()
        source = dispatchSource
    }

    private func handleFileEvent(_ events: DispatchSource.FileSystemEvent) {
        if events.contains(.delete) || events.contains(.rename) {
            stop()
            readOffset = 0
            partial = ""
            ensureFileExists()
            loadInitialLines()
            startWatching()
            return
        }
        readAppendedBytes()
    }

    private func readAppendedBytes() {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: url.path),
              let size = attributes[.size] as? UInt64
        else { return }

        if size < readOffset {
            readOffset = 0
            partial = ""
            let lines = ExtensionLogTail.read(url: url, maxLines: Self.maxBufferedLines)
            onUpdate(.reset(lines))
            readOffset = size
            return
        }
        guard size > readOffset else { return }

        guard let handle = try? FileHandle(forReadingFrom: url) else { return }
        defer { try? handle.close() }
        do {
            try handle.seek(toOffset: readOffset)
        } catch {
            return
        }
        guard let data = try? handle.readToEnd(), !data.isEmpty,
              let text = String(data: data, encoding: .utf8)
        else {
            readOffset = size
            return
        }
        readOffset = size
        let combined = partial + text
        let pieces = combined.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        if combined.hasSuffix("\n") {
            partial = ""
        } else {
            partial = pieces.last ?? ""
        }
        let lines = pieces.dropLast()
        if !lines.isEmpty {
            onUpdate(.append(Array(lines)))
        }
    }
}
