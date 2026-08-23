import Foundation
import Testing

@testable import Muxy

@Suite("Warning gate script")
struct WarningGateScriptTests {
    @Test("accepts successful commands without diagnostics")
    func acceptsCleanCommand() throws {
        let result = try run("printf 'Build complete\\n'")

        #expect(result.status == 0)
        #expect(result.output.contains("Build complete"))
    }

    @Test("rejects compiler and linker warning diagnostics", arguments: [
        "warning: unhandled file",
        "Source.swift:1:1: warning: deprecated API",
        "ld: warning: missing symbol",
    ])
    func rejectsWarnings(_ diagnostic: String) throws {
        let result = try run("printf '%s\\n' \(shellQuote(diagnostic))")

        #expect(result.status != 0)
        #expect(result.output.contains("Build emitted warning diagnostics"))
    }

    @Test("rejects colored warning diagnostics")
    func rejectsColoredWarning() throws {
        let result = try run("printf '\\033[33mwarning:\\033[0m deprecated API\\n'")

        #expect(result.status != 0)
    }

    @Test("rejects warnings before large output")
    func rejectsWarningBeforeLargeOutput() throws {
        let result = try run("printf 'warning: deprecated API\\n'; index=0; while [ $index -lt 2000 ]; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\\n'; index=$((index + 1)); done")

        #expect(result.status != 0)
    }

    @Test("preserves command failures")
    func preservesFailure() throws {
        let result = try run("printf 'failed\\n'; exit 7")

        #expect(result.status == 7)
    }

    @Test("ignores ordinary uses of warning")
    func ignoresOrdinaryText() throws {
        let result = try run("printf 'warning label is visible\\n'")

        #expect(result.status == 0)
    }

    private func run(_ command: String) throws -> (status: Int32, output: String) {
        let root = RepositoryRoot.find()
        let script = root.appendingPathComponent("scripts/run-without-warnings.sh")
        let process = Process()
        let output = Pipe()
        process.executableURL = script
        process.arguments = ["/bin/sh", "-c", command]
        process.standardOutput = output
        process.standardError = output

        try process.run()
        let data = output.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        return (process.terminationStatus, String(decoding: data, as: UTF8.self))
    }

    private func shellQuote(_ value: String) -> String {
        "'\(value.replacingOccurrences(of: "'", with: "'\\''"))'"
    }
}
