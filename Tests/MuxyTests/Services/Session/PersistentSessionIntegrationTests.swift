import Foundation
import MuxySessionProtocol
import Testing

@testable import Muxy

@Suite("TerminalPersistentSessionPolicy")
struct TerminalPersistentSessionPolicyTests {
    @Test("uses a background session for local panes when enabled and available")
    func enablesForLocalPanes() {
        #expect(TerminalPersistentSessionPolicy.usesPersistentSession(
            preferenceEnabled: true,
            workspaceContext: .local,
            isAvailable: true
        ))
    }

    @Test("stays off when the preference is disabled")
    func respectsPreference() {
        #expect(!TerminalPersistentSessionPolicy.usesPersistentSession(
            preferenceEnabled: false,
            workspaceContext: .local,
            isAvailable: true
        ))
    }

    @Test("stays off when the helper binary is unavailable")
    func requiresAvailability() {
        #expect(!TerminalPersistentSessionPolicy.usesPersistentSession(
            preferenceEnabled: true,
            workspaceContext: .local,
            isAvailable: false
        ))
    }

    @Test("never applies to remote panes")
    func excludesRemotePanes() {
        let destination = SSHDestination(host: "example.com", user: "test")
        #expect(!TerminalPersistentSessionPolicy.usesPersistentSession(
            preferenceEnabled: true,
            workspaceContext: .ssh(destination),
            isAvailable: true
        ))
    }
}

@Suite("TerminalTTYForegroundProcess")
struct TerminalTTYForegroundProcessTests {
    @Test("returns nothing when the tty has no processes")
    func handlesEmptyTTY() {
        #expect(TerminalTTYForegroundProcess.select(from: []) == nil)
    }

    @Test("returns nothing when no foreground group is set")
    func handlesMissingForegroundGroup() {
        let entries = [TTYProcessEntry(processID: 10, processGroupID: 10, foregroundGroupID: 0)]
        #expect(TerminalTTYForegroundProcess.select(from: entries) == nil)
    }

    @Test("returns the foreground group leader")
    func returnsGroupLeader() {
        let entries = [
            TTYProcessEntry(processID: 10, processGroupID: 10, foregroundGroupID: 42),
            TTYProcessEntry(processID: 42, processGroupID: 42, foregroundGroupID: 42),
            TTYProcessEntry(processID: 43, processGroupID: 42, foregroundGroupID: 42),
        ]
        #expect(TerminalTTYForegroundProcess.select(from: entries) == 42)
    }

    @Test("falls back to the lowest member when the group leader has gone")
    func fallsBackToGroupMember() {
        let entries = [
            TTYProcessEntry(processID: 10, processGroupID: 10, foregroundGroupID: 42),
            TTYProcessEntry(processID: 44, processGroupID: 42, foregroundGroupID: 42),
            TTYProcessEntry(processID: 43, processGroupID: 42, foregroundGroupID: 42),
        ]
        #expect(TerminalTTYForegroundProcess.select(from: entries) == 43)
    }

    @Test("returns nothing when the foreground group has no surviving members")
    func handlesVanishedGroup() {
        let entries = [TTYProcessEntry(processID: 10, processGroupID: 10, foregroundGroupID: 42)]
        #expect(TerminalTTYForegroundProcess.select(from: entries) == nil)
    }

    @Test("treats the shell itself as no running command")
    func detectsIdleShell() {
        #expect(!TerminalTTYForegroundProcess.isRunningCommand(foregroundProcessID: 900, shellProcessID: 900))
    }

    @Test("treats any other foreground process as a running command")
    func detectsRunningCommand() {
        #expect(TerminalTTYForegroundProcess.isRunningCommand(foregroundProcessID: 950, shellProcessID: 900))
    }

    @Test("reports no running command without a resolvable foreground process")
    func handlesUnknownForeground() {
        #expect(!TerminalTTYForegroundProcess.isRunningCommand(foregroundProcessID: nil, shellProcessID: 900))
        #expect(!TerminalTTYForegroundProcess.isRunningCommand(foregroundProcessID: 0, shellProcessID: 900))
    }

    @Test("returns nothing for an unknown tty device")
    func handlesUnknownDevice() {
        #expect(TerminalTTYForegroundProcess.entries(ttyDevice: 0).isEmpty)
        #expect(TerminalTTYForegroundProcess.processID(ttyDevice: 0) == nil)
    }
}

@Suite("PersistentSessionPaths")
struct PersistentSessionPathsTests {
    @Test("accepts paths that fit a unix socket address")
    func acceptsShortPaths() {
        #expect(PersistentSessionPaths.fits("/tmp/muxy/control.sock"))
        #expect(PersistentSessionPaths.fits(String(repeating: "a", count: 103)))
    }

