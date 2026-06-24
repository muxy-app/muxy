import Foundation
import Testing
@testable import Muxy

@Suite("ProviderExecutableLocator")
struct ProviderExecutableLocatorTests {
    @Test("temp home with empty PATH excludes system-wide bins")
    func tempHomeExcludesSystemWideBins() {
        let dirs = ProviderExecutableLocator.candidateDirectories(
            homeDirectory: "/tmp/fixture-home",
            pathEnvironment: "",
            includeSystemWide: false,
            homeRelativeBins: [".local/bin", ".npm-global/bin"]
        )
        #expect(dirs == [
            "/tmp/fixture-home/.local/bin",
            "/tmp/fixture-home/.npm-global/bin",
        ])
        #expect(!dirs.contains("/usr/local/bin"))
        #expect(!dirs.contains("/opt/homebrew/bin"))
    }

    @Test("includeSystemWide adds usr/local and homebrew bins")
    func includeSystemWideAddsStandardBins() {
        let dirs = ProviderExecutableLocator.candidateDirectories(
            homeDirectory: "/Users/real",
            pathEnvironment: "",
            includeSystemWide: true
        )
        #expect(dirs.contains("/usr/local/bin"))
        #expect(dirs.contains("/opt/homebrew/bin"))
        #expect(dirs.contains("/Users/real/.local/bin"))
    }

    @Test("PATH entries are appended after home bins")
    func pathEnvironmentIsAppended() {
        let dirs = ProviderExecutableLocator.candidateDirectories(
            homeDirectory: "/tmp/home",
            pathEnvironment: "/custom/a:/custom/b",
            includeSystemWide: false
        )
        #expect(dirs == [
            "/tmp/home/.local/bin",
            "/custom/a",
            "/custom/b",
        ])
    }

    @Test("duplicate directories are deduplicated in order")
    func duplicatesAreDeduplicated() {
        let dirs = ProviderExecutableLocator.candidateDirectories(
            homeDirectory: "/tmp/home",
            pathEnvironment: "/tmp/home/.local/bin:/other",
            includeSystemWide: false
        )
        #expect(dirs == [
            "/tmp/home/.local/bin",
            "/other",
        ])
    }

    @Test("isInstalled is false when injectable probe finds nothing")
    func isInstalledFalseWithStubProbe() {
        let installed = ProviderExecutableLocator.isInstalled(
            names: ["codex"],
            homeDirectory: "/tmp/fixture-home",
            pathEnvironment: "",
            includeSystemWide: false,
            homeRelativeBins: [".local/bin", ".npm-global/bin"],
            isExecutable: { _ in false }
        )
        #expect(!installed)
    }

    @Test("isInstalled is true when injectable probe matches a candidate path")
    func isInstalledTrueWithStubProbe() {
        let installed = ProviderExecutableLocator.isInstalled(
            names: ["grok"],
            homeDirectory: "/tmp/fixture-home",
            pathEnvironment: "",
            includeSystemWide: false,
            isExecutable: { $0 == "/tmp/fixture-home/.local/bin/grok" }
        )
        #expect(installed)
    }

    @Test("isInstalled ignores system-wide paths when includeSystemWide is false even if probe would accept them")
    func doesNotProbeSystemWideWhenDisabled() {
        var probed: [String] = []
        let installed = ProviderExecutableLocator.isInstalled(
            names: ["codex"],
            homeDirectory: "/tmp/fixture-home",
            pathEnvironment: "",
            includeSystemWide: false,
            homeRelativeBins: [".local/bin", ".npm-global/bin"],
            isExecutable: { path in
                probed.append(path)
                return path == "/opt/homebrew/bin/codex"
            }
        )
        #expect(!installed)
        #expect(!probed.contains("/opt/homebrew/bin/codex"))
        #expect(!probed.contains("/usr/local/bin/codex"))
    }
}
