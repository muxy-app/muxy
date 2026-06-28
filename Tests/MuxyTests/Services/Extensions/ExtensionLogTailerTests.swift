import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("ExtensionLogTailer")
struct ExtensionLogTailerTests {
    @Test("start resets to the existing log contents")
    func startResetsToExistingContents() throws {
        let url = try makeLogFile(contents: "first\nsecond\n")
        defer { try? FileManager.default.removeItem(at: url) }
        var updates: [ExtensionLogUpdate] = []
        let tailer = ExtensionLogTailer(url: url) { updates.append($0) }

        tailer.start()
        defer { tailer.stop() }

        #expect(updates.count == 1)
        #expect(updates.last.map(lines(of:)) == ["first", "second"])
        #expect(isReset(updates.last))
    }

    @Test("clear resets the view to empty")
    func clearResetsToEmpty() throws {
        let url = try makeLogFile(contents: "noise\n")
        defer { try? FileManager.default.removeItem(at: url) }
        var updates: [ExtensionLogUpdate] = []
        let tailer = ExtensionLogTailer(url: url) { updates.append($0) }

        tailer.start()
        defer { tailer.stop() }
        tailer.clear()

        #expect(isReset(updates.last))
        #expect(updates.last.map(lines(of:)) == [])
        let data = try Data(contentsOf: url)
        #expect(data.isEmpty)
    }

    private func makeLogFile(contents: String) throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("ext-tailer-\(UUID().uuidString).log")
        try contents.write(to: url, atomically: true, encoding: .utf8)
        return url
    }

    private func lines(of update: ExtensionLogUpdate) -> [String] {
        switch update {
        case let .reset(lines), let .append(lines): lines
        }
    }

    private func isReset(_ update: ExtensionLogUpdate?) -> Bool {
        if case .reset = update { return true }
        return false
    }
}
