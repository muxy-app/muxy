import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionSnapshot")
struct ExtensionSnapshotTests {
    @Test("canSubscribe accepts events declared in manifest")
    func canSubscribeAcceptsDeclaredEvents() {
        let entry = NotificationSocketServer.ExtensionSnapshotEntry(
            allowedEvents: ["pane.created", "tab.focused"],
            commandEvents: [],
            permissions: [],
            token: "test-token"
        )
        #expect(NotificationSocketServer.canSubscribeForTesting(entry: entry, to: "pane.created"))
        #expect(NotificationSocketServer.canSubscribeForTesting(entry: entry, to: "tab.focused"))
        #expect(!NotificationSocketServer.canSubscribeForTesting(entry: entry, to: "pane.closed"))
    }

    @Test("canSubscribe auto-allows the extension's own command events")
    func canSubscribeAllowsOwnCommands() {
        let entry = NotificationSocketServer.ExtensionSnapshotEntry(
            allowedEvents: [],
            commandEvents: ["command.ping", "command.run"],
            permissions: [],
            token: "test-token"
        )
        #expect(NotificationSocketServer.canSubscribeForTesting(entry: entry, to: "command.ping"))
        #expect(NotificationSocketServer.canSubscribeForTesting(entry: entry, to: "command.run"))
        #expect(!NotificationSocketServer.canSubscribeForTesting(entry: entry, to: "command.other"))
    }
}

@Suite("ExtensionStore snapshot")
@MainActor
struct ExtensionStoreSnapshotBuildingTests {
    @Test("snapshotForSocketServer includes enabled extensions only")
    func snapshotIncludesEnabledOnly() throws {
        let enabledDir = try makeTemporaryExtension(
            manifest: """
            {
                "name": "enabled-ext",
                "version": "1.0.0",
                "events": ["pane.created"],
                "commands": [{ "id": "ping", "title": "Ping" }],
                "permissions": ["panes:read", "notifications:write"]
            }
            """
        )
        let disabledDir = try makeTemporaryExtension(
            manifest: """
            {
                "name": "disabled-ext",
                "version": "1.0.0",
                "events": ["pane.closed"]
            }
            """
        )
        defer {
            try? FileManager.default.removeItem(at: enabledDir)
            try? FileManager.default.removeItem(at: disabledDir)
        }

        let enabled = try ExtensionManifestLoader.load(from: enabledDir)
        let disabled = try ExtensionManifestLoader.load(from: disabledDir)
        let snapshot = ExtensionStore.buildSnapshotForTesting(
            from: [(enabled, isEnabled: true), (disabled, isEnabled: false)]
        )

        let entry = try #require(snapshot.entries["enabled-ext"])
        #expect(entry.allowedEvents == ["pane.created"])
        #expect(entry.commandEvents == ["command.ping"])
        #expect(entry.permissions == [.panesRead, .notificationsWrite])
        #expect(snapshot.entries["disabled-ext"] == nil)
    }

    private func makeTemporaryExtension(manifest: String) throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ext-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try ExtensionManifestFixture.write(flatManifest: manifest, to: directory)
        return directory
    }
}

@Suite("ExtensionStore termination classification")
struct ExtensionStoreTerminationClassificationTests {
    @Test("intentional stop is reported as stopped regardless of exit status")
    func intentionalStopMapsToStopped() {
        #expect(ExtensionStore.classifyTermination(wasIntentional: true, terminationStatus: 0) == .stopped)
        #expect(ExtensionStore.classifyTermination(wasIntentional: true, terminationStatus: 15) == .stopped)
        #expect(ExtensionStore.classifyTermination(wasIntentional: true, terminationStatus: 1) == .stopped)
    }

    @Test("unintended zero exit is reported as exitedCleanly")
    func zeroExitMapsToCleanly() {
        #expect(ExtensionStore.classifyTermination(wasIntentional: false, terminationStatus: 0) == .exitedCleanly)
    }

    @Test("unintended non-zero exit reports the underlying status")
    func nonZeroExitMapsToStatus() {
        #expect(ExtensionStore.classifyTermination(wasIntentional: false, terminationStatus: 15) == .exitedWithStatus(15))
        #expect(ExtensionStore.classifyTermination(wasIntentional: false, terminationStatus: 1) == .exitedWithStatus(1))
    }
}

@Suite("ExtensionStore crash restart policy")
struct ExtensionStoreCrashRestartTests {
    @Test("a crash restarts while attempts remain")
    func restartsWhileAttemptsRemain() {
        #expect(ExtensionStore.shouldRestartAfterCrash(isEnabled: true, attempts: 0))
        #expect(ExtensionStore.shouldRestartAfterCrash(isEnabled: true, attempts: 4))
    }

    @Test("a crash stops restarting once the attempt budget is exhausted")
    func stopsAfterBudgetExhausted() {
        #expect(!ExtensionStore.shouldRestartAfterCrash(isEnabled: true, attempts: 5))
        #expect(!ExtensionStore.shouldRestartAfterCrash(isEnabled: true, attempts: 6))
    }

    @Test("a disabled extension never restarts")
    func disabledNeverRestarts() {
        #expect(!ExtensionStore.shouldRestartAfterCrash(isEnabled: false, attempts: 0))
    }
}
