import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionProcessTree")
struct ExtensionProcessTreeTests {
    @Test("terminating the extension host kills its child processes")
    func terminatingHostKillsChildren() async throws {
        let marker = UUID().uuidString
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = ["-c", "sh -c 'sleep 90; echo \(marker)' & wait"]
        try process.run()

        try await Task.sleep(for: .milliseconds(400))
        #expect(childCount(marker: marker) >= 1)

        ExtensionStore.terminateProcessTreeForTesting(pid: process.processIdentifier)

        try await Task.sleep(for: .milliseconds(600))
        #expect(childCount(marker: marker) == 0)
    }

    private func childCount(marker: String) -> Int {
        let probe = Process()
        probe.executableURL = URL(fileURLWithPath: "/bin/sh")
        probe.arguments = ["-c", "pgrep -f 'sleep 90; echo \(marker)' | wc -l"]
        let pipe = Pipe()
        probe.standardOutput = pipe
        try? probe.run()
        probe.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let text = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? "0"
        return Int(text) ?? 0
    }
}