    @Test("rejects paths at or beyond the sun_path limit")
    func rejectsLongPaths() {
        #expect(!PersistentSessionPaths.fits(String(repeating: "a", count: 104)))
        #expect(!PersistentSessionPaths.fits(String(repeating: "a", count: 400)))
    }

    @Test("builds the socket path inside the app support directory")
    func buildsPreferredPath() {
        let directory = URL(fileURLWithPath: "/Users/test/Library/Application Support/Muxy", isDirectory: true)
        let releasePath = PersistentSessionPaths.preferredSocketPath(appSupportDirectory: directory, isDevelopment: false)
        let developmentPath = PersistentSessionPaths.preferredSocketPath(appSupportDirectory: directory, isDevelopment: true)
        #expect(releasePath == "/Users/test/Library/Application Support/Muxy/sessions/control.sock")
        #expect(developmentPath == "/Users/test/Library/Application Support/Muxy/sessions-dev/control.sock")
    }

    @Test("scopes the fallback path to the user")
    func buildsFallbackPath() {
        #expect(PersistentSessionPaths.fallbackSocketPath(userID: 501, isDevelopment: false) == "/tmp/muxy-501/control.sock")
        #expect(PersistentSessionPaths.fallbackSocketPath(userID: 501, isDevelopment: true) == "/tmp/muxy-dev-501/control.sock")
        #expect(PersistentSessionPaths.fits(PersistentSessionPaths.fallbackSocketPath(userID: 4_294_967_295, isDevelopment: false)))
        #expect(PersistentSessionPaths.fits(PersistentSessionPaths.fallbackSocketPath(userID: 4_294_967_295, isDevelopment: true)))
    }

    @Test("keeps development and release paths separate")
    func separatesDevelopmentAndReleasePaths() {
        let directory = URL(fileURLWithPath: "/Users/test/Library/Application Support/Muxy", isDirectory: true)
        let releasePreferred = PersistentSessionPaths.preferredSocketPath(appSupportDirectory: directory, isDevelopment: false)
        let developmentPreferred = PersistentSessionPaths.preferredSocketPath(appSupportDirectory: directory, isDevelopment: true)
        let releaseFallback = PersistentSessionPaths.fallbackSocketPath(userID: 501, isDevelopment: false)
        let developmentFallback = PersistentSessionPaths.fallbackSocketPath(userID: 501, isDevelopment: true)
        #expect(releasePreferred != developmentPreferred)
        #expect(releaseFallback != developmentFallback)
    }

    private func makeFallbackRoot() throws -> URL {
        let root = URL(fileURLWithPath: "/tmp", isDirectory: true)
            .appendingPathComponent("mxf-\(UUID().uuidString.prefix(6))", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    private var overlongAppSupportDirectory: URL {
        URL(fileURLWithPath: "/" + String(repeating: "d", count: 120), isDirectory: true)
    }

    @Test("uses the app support directory when the path fits")
    func resolvesPreferredPath() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("mx-\(UUID().uuidString.prefix(6))")
        defer { try? FileManager.default.removeItem(at: root) }
        let resolved = try PersistentSessionPaths.resolveSocketPath(appSupportDirectory: root, userID: getuid(), isDevelopment: false)
        #expect(resolved == PersistentSessionPaths.preferredSocketPath(appSupportDirectory: root, isDevelopment: false))
        #expect(FileManager.default.fileExists(atPath: URL(fileURLWithPath: resolved).deletingLastPathComponent().path))
    }

    @Test("resolves the development socket path into its own directory")
    func resolvesDevelopmentPreferredPath() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("mx-\(UUID().uuidString.prefix(6))")
        defer { try? FileManager.default.removeItem(at: root) }
        let resolved = try PersistentSessionPaths.resolveSocketPath(appSupportDirectory: root, userID: getuid(), isDevelopment: true)
        #expect(resolved == PersistentSessionPaths.preferredSocketPath(appSupportDirectory: root, isDevelopment: true))
        #expect(resolved.contains("sessions-dev"))
        #expect(FileManager.default.fileExists(atPath: URL(fileURLWithPath: resolved).deletingLastPathComponent().path))
    }

    @Test("falls back to a user-scoped directory when the app support path is too long")
    func resolvesFallbackPath() throws {
        let root = try makeFallbackRoot()
        defer { try? FileManager.default.removeItem(at: root) }

        let resolved = try PersistentSessionPaths.resolveSocketPath(
            appSupportDirectory: overlongAppSupportDirectory,
            userID: getuid(),
            fallbackRoot: root.path,
            isDevelopment: false
        )
        #expect(resolved == PersistentSessionPaths.fallbackSocketPath(userID: getuid(), root: root.path, isDevelopment: false))
        #expect(FileManager.default.fileExists(atPath: URL(fileURLWithPath: resolved).deletingLastPathComponent().path))
    }

