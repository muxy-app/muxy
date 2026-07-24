import Foundation
import Testing

@testable import Muxy

@Suite("HookConfigWriter")
struct HookConfigWriterTests {
    @Test("backup captures the original content and is never overwritten by later repairs")
    func backupIsWrittenOnlyOnce() throws {
        let path = try makeConfig(contents: #"{"user":"original"}"#)
        defer { cleanUp(path) }

        try HookConfigWriter.write(["muxy": "first"], to: path)
        try HookConfigWriter.write(["muxy": "second"], to: path)

        let backup = try String(contentsOfFile: path + HookConfigWriter.backupSuffix, encoding: .utf8)
        #expect(backup == #"{"user":"original"}"#)
    }

    @Test("no backup is created when the config did not exist before")
    func noBackupForNewConfig() throws {
        let path = uniqueConfigPath()
        defer { cleanUp(path) }

        try HookConfigWriter.write(["muxy": "first"], to: path)

        #expect(!FileManager.default.fileExists(atPath: path + HookConfigWriter.backupSuffix))
    }

    @Test("a write we performed ourselves is recognised as a self write")
    func recognisesSelfWrite() throws {
        let path = try makeConfig(contents: "{}")
        defer { cleanUp(path) }
        let ledger = HookConfigWriteLedger.shared
        ledger.reset(path: path)

        try HookConfigWriter.write(["muxy": "value"], to: path)

        #expect(ledger.isSelfWrite(path: path))
    }

    @Test("a foreign write is not treated as a self write")
    func detectsForeignWrite() throws {
        let path = try makeConfig(contents: "{}")
        defer { cleanUp(path) }
        let ledger = HookConfigWriteLedger.shared
        ledger.reset(path: path)

        try HookConfigWriter.write(["muxy": "value"], to: path)
        try #"{"someone":"else"}"#.write(toFile: path, atomically: true, encoding: .utf8)

        #expect(!ledger.isSelfWrite(path: path))
    }

    @Test("the repair budget trips after repeated rewrites inside the window")
    func repairBudgetTrips() {
        let path = uniqueConfigPath()
        let clock = MutableClock()
        let ledger = HookConfigWriteLedger(now: clock.now)

        for _ in 0 ..< HookConfigWriteLedger.maximumRepairsPerWindow {
            ledger.recordWrite(path: path, contents: Data("x".utf8))
        }

        #expect(ledger.hasExceededRepairBudget(path: path))
    }

    @Test("the repair budget recovers once the window has passed")
    func repairBudgetRecovers() {
        let path = uniqueConfigPath()
        let clock = MutableClock()
        let ledger = HookConfigWriteLedger(now: clock.now)

        for _ in 0 ..< HookConfigWriteLedger.maximumRepairsPerWindow {
            ledger.recordWrite(path: path, contents: Data("x".utf8))
        }
        #expect(ledger.hasExceededRepairBudget(path: path))

        clock.advance(by: HookConfigWriteLedger.repairWindow + 1)

        #expect(!ledger.hasExceededRepairBudget(path: path))
    }

    private func uniqueConfigPath() -> String {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("hcw-\(UUID().uuidString.prefix(8)).json")
            .path
    }

    private func makeConfig(contents: String) throws -> String {
        let path = uniqueConfigPath()
        try contents.write(toFile: path, atomically: true, encoding: .utf8)
        return path
    }

    private func cleanUp(_ path: String) {
        try? FileManager.default.removeItem(atPath: path)
        try? FileManager.default.removeItem(atPath: path + HookConfigWriter.backupSuffix)
        HookConfigWriteLedger.shared.reset(path: path)
    }
}

private final class MutableClock: @unchecked Sendable {
    private let lock = NSLock()
    private var current = Date(timeIntervalSince1970: 1_000_000)

    func now() -> Date {
        lock.withLock { current }
    }

    func advance(by interval: TimeInterval) {
        lock.lock()
        current += interval
        lock.unlock()
    }
}
