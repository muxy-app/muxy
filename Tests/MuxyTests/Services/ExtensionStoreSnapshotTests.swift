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
            permissions: []
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
            permissions: []
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
            name: "enabled-ext",
            manifest: """
            {
                "name": "enabled-ext",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "events": ["pane.created"],
                "commands": [{ "id": "ping", "title": "Ping" }],
                "permissions": ["panes:read", "notifications:write"]
            }
            """
        )
        let disabledDir = try makeTemporaryExtension(
            name: "disabled-ext",
            manifest: """
            {
                "name": "disabled-ext",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "enabled": false,
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
        let snapshot = ExtensionStore.buildSnapshotForTesting(from: [enabled, disabled])

        let entry = try #require(snapshot.entries["enabled-ext"])
        #expect(entry.allowedEvents == ["pane.created"])
        #expect(entry.commandEvents == ["command.ping"])
        #expect(entry.permissions == [.panesRead, .notificationsWrite])
        #expect(snapshot.entries["disabled-ext"] == nil)
    }

    private func makeTemporaryExtension(name: String, manifest: String) throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ext-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let manifestURL = directory.appendingPathComponent("manifest.json")
        try Data(manifest.utf8).write(to: manifestURL)
        let entrypoint = directory.appendingPathComponent("run.sh")
        try Data("#!/bin/sh\n".utf8).write(to: entrypoint)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: entrypoint.path
        )
        _ = name
        return directory
    }
}
