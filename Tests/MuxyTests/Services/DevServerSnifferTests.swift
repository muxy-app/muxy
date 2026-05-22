import Foundation
import Testing

@testable import Muxy

@Suite("DevServerSniffer")
@MainActor
struct DevServerSnifferTests {
    @Test("recognises common dev server commands")
    func recognisesDevCommands() {
        let commands = [
            "npm run dev",
            "pnpm dev",
            "yarn start",
            "bun dev",
            "next dev",
            "vite",
            "vite --host 0.0.0.0",
            "python3 -m http.server",
            "uvicorn app.main:app",
            "go run ./cmd/server",
        ]
        for command in commands {
            #expect(DevServerSniffer.isDevServerCommand(command), "expected '\(command)' to be recognised")
        }
    }

    @Test("ignores unrelated commands")
    func ignoresUnrelatedCommands() {
        let commands = [
            "ls",
            "git status",
            "echo hello",
            "npm test",
            "yarn test",
            "node script.js",
        ]
        for command in commands {
            #expect(!DevServerSniffer.isDevServerCommand(command), "expected '\(command)' to not be recognised")
        }
    }
}
