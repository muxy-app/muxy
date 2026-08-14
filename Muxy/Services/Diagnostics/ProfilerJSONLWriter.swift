import Foundation

protocol ProfilerRecordWriting: Sendable {
    var fileURL: URL { get }
    func startSession(_ session: ProfilerSessionRecord) throws
    func append(_ sample: ProfilerSampleRecord, session: ProfilerSessionRecord) throws
}

final class ProfilerJSONLWriter: ProfilerRecordWriting, @unchecked Sendable {
    static let maximumFileSize = 25 * 1024 * 1024

    let fileURL: URL

    private let directoryURL: URL
    private let maximumFileSize: Int
    private let fileManager: FileManager

    init(
        fileURL: URL,
        maximumFileSize: Int = ProfilerJSONLWriter.maximumFileSize,
        fileManager: FileManager = .default
    ) {
        self.fileURL = fileURL
        directoryURL = fileURL.deletingLastPathComponent()
        self.maximumFileSize = maximumFileSize
        self.fileManager = fileManager
    }

    func startSession(_ session: ProfilerSessionRecord) throws {
        try prepareStorage()
        try appendLine(ProfilerRecordEncoder.encode(session), rolloverSession: session)
    }

    func append(_ sample: ProfilerSampleRecord, session: ProfilerSessionRecord) throws {
        try prepareStorage()
        try appendLine(ProfilerRecordEncoder.encode(sample), rolloverSession: session)
    }

    private func prepareStorage() throws {
        try fileManager.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        try fileManager.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directoryURL.path)

        guard fileManager.fileExists(atPath: fileURL.path) else { return }
        try fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: fileURL.path)
    }

    private func appendLine(_ record: Data, rolloverSession: ProfilerSessionRecord) throws {
        let existingSize = try repairTrailingRecordIfNeeded()
        let sessionLine = try ProfilerRecordEncoder.encode(rolloverSession)
        if existingSize == 0, record != sessionLine {
            try replaceFile(with: record, session: rolloverSession)
            return
        }
        let appendedSize = existingSize + record.count + 1

        if appendedSize > maximumFileSize {
            try replaceFile(with: record, session: rolloverSession)
            return
        }

        if !fileManager.fileExists(atPath: fileURL.path) {
            guard fileManager.createFile(
                atPath: fileURL.path,
                contents: nil,
                attributes: [.posixPermissions: 0o600]
            )
            else {
                throw CocoaError(.fileWriteUnknown)
            }
        }

        var line = record
        line.append(0x0A)
        let handle = try FileHandle(forWritingTo: fileURL)
        defer { try? handle.close() }
        try handle.seekToEnd()
        try handle.write(contentsOf: line)
        try fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: fileURL.path)
    }

    private func replaceFile(with record: Data, session: ProfilerSessionRecord) throws {
        let sessionLine = try ProfilerRecordEncoder.encode(session)
        var replacement = Data()
        replacement.append(sessionLine)
        replacement.append(0x0A)
        if record != sessionLine {
            replacement.append(record)
            replacement.append(0x0A)
        }
        try replacement.write(to: fileURL, options: .atomic)
        try fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: fileURL.path)
    }

    private func repairTrailingRecordIfNeeded() throws -> Int {
        guard fileManager.fileExists(atPath: fileURL.path) else { return 0 }
        let handle = try FileHandle(forUpdating: fileURL)
        defer { try? handle.close() }
        let end = try handle.seekToEnd()
        guard end > 0 else { return 0 }

        try handle.seek(toOffset: end - 1)
        guard try handle.read(upToCount: 1) != Data([0x0A]) else { return Int(end) }

        let recordStart = try trailingRecordStart(in: handle, endOffset: end)
        try handle.seek(toOffset: recordStart)
        let trailingData = try handle.readToEnd() ?? Data()
        if (try? JSONSerialization.jsonObject(with: trailingData)) != nil {
            try handle.seekToEnd()
            try handle.write(contentsOf: Data([0x0A]))
            return Int(end) + 1
        }

        try handle.truncate(atOffset: recordStart)
        return Int(recordStart)
    }

    private func trailingRecordStart(in handle: FileHandle, endOffset: UInt64) throws -> UInt64 {
        let chunkSize: UInt64 = 4096
        var searchEnd = endOffset
        while searchEnd > 0 {
            let searchStart = searchEnd > chunkSize ? searchEnd - chunkSize : 0
            try handle.seek(toOffset: searchStart)
            let data = try handle.read(upToCount: Int(searchEnd - searchStart)) ?? Data()
            if let newlineIndex = data.lastIndex(of: 0x0A) {
                return searchStart + UInt64(newlineIndex) + 1
            }
            searchEnd = searchStart
        }
        return 0
    }
}
