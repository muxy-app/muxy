import Foundation
import Testing

@testable import Muxy

@Suite("Test isolation script", .serialized)
struct TestIsolationScriptTests {
    @Test("test processes never resolve production app storage")
    func testProcessUsesIsolatedAppStorage() {
        let path = MuxyFileStorage.appSupportDirectory().path

        #expect(MuxyFileStorage.isTestProcess)
        #expect(Bundle.main.bundleIdentifier != "com.muxy.app")
        #expect(path.contains("muxy-tests.") || path.contains("MuxyTests-"))
    }

    @Test("runs commands with a disposable Core Foundation home")
    func runsWithDisposableHome() throws {
        let scriptURL = RepositoryRoot.find().appendingPathComponent("scripts/run-tests-isolated.sh")
        let process = Process()
        let output = Pipe()
        process.executableURL = scriptURL
        process.arguments = [
            "/usr/bin/swift",
            "-e",
            "import Foundation; let url = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!.appendingPathComponent(\"Muxy\"); try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true); print(url.path)",
        ]
        process.standardOutput = output
        process.standardError = output

        try process.run()
        process.waitUntilExit()

        let data = output.fileHandleForReading.readDataToEndOfFile()
        let path = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)

        #expect(process.terminationStatus == 0)
        #expect(path.contains("muxy-tests."))
        #expect(path.hasSuffix("Library/Application Support/Muxy"))
        #expect(!FileManager.default.fileExists(atPath: path))
    }

    @Test("all local test entry points use isolation")
    func localTestEntryPointsUseIsolation() throws {
        let root = RepositoryRoot.find()
        let checks = try String(contentsOf: root.appendingPathComponent("scripts/checks.sh"), encoding: .utf8)
        let coverage = try String(contentsOf: root.appendingPathComponent("scripts/coverage.sh"), encoding: .utf8)

        #expect(checks.contains("run-tests-isolated.sh\" swift test --quiet"))
        #expect(coverage.contains("run-tests-isolated.sh\" swift test --enable-code-coverage"))
    }
}
