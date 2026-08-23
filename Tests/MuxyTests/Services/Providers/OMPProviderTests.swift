import Foundation
import Testing

@testable import Muxy

@Suite("OMPProvider")
struct OMPProviderTests {
    private let provider = OMPProvider()

    @Test("identity matches Oh My Pi")
    func identity() {
        #expect(provider.id == "omp")
        #expect(provider.displayName == "Oh My Pi")
        #expect(provider.socketTypeKey == "omp")
        #expect(provider.iconName == "omp")
        #expect(provider.executableNames == ["omp"])
        #expect(provider.hookScriptName == "muxy-omp-extension")
        #expect(provider.hookScriptExtension == "ts")
        #expect(provider.settingsKey == "muxy.notifications.provider.omp.enabled")
    }

    @Test("isEnabled stores and retrieves value via UserDefaults")
    func isEnabledStorage() {
        let key = provider.settingsKey
        let defaults = UserDefaults.standard

        defaults.removeObject(forKey: key)
        #expect(defaults.bool(forKey: key, fallback: true) == true)

        provider.isEnabled = false
        #expect(provider.isEnabled == false)

        provider.isEnabled = true
        #expect(provider.isEnabled == true)

        defaults.removeObject(forKey: key)
    }

    @Test("agentLaunchConfiguration produces deterministic headless invocation")
    func agentLaunchConfiguration() {
        let configuration = provider.agentLaunchConfiguration
        #expect(configuration.executable == "omp")
        #expect(configuration.headlessArguments == [
            "-p",
            "--no-session",
            "--no-tools",
            "--no-title",
            "--mode",
            "text",
        ])
        #expect(configuration.modelArgument == "--model")
        #expect(configuration.environment["PI_NO_PTY"] == "1")

        let prompt = "Generate a commit message"
        let invocation = configuration.invocation(prompt: prompt, model: "opus")
        #expect(invocation?.executable == "omp")
        #expect(invocation?.arguments == [
            "-p",
            "--no-session",
            "--no-tools",
            "--no-title",
            "--mode",
            "text",
            "--model",
            "opus",
            prompt,
        ])

        let leadingDash = configuration.invocation(prompt: "-m 'Initial commit'")
        #expect(leadingDash?.arguments == [
            "-p",
            "--no-session",
            "--no-tools",
            "--no-title",
            "--mode",
            "text",
            " -m 'Initial commit'",
        ])

        let noModel = configuration.invocation(prompt: "Write a test")
        #expect(noModel?.arguments == [
            "-p",
            "--no-session",
            "--no-tools",
            "--no-title",
            "--mode",
            "text",
            "Write a test",
        ])

        #expect(configuration.invocation(prompt: "   \n\t  ") == nil)
    }

    @Test("discovery properties return standard arguments and working directory")
    func discoveryProperties() {
        #expect(provider.discoveryArguments == ["--version"])
        #expect(provider.discoveryWorkingDirectory == NSHomeDirectory())
    }

    @Test("discoveryDetails extracts version from CLI output")
    func discoveryDetails() {
        let details1 = provider.discoveryDetails(from: "omp/18.0.0")
        #expect(details1.version == "18.0.0")
        #expect(details1.state == .ready)

        let details2 = provider.discoveryDetails(from: "omp v18.1.0 (darwin-arm64)")
        #expect(details2.version == "18.1.0")
        #expect(details2.state == .ready)

        let details3 = provider.discoveryDetails(from: "18.2.3-beta.1\n")
        #expect(details3.version == "18.2.3-beta.1")
        #expect(details3.state == .ready)

        let details4 = provider.discoveryDetails(from: "v18.5.0")
        #expect(details4.version == "18.5.0")
        #expect(details4.state == .ready)

        let details5 = provider.discoveryDetails(from: "")
        #expect(details5.version == nil)
        #expect(details5.state == .ready)
    }
    @Test("configPaths and hasManagedState track installed state")
    func managedStateAndConfigPaths() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()
        #expect(!provider.hasManagedState())
        #expect(provider.configPaths == [fixture.destinationURL.path])
        try provider.install(hookScriptPath: fixture.sourceURL.path)
        #expect(provider.hasManagedState())
    }

    @Test("verify returns satisfied when file, content, and permissions match")
    func verifySatisfied() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()
        try provider.install(hookScriptPath: fixture.sourceURL.path)
        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .satisfied)
    }

    @Test("verify returns needsRepair when destination is missing, content differs, or permissions wrong")
    func verifyFailureModes() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()

        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .needsRepair)

        try provider.install(hookScriptPath: fixture.sourceURL.path)
        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .satisfied)

        try Data("different content".utf8).write(to: fixture.sourceURL)
        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .needsRepair)

        try Data("extension source".utf8).write(to: fixture.sourceURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o644],
            ofItemAtPath: fixture.destinationURL.path
        )
        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .needsRepair)
    }

    @Test("install replaces a managed destination symlink")
    func installReplacesDestinationSymlink() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let target = fixture.rootURL.appendingPathComponent("external.ts")
        try Data("external".utf8).write(to: target)
        try FileManager.default.createSymbolicLink(
            atPath: fixture.destinationURL.path,
            withDestinationPath: target.path
        )
        let provider = fixture.provider()

        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .needsRepair)
        try provider.install(hookScriptPath: fixture.sourceURL.path)

        #expect(try Data(contentsOf: fixture.destinationURL) == Data("extension source".utf8))
        #expect(try Data(contentsOf: target) == Data("external".utf8))
        #expect((try? FileManager.default.destinationOfSymbolicLink(atPath: fixture.destinationURL.path)) == nil)
    }

    @Test("install creates an auto-discovered extension with 0o600 permissions")
    func installCreatesExtension() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()

        try provider.install(hookScriptPath: fixture.sourceURL.path)

        let installedData = try Data(contentsOf: fixture.destinationURL)
        let sourceData = try Data(contentsOf: fixture.sourceURL)
        #expect(installedData == sourceData)
        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .satisfied)

        let attributes = try FileManager.default.attributesOfItem(atPath: fixture.destinationURL.path)
        let permissions = (attributes[.posixPermissions] as? NSNumber)?.intValue
        #expect(permissions == FilePermissions.privateFile)
    }

    @Test("install is idempotent and repairs permissions when existing content matches")
    func installIsIdempotent() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()

        try provider.install(hookScriptPath: fixture.sourceURL.path)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o644],
            ofItemAtPath: fixture.destinationURL.path
        )
        try provider.install(hookScriptPath: fixture.sourceURL.path)
        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .satisfied)

        #expect(HookConfigWriteLedger.shared.isSelfWrite(path: fixture.destinationURL.path))
        let attributes = try FileManager.default.attributesOfItem(atPath: fixture.destinationURL.path)
        let permissions = (attributes[.posixPermissions] as? NSNumber)?.intValue
        #expect(permissions == FilePermissions.privateFile)
    }
    @Test("install refreshes from the supplied staged extension")
    func installRefreshesFromSuppliedPath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.provider()

        try provider.install(hookScriptPath: fixture.sourceURL.path)
        try Data("updated extension source".utf8).write(to: fixture.sourceURL)
        try provider.install(hookScriptPath: fixture.sourceURL.path)

        #expect(try Data(contentsOf: fixture.destinationURL) == Data("updated extension source".utf8))
    }

    @Test("uninstall removes extension file")
    func uninstallRemovesFile() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()

        try provider.install(hookScriptPath: fixture.sourceURL.path)
        #expect(provider.isHookInstalled())
        try provider.uninstall()

        #expect(!FileManager.default.fileExists(atPath: fixture.destinationURL.path))
        #expect(!provider.isHookInstalled())
    }

    @Test("uninstall does nothing when file does not exist")
    func uninstallNoFile() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()
        try provider.uninstall()
    }

    @Test("agentCLIExecutablePath and isToolInstalled resolve .bun/bin and other candidate paths")
    func isToolInstalledFromCommonPath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        #expect(fixture.provider().agentCLIExecutablePath() == nil)
        #expect(!fixture.provider().isToolInstalled())

        let bunExecutableURL = fixture.homeURL.appendingPathComponent(".bun/bin/omp")
        try FileManager.default.createDirectory(
            at: bunExecutableURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data().write(to: bunExecutableURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: bunExecutableURL.path
        )

        #expect(fixture.provider().agentCLIExecutablePath() == bunExecutableURL.path)
        #expect(fixture.provider().isToolInstalled())
    }

    @Test("isToolInstalled checks PATH entries")
    func isToolInstalledFromPath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let binURL = fixture.rootURL.appendingPathComponent("bin")
        let executableURL = binURL.appendingPathComponent("omp")
        try FileManager.default.createDirectory(at: binURL, withIntermediateDirectories: true)
        try Data().write(to: executableURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: executableURL.path
        )

        #expect(fixture.provider(pathEnvironment: binURL.path).isToolInstalled())
    }

    @Test("isToolInstalled evaluates PATH at call time")
    func isToolInstalledUsesCurrentPathEnvironment() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let pathEnvironment = PathEnvironment()
        let provider = OMPProvider(
            homeDirectory: fixture.homeURL.path,
            pathEnvironment: { pathEnvironment.value },
            environment: { [:] }
        )

        let binURL = fixture.rootURL.appendingPathComponent("late-bin")
        let executableURL = binURL.appendingPathComponent("omp")
        try FileManager.default.createDirectory(at: binURL, withIntermediateDirectories: true)
        try Data().write(to: executableURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: executableURL.path
        )
        pathEnvironment.value = binURL.path

        #expect(provider.isToolInstalled())
    }

    @Test("install throws when resource is missing and error description is formatted")
    func installThrowsWhenResourceMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = OMPProvider(homeDirectory: fixture.homeURL.path, pathEnvironment: "", environment: { [:] })
        let missingURL = fixture.rootURL.appendingPathComponent("missing.ts")

        #expect(throws: OMPProviderError.hookResourceNotFound) {
            try provider.install(hookScriptPath: missingURL.path)
        }

        let error = OMPProviderError.hookResourceNotFound
        #expect(error.errorDescription == "OMP extension file (muxy-omp-extension.ts) not found at the staged hook path")
    }

    @Test("extension paths follow OMP profile and agent directory settings")
    func extensionPathsFollowOMPSettings() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let defaultPath = fixture.homeURL.appendingPathComponent(".omp/agent/extensions/muxy-notify.ts").path
        let defaultProvider = OMPProvider(
            homeDirectory: fixture.homeURL.path,
            pathEnvironment: "",
            environment: { [:] }
        )
        #expect(defaultProvider.configPaths == [defaultPath])

        let customAgentDir = fixture.rootURL.appendingPathComponent("custom-agent")
        let customProvider = OMPProvider(
            homeDirectory: fixture.homeURL.path,
            pathEnvironment: "",
            environment: { ["PI_CODING_AGENT_DIR": customAgentDir.path] }
        )
        #expect(customProvider.configPaths == [
            customAgentDir.appendingPathComponent("extensions/muxy-notify.ts").path,
        ])

        let profileProvider = OMPProvider(
            homeDirectory: fixture.homeURL.path,
            pathEnvironment: "",
            environment: {
                [
                    "OMP_PROFILE": "work",
                    "PI_PROFILE": "ignored",
                    "PI_CODING_AGENT_DIR": customAgentDir.path,
                ]
            }
        )
        #expect(profileProvider.configPaths == [
            fixture.homeURL.appendingPathComponent(".omp/profiles/work/agent/extensions/muxy-notify.ts").path,
        ])

        for profile in ["", "default", "../invalid"] {
            let provider = OMPProvider(
                homeDirectory: fixture.homeURL.path,
                pathEnvironment: "",
                environment: {
                    [
                        "OMP_PROFILE": profile,
                        "PI_PROFILE": "ignored",
                        "PI_CODING_AGENT_DIR": customAgentDir.path,
                    ]
                }
            )
            #expect(provider.configPaths == [
                customAgentDir.appendingPathComponent("extensions/muxy-notify.ts").path,
            ])
        }

        let customConfigProvider = OMPProvider(
            homeDirectory: fixture.homeURL.path,
            pathEnvironment: "",
            environment: { ["PI_CONFIG_DIR": ".omp-custom", "PI_PROFILE": "legacy"] }
        )
        #expect(customConfigProvider.configPaths == [
            fixture.homeURL.appendingPathComponent(".omp-custom/profiles/legacy/agent/extensions/muxy-notify.ts").path,
        ])
    }

    @Test("install manages existing OMP profiles")
    func installManagesExistingProfiles() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let workProfile = fixture.homeURL.appendingPathComponent(".omp/profiles/work")
        let personalProfile = fixture.homeURL.appendingPathComponent(".omp/profiles/personal")
        try FileManager.default.createDirectory(at: workProfile, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: personalProfile, withIntermediateDirectories: true)

        let provider = fixture.provider()
        try provider.install(hookScriptPath: fixture.sourceURL.path)

        let sourceData = try Data(contentsOf: fixture.sourceURL)
        for profile in [personalProfile, workProfile] {
            let installed = profile.appendingPathComponent("agent/extensions/muxy-notify.ts")
            #expect(try Data(contentsOf: installed) == sourceData)
        }
        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .satisfied)
    }
    private final class PathEnvironment: @unchecked Sendable {
        private let lock = NSLock()
        private var storage = ""

        var value: String {
            get { lock.withLock { storage } }
            set { lock.withLock { storage = newValue } }
        }
    }

    private struct Fixture {
        let rootURL: URL
        let homeURL: URL
        let sourceURL: URL
        let destinationURL: URL

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("OMPProviderTests-\(UUID().uuidString)", isDirectory: true)
            homeURL = rootURL.appendingPathComponent("home", isDirectory: true)
            sourceURL = rootURL.appendingPathComponent("muxy-omp-extension.ts")
            destinationURL = homeURL.appendingPathComponent(".omp/agent/extensions/muxy-notify.ts")

            try FileManager.default.createDirectory(
                at: destinationURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try Data("extension source".utf8).write(to: sourceURL)
        }

        func provider(pathEnvironment: String = "") -> OMPProvider {
            OMPProvider(
                homeDirectory: homeURL.path,
                pathEnvironment: pathEnvironment,
                environment: { [:] }
            )
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootURL)
        }
    }
}
