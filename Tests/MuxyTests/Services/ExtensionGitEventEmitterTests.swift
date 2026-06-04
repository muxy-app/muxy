import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionGitEventEmitter")
struct ExtensionGitEventEmitterTests {
    @Test("parses branch and dirty flag from porcelain branch output")
    func parsesDirty() {
        let output = "## main...origin/main [ahead 1]\n M file.swift\n?? new.txt\n"
        let state = ExtensionGitEventEmitter.parseStatus(output)
        #expect(state.branch == "main")
        #expect(state.hasChanges)
    }

    @Test("reports no changes for a clean tree")
    func parsesClean() {
        let state = ExtensionGitEventEmitter.parseStatus("## feature/x\n")
        #expect(state.branch == "feature/x")
        #expect(!state.hasChanges)
    }

    @Test("parses a detached head")
    func parsesDetached() {
        let state = ExtensionGitEventEmitter.parseStatus("## HEAD (no branch)\n M a.txt\n")
        #expect(state.branch == "HEAD")
        #expect(state.hasChanges)
    }

    @Test("emits once per repo within the dedupe window, again after it")
    func dedupesWithinWindow() {
        let emitter = ExtensionGitEventEmitter()
        #expect(emitter.shouldEmit(projectPath: "/repo", now: 1.0))
        #expect(!emitter.shouldEmit(projectPath: "/repo", now: 1.1))
        #expect(emitter.shouldEmit(projectPath: "/repo", now: 1.5))
    }

    @Test("dedupes each repo independently")
    func dedupesPerRepo() {
        let emitter = ExtensionGitEventEmitter()
        #expect(emitter.shouldEmit(projectPath: "/a", now: 1.0))
        #expect(emitter.shouldEmit(projectPath: "/b", now: 1.0))
    }
}
