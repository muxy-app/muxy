import Foundation
import Testing

@testable import Muxy

@Suite("HookInstaller")
@MainActor
struct HookInstallerTests {
    private static func installer(health: HookHealthStore) -> HookInstaller {
        HookInstaller(
            hookScriptPath: { _, _ in Fixture.stagedScriptPath },
            stagedFileExists: { _ in true },
            stagedFileExecutable: { _ in true },
            health: health
        )
    }

    @Test("verify reports satisfied when config references the current staged path")
    func verifySatisfiedForCurrentStagedPath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.grokProvider()
        try provider.install(hookScriptPath: Fixture.stagedScriptPath)

        #expect(provider.verify(hookScriptPath: Fixture.stagedScriptPath) == .satisfied)
    }

    @Test("verify needs repair when config references a stale staged path")
    func verifyNeedsRepairForStalePath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.grokProvider()
        try provider.install(hookScriptPath: "/old/muxy-grok-hook.sh")

        #expect(provider.verify(hookScriptPath: Fixture.stagedScriptPath) == .needsRepair)
    }

    @Test("reconcile repairs a config broken externally")
    func reconcileRepairsBrokenConfig() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.grokProvider(toolInstalled: true)
        provider.isEnabled = true
        defer { provider.isEnabled = false }
        try provider.install(hookScriptPath: Fixture.stagedScriptPath)

        try fixture.writeGrokHooks(["hooks": [:]])
        #expect(provider.verify(hookScriptPath: Fixture.stagedScriptPath) == .needsRepair)

        let health = HookHealthStore()
        let outcome = Self.installer(health: health).reconcile(provider)

        #expect(outcome == .repaired)
        #expect(provider.verify(hookScriptPath: Fixture.stagedScriptPath) == .satisfied)
        #expect(health.health(for: provider.id).installState == .installed)
        #expect(health.health(for: provider.id).lastRepairedAt != nil)
    }

    @Test("reconcile repairs a stale staged path")
    func reconcileRepairsStalePath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.grokProvider(toolInstalled: true)
        provider.isEnabled = true
        defer { provider.isEnabled = false }
        try provider.install(hookScriptPath: "/old/muxy-grok-hook.sh")

        let outcome = Self.installer(health: HookHealthStore()).reconcile(provider)

        #expect(outcome == .repaired)
        #expect(provider.verify(hookScriptPath: Fixture.stagedScriptPath) == .satisfied)
    }

    @Test("reconcile reports Codex conflict without clobbering config")
    func reconcileReportsCodexConflict() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.codexProvider()
        provider.isEnabled = true
        defer { provider.isEnabled = false }
        try fixture.writeCodexConfig("[[hooks.Stop]]")

        let health = HookHealthStore()
        let outcome = Self.installer(health: health).reconcile(provider)

        guard case .conflict = outcome else {
            Issue.record("expected conflict outcome, got \(outcome)")
            return
        }
        #expect(!FileManager.default.fileExists(atPath: fixture.codexHooksURL.path))
        guard case .conflict = health.health(for: provider.id).installState else {
            Issue.record("expected conflict install state")
            return
        }
    }

    @Test("reconcile reports cliMissing when hook absent and CLI not installed")
    func reconcileReportsCLIMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.grokProvider(toolInstalled: false)
        provider.isEnabled = true
        defer { provider.isEnabled = false }

        let health = HookHealthStore()
        let outcome = Self.installer(health: health).reconcile(provider)

        #expect(outcome == .cliMissing)
        #expect(health.health(for: provider.id).installState == .cliMissing)
    }

    @Test("reconcile fails when staged resource is missing")
    func reconcileFailsWhenStagedResourceMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.grokProvider(toolInstalled: true)
        provider.isEnabled = true
        defer { provider.isEnabled = false }

        let installer = HookInstaller(
            hookScriptPath: { _, _ in Fixture.stagedScriptPath },
            stagedFileExists: { _ in false },
            stagedFileExecutable: { _ in true },
            health: HookHealthStore()
        )
        let outcome = installer.reconcile(provider)

        guard case .failed = outcome else {
            Issue.record("expected failed outcome, got \(outcome)")
            return
        }
    }

    @Test("disabled provider is skipped and health reset")
    func disabledProviderSkipped() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.grokProvider()
        provider.isEnabled = false

        let outcome = Self.installer(health: HookHealthStore()).reconcile(provider)
        #expect(outcome == .skippedDisabled)
    }

    private struct Fixture {
        static let stagedScriptPath = "/tmp/muxy-staged-hook.sh"
        let rootURL: URL
        let homeURL: URL
        let codexHooksURL: URL

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("HookInstallerTests-\(UUID().uuidString)", isDirectory: true)
            homeURL = rootURL.appendingPathComponent("home", isDirectory: true)
            codexHooksURL = homeURL.appendingPathComponent(".codex/hooks.json")
            try FileManager.default.createDirectory(at: homeURL, withIntermediateDirectories: true)
        }

        func grokProvider(toolInstalled: Bool = false) -> GrokProvider {
            if toolInstalled {
                let binURL = rootURL.appendingPathComponent("bin")
                let executableURL = binURL.appendingPathComponent("grok")
                try? FileManager.default.createDirectory(at: binURL, withIntermediateDirectories: true)
                try? Data().write(to: executableURL)
                try? FileManager.default.setAttributes(
                    [.posixPermissions: FilePermissions.executable],
                    ofItemAtPath: executableURL.path
                )
                return GrokProvider(homeDirectory: homeURL.path, pathEnvironment: binURL.path)
            }
            return GrokProvider(homeDirectory: homeURL.path, pathEnvironment: "")
        }

        func codexProvider() -> CodexProvider {
            CodexProvider(homeDirectory: homeURL.path, pathEnvironment: "", hooksPath: codexHooksURL.path)
        }

        func writeGrokHooks(_ settings: [String: Any]) throws {
            let url = homeURL.appendingPathComponent(".grok/hooks/muxy-notify.json")
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: url, options: .atomic)
        }

        func writeCodexConfig(_ config: String) throws {
            let url = homeURL.appendingPathComponent(".codex/config.toml")
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try config.write(to: url, atomically: true, encoding: .utf8)
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootURL)
        }
    }
}
