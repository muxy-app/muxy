import Darwin
import Foundation
import Testing

@testable import Muxy

@Suite("ForegroundProcessInspector")
struct ForegroundProcessInspectorTests {
    @Test("resolves executable name candidates for the current process")
    func resolvesCurrentProcess() {
        let pid = UInt64(getpid())
        let candidates = ForegroundProcessInspector.executableNameCandidates(pid: pid)
        #expect(!candidates.isEmpty)
        #expect(candidates.allSatisfy { !$0.contains("/") })
    }

    @Test("returns no candidates for an invalid pid")
    func returnsEmptyForInvalidPID() {
        #expect(ForegroundProcessInspector.executableNameCandidates(pid: 0).isEmpty)
    }

    @Test("detects an interpreter-launched agent via the script argument")
    func detectsInterpreterLaunchedAgent() {
        let executables = [AIAgentExecutable(providerID: "codex", executableNames: ["codex"])]
        let nodeWrapperCandidates = ["node", "node", "codex"]
        #expect(
            AIAgentDetector.providerID(forCandidateNames: nodeWrapperCandidates, executables: executables) == "codex"
        )
    }
}
