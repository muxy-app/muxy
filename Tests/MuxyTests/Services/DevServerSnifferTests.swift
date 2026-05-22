import Foundation
import Testing

@testable import Muxy

@Suite("DevServerSniffer", .serialized)
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
        ]
        for command in commands {
            #expect(DevServerSniffer.isDevServerCommand(command), "expected '\(command)' to be recognised")
        }
    }

    @Test("ignores broad runtime commands that frequently are not dev servers")
    func ignoresBroadRuntimeCommands() {
        #expect(!DevServerSniffer.isDevServerCommand("go run ./cmd/cli"))
        #expect(!DevServerSniffer.isDevServerCommand("cargo run -- --help"))
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

    @Test("starting a new dev command clears previously detected URLs for that pane")
    func resetsDetectionsOnNewCommand() async {
        let sniffer = DevServerSniffer.shared
        sniffer.reset()
        let paneA = UUID()

        sniffer.probeURL = { url, completion in completion(url.port == 3000) }

        sniffer.observe(command: "npm run dev", paneID: paneA)

        let firstURL = await waitForDetection(sniffer: sniffer, scope: paneA, timeout: 6.0)
        #expect(firstURL == "http://localhost:3000")

        sniffer.observe(command: "npm run dev", paneID: paneA)
        #expect(sniffer.detectedURLs(paneID: paneA).isEmpty)

        let secondURL = await waitForDetection(sniffer: sniffer, scope: paneA, timeout: 6.0)
        #expect(secondURL == "http://localhost:3000")

        sniffer.reset()
    }

    @Test("detections for different panes are independent")
    func detectionsAreScopedPerPane() async {
        let sniffer = DevServerSniffer.shared
        sniffer.reset()
        let paneA = UUID()
        let paneB = UUID()

        sniffer.probeURL = { url, completion in completion(url.port == 5173) }

        sniffer.observe(command: "vite", paneID: paneA)
        let firstURL = await waitForDetection(sniffer: sniffer, scope: paneA, timeout: 6.0)
        #expect(firstURL == "http://localhost:5173")
        #expect(sniffer.detectedURLs(paneID: paneB).isEmpty)

        sniffer.observe(command: "vite", paneID: paneB)
        let secondURL = await waitForDetection(sniffer: sniffer, scope: paneB, timeout: 6.0)
        #expect(secondURL == "http://localhost:5173")

        sniffer.reset()
    }

    private func waitForDetection(
        sniffer: DevServerSniffer,
        scope: UUID,
        timeout: TimeInterval
    ) async -> String? {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let urls = sniffer.detectedURLs(paneID: scope)
            if let first = urls.first { return first }
            try? await Task.sleep(nanoseconds: 200_000_000)
        }
        return nil
    }
}
