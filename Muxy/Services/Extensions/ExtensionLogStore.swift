import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionLogStore")

final class ExtensionLogStore: @unchecked Sendable {
    static let shared = ExtensionLogStore()

    static let maxSize: Int = 5 * 1024 * 1024
    static let trimToSize: Int = 1_250_000
    static let trimInterval: TimeInterval = 600

    private struct Entry {
        let extensionDirectory: URL
        var handle: FileHandle?
        var lastTrimChecked: Date
    }

    private let queue = DispatchQueue(label: "app.muxy.extension-logs", qos: .utility)
    private var entries: [String: Entry] = [:]
    private var trimTimer: DispatchSourceTimer?

    private init() {
        startTrimTimer()
    }

    func logURL(extensionID: String, directory: URL) -> URL {
        directory.appendingPathComponent("logs/output.log")
    }

    func register(extensionID: String, directory: URL) {
        queue.async {
            if self.entries[extensionID] == nil {
                self.entries[extensionID] = Entry(
                    extensionDirectory: directory,
                    handle: nil,
                    lastTrimChecked: .distantPast
                )
            }
        }
    }

    func unregister(extensionID: String) {
        queue.async {
            if var entry = self.entries.removeValue(forKey: extensionID) {
                try? entry.handle?.close()
                entry.handle = nil
            }
        }
    }

    func append(extensionID: String, line: String) {
        queue.async {
            self.appendLocked(extensionID: extensionID, line: line)
        }
    }

    func clear(extensionID: String) {
        queue.async {
            guard let directory = self.entries[extensionID]?.extensionDirectory else { return }
            let url = self.logURL(extensionID: extensionID, directory: directory)
            self.closeHandle(for: extensionID)
            try? Data().write(to: url, options: [.atomic])
        }
    }

    func flush() {
        queue.sync {}
    }

    private func startTrimTimer() {
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + Self.trimInterval, repeating: Self.trimInterval)
        timer.setEventHandler { [weak self] in
            self?.runTrimPass()
        }
        timer.resume()
        trimTimer = timer
    }

    private func appendLocked(extensionID: String, line: String) {
        guard let entry = entries[extensionID] else { return }
        let url = logURL(extensionID: extensionID, directory: entry.extensionDirectory)
        do {
            try ensureDirectoryExists(for: url)
            let handle = try resolveHandle(for: extensionID, url: url)
            let payload = line.hasSuffix("\n") ? line : line + "\n"
            try handle.write(contentsOf: Data(payload.utf8))
        } catch {
            logger.error("Failed to append log for \(extensionID): \(error.localizedDescription)")
        }
    }

    private func resolveHandle(for extensionID: String, url: URL) throws -> FileHandle {
        if let existing = entries[extensionID]?.handle {
            return existing
        }
        if !FileManager.default.fileExists(atPath: url.path) {
            FileManager.default.createFile(atPath: url.path, contents: nil)
        }
        let handle = try FileHandle(forWritingTo: url)
        try handle.seekToEnd()
        entries[extensionID]?.handle = handle
        return handle
    }

    private func closeHandle(for extensionID: String) {
        if var entry = entries[extensionID] {
            try? entry.handle?.close()
            entry.handle = nil
            entries[extensionID] = entry
        }
    }

    private func ensureDirectoryExists(for url: URL) throws {
        let directory = url.deletingLastPathComponent()
        if !FileManager.default.fileExists(atPath: directory.path) {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
    }

    private func runTrimPass() {
        for (id, entry) in entries {
            let url = logURL(extensionID: id, directory: entry.extensionDirectory)
            guard let attributes = try? FileManager.default.attributesOfItem(atPath: url.path),
                  let size = attributes[.size] as? Int,
                  let modified = attributes[.modificationDate] as? Date
            else { continue }
            if modified <= entry.lastTrimChecked, size <= Self.maxSize {
                continue
            }
            entries[id]?.lastTrimChecked = Date()
            guard size > Self.maxSize else { continue }
            trimFile(extensionID: id, url: url, size: size)
        }
    }

    private func trimFile(extensionID: String, url: URL, size: Int) {
        closeHandle(for: extensionID)
        let dropPrefixBytes = size - Self.trimToSize
        guard dropPrefixBytes > 0 else { return }

        let tempURL = url.deletingLastPathComponent()
            .appendingPathComponent("output.log.tmp-\(UUID().uuidString)")
        do {
            let readHandle = try FileHandle(forReadingFrom: url)
            defer { try? readHandle.close() }
            try readHandle.seek(toOffset: UInt64(dropPrefixBytes))
            try advancePastNextNewline(handle: readHandle)
            FileManager.default.createFile(atPath: tempURL.path, contents: nil)
            let writeHandle = try FileHandle(forWritingTo: tempURL)
            defer { try? writeHandle.close() }
            while let chunk = try readHandle.read(upToCount: 64 * 1024), !chunk.isEmpty {
                try writeHandle.write(contentsOf: chunk)
            }
            _ = try FileManager.default.replaceItemAt(url, withItemAt: tempURL)
        } catch {
            try? FileManager.default.removeItem(at: tempURL)
            logger.error("Failed to trim log for \(extensionID): \(error.localizedDescription)")
        }
    }

    private func advancePastNextNewline(handle: FileHandle) throws {
        while let chunk = try handle.read(upToCount: 4096), !chunk.isEmpty {
            if let index = chunk.firstIndex(of: UInt8(ascii: "\n")) {
                let bytesIntoChunk = chunk.distance(from: chunk.startIndex, to: index) + 1
                let consumed = chunk.count
                let rewind = consumed - bytesIntoChunk
                let position = try handle.offset()
                try handle.seek(toOffset: position - UInt64(rewind))
                return
            }
        }
    }
}