    @Test("tightens a fallback directory we own but that is too permissive")
    func tightensLoosePermissions() throws {
        let root = try makeFallbackRoot()
        defer { try? FileManager.default.removeItem(at: root) }

        let directory = URL(fileURLWithPath: PersistentSessionPaths.fallbackSocketPath(userID: getuid(), root: root.path, isDevelopment: false))
            .deletingLastPathComponent()
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o777]
        )

        _ = try PersistentSessionPaths.resolveSocketPath(
            appSupportDirectory: overlongAppSupportDirectory,
            userID: getuid(),
            fallbackRoot: root.path,
            isDevelopment: false
        )

        let permissions = try #require(
            FileManager.default.attributesOfItem(atPath: directory.path)[.posixPermissions] as? NSNumber
        )
        #expect(permissions.int16Value & 0o077 == 0)
    }

    @Test("refuses a fallback directory owned by another user")
    func rejectsForeignFallback() throws {
        let root = try makeFallbackRoot()
        defer { try? FileManager.default.removeItem(at: root) }

        let foreignUserID = getuid() &+ 1
        let directory = URL(fileURLWithPath: PersistentSessionPaths.fallbackSocketPath(userID: foreignUserID, root: root.path, isDevelopment: false))
            .deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)

        #expect(throws: PersistentSessionPathError.insecureFallbackDirectory) {
            try PersistentSessionPaths.resolveSocketPath(
                appSupportDirectory: overlongAppSupportDirectory,
                userID: foreignUserID,
                fallbackRoot: root.path,
                isDevelopment: false
            )
        }
    }

    @Test("locates the helper binary next to the app executable")
    func locatesBinary() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("mx-\(UUID().uuidString.prefix(6))")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let executable = root.appendingPathComponent("Muxy")
        let helper = root.appendingPathComponent(PersistentSessionPaths.binaryName)
        #expect(PersistentSessionPaths.binaryURL(executableURL: executable) == nil)

        try Data("#!/bin/sh\n".utf8).write(to: helper)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)
        #expect(PersistentSessionPaths.binaryURL(executableURL: executable)?.path == helper.path)
        #expect(PersistentSessionPaths.binaryURL(executableURL: nil) == nil)
    }
}

@Suite("TerminalPersistentSessionPreferences")
struct TerminalPersistentSessionPreferencesTests {
    @Test("defaults to off")
    func defaultsToOff() {
        #expect(!TerminalPersistentSessionPreferences.defaultIsEnabled)
    }

    @Test("round-trips through user defaults")
    func roundTripsPreference() {
        let original = TerminalPersistentSessionPreferences.isEnabled
        defer { TerminalPersistentSessionPreferences.isEnabled = original }

        TerminalPersistentSessionPreferences.isEnabled = true
        #expect(TerminalPersistentSessionPreferences.isEnabled)
        TerminalPersistentSessionPreferences.isEnabled = false
        #expect(!TerminalPersistentSessionPreferences.isEnabled)
    }
}

@Suite("PersistentSessionControlClient", .enabled(if: SessionDaemonHarness.binaryURL != nil))
struct PersistentSessionControlClientTests {
    @Test("returns nothing when no daemon is listening")
    func toleratesMissingDaemon() throws {
        let client = PersistentSessionControlClient(socketPath: "/tmp/muxy-missing-\(UUID().uuidString.prefix(8)).sock")
        let identifier = try #require(SessionIdentifier(uuidString: UUID().uuidString))
        #expect(client.list().isEmpty)
        #expect(client.info(identifier: identifier) == nil)
        #expect(!client.killAll())
    }

    @Test("lists and kills sessions through the control socket")
    func listsAndKillsSessions() throws {
        let harness = try SessionDaemonHarness()
        try harness.start()
        defer { harness.stop() }

        let identifier = try #require(SessionIdentifier(uuidString: UUID().uuidString))
        let attached = try #require(SessionTestConnection(socketPath: harness.socketPath))
        defer { attached.close() }
        attached.send(harness.attachRequest(identifier: identifier, command: "sh -c 'sleep 30'"))
        _ = try #require(attached.waitForFrame(timeout: 5) { $0.kind == .attached })

        let client = PersistentSessionControlClient(socketPath: harness.socketPath)
        let listed = client.list()
        #expect(listed.count == 1)
        #expect(listed.first?.identifier == identifier)
        #expect(client.info(identifier: identifier)?.shellProcessID == listed.first?.shellProcessID)

        #expect(client.kill(identifier: identifier))
        #expect(client.list().isEmpty)
    }
}
