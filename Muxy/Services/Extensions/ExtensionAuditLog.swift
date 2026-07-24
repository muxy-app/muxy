import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionAuditLog")

struct ExtensionAuditEntry: Codable {
    let timestamp: Date
    let extensionID: String
    let verb: String
    let payloadSummary: String
    let decision: String
    let ruleID: String?
    let source: String
}

final class ExtensionAuditLog: @unchecked Sendable {
    static let shared = ExtensionAuditLog()

    static let maxSize: Int = 1 * 1024 * 1024
    static let trimToSize: Int = 256 * 1024

    private let queue = DispatchQueue(label: "app.muxy.extension-audit", qos: .utility)
    private let fileURL: URL
    private let encoder: JSONEncoder
    private var handle: FileHandle?

    init(fileURL: URL = ExtensionAuditLog.defaultFileURL) {
        self.fileURL = fileURL
        encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
    }

    deinit {
        try? handle?.close()
    }

    static var defaultFileURL: URL {
        MuxyFileStorage.appSupportDirectory().appendingPathComponent("extension-audit.log")
    }

    var auditFileURL: URL { fileURL }

    func record(_ entry: ExtensionAuditEntry) {
        queue.async { [weak self] in
            self?.append(entry: entry)
        }
    }

    private func append(entry: ExtensionAuditEntry) {
        do {
            let data = try encoder.encode(entry)
            var line = data
            line.append(0x0A)
            let handle = try resolveHandle()
            try handle.write(contentsOf: line)
            trimIfNeeded()
        } catch {
            logger.error("Failed to write audit log: \(error.localizedDescription)")
        }
    }

    private func resolveHandle() throws -> FileHandle {
        if let handle {
            return handle
        }
        if !FileManager.default.fileExists(atPath: fileURL.path) {
            FileManager.default.createFile(atPath: fileURL.path, contents: nil)
            try? FileManager.default.setAttributes(
                [.posixPermissions: FilePermissions.privateFile],
                ofItemAtPath: fileURL.path
            )
        }
        let newHandle = try FileHandle(forWritingTo: fileURL)
        try newHandle.seekToEnd()
        handle = newHandle
        return newHandle
    }

    private func closeHandle() {
        try? handle?.close()
        handle = nil
    }

    private func trimIfNeeded() {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: fileURL.path),
              let size = attributes[.size] as? Int,
              size > Self.maxSize
        else { return }

        let dropBytes = size - Self.trimToSize
        guard dropBytes > 0 else { return }

        let tempURL = fileURL.deletingLastPathComponent()
            .appendingPathComponent("extension-audit.log.tmp-\(UUID().uuidString)")
        do {
            closeHandle()
            let readHandle = try FileHandle(forReadingFrom: fileURL)
            defer { try? readHandle.close() }
            try readHandle.seek(toOffset: UInt64(dropBytes))
            try advancePastNextNewline(handle: readHandle)
            FileManager.default.createFile(atPath: tempURL.path, contents: nil)
            let writeHandle = try FileHandle(forWritingTo: tempURL)
            defer { try? writeHandle.close() }
            while let chunk = try readHandle.read(upToCount: 64 * 1024), !chunk.isEmpty {
                try writeHandle.write(contentsOf: chunk)
            }
            _ = try FileManager.default.replaceItemAt(fileURL, withItemAt: tempURL)
            try? FileManager.default.setAttributes(
                [.posixPermissions: FilePermissions.privateFile],
                ofItemAtPath: fileURL.path
            )
        } catch {
            try? FileManager.default.removeItem(at: tempURL)
            logger.error("Failed to trim audit log: \(error.localizedDescription)")
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
