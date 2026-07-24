import CryptoKit
import Foundation

enum HookConfigWriter {
    static let backupSuffix = ".muxy-backup"

    static func write(_ settings: [String: Any], to path: String) throws {
        let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
        try write(data: data, to: path)
    }

    static func write(data: Data, to path: String) throws {
        let directory = (path as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(
            atPath: directory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )

        try writeBackupIfNeeded(path: path)

        let fileURL = URL(fileURLWithPath: path)
        try data.write(to: fileURL, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: path
        )
        HookConfigWriteLedger.shared.recordWrite(path: path, contents: data)
    }

    static func contentHash(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    private static func writeBackupIfNeeded(path: String) throws {
        let backupPath = path + backupSuffix
        guard FileManager.default.fileExists(atPath: path) else { return }
        if !FileManager.default.fileExists(atPath: backupPath) {
            try FileManager.default.copyItem(atPath: path, toPath: backupPath)
        }
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: backupPath
        )
    }
}

final class HookConfigWriteLedger: @unchecked Sendable {
    static let shared = HookConfigWriteLedger()

    static let repairWindow: TimeInterval = 60
    static let maximumRepairsPerWindow = 6

    private let lock = NSLock()
    private var lastWrittenHash: [String: String] = [:]
    private var repairTimestamps: [String: [Date]] = [:]
    private let now: () -> Date

    init(now: @escaping () -> Date = { Date() }) {
        self.now = now
    }

    func recordWrite(path: String, contents: Data) {
        let hash = HookConfigWriter.contentHash(contents)
        lock.lock()
        lastWrittenHash[path] = hash
        var stamps = repairTimestamps[path] ?? []
        stamps.append(now())
        repairTimestamps[path] = prune(stamps)
        lock.unlock()
    }

    func matchesLastWrite(path: String, contents: Data) -> Bool {
        let hash = HookConfigWriter.contentHash(contents)
        lock.lock()
        defer { lock.unlock() }
        return lastWrittenHash[path] == hash
    }

    func isSelfWrite(path: String) -> Bool {
        guard let contents = try? Data(contentsOf: URL(fileURLWithPath: path)) else { return false }
        return matchesLastWrite(path: path, contents: contents)
    }

    func hasExceededRepairBudget(path: String) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        let stamps = prune(repairTimestamps[path] ?? [])
        repairTimestamps[path] = stamps
        return stamps.count >= Self.maximumRepairsPerWindow
    }

    func reset(path: String) {
        lock.lock()
        lastWrittenHash.removeValue(forKey: path)
        repairTimestamps.removeValue(forKey: path)
        lock.unlock()
    }

    private func prune(_ stamps: [Date]) -> [Date] {
        let cutoff = now().addingTimeInterval(-Self.repairWindow)
        return stamps.filter { $0 > cutoff }
    }
}
