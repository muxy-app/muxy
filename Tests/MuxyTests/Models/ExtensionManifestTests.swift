import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionManifestLoader")
struct ExtensionManifestTests {
    @Test("decodes a minimal manifest")
    func decodesMinimalManifest() throws {
        let json = #"""
        {
            "name": "hello",
            "version": "1.0.0",
            "entrypoint": "run.sh"
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))

        #expect(manifest.name == "hello")
        #expect(manifest.version == "1.0.0")
        #expect(manifest.entrypoint == "run.sh")
        #expect(manifest.events.isEmpty)
        #expect(manifest.commands.isEmpty)
        #expect(manifest.permissions.isEmpty)
        #expect(manifest.aiProvider == nil)
    }

    @Test("decodes full manifest with permissions, events, commands and aiProvider")
    func decodesFullManifest() throws {
        let json = #"""
        {
            "name": "demo",
            "version": "2.1",
            "description": "Test extension",
            "entrypoint": "bin/main",
            "events": ["pane.created", "tab.focused"],
            "commands": [
                { "id": "greet", "title": "Say hello", "subtitle": "demo" }
            ],
            "permissions": ["panes:read", "tabs:write"],
            "aiProvider": { "socketTypeKey": "demo", "displayName": "Demo", "iconName": "sparkles" }
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))

        #expect(manifest.description == "Test extension")
        #expect(manifest.events == ["pane.created", "tab.focused"])
        #expect(manifest.commands == [ExtensionPaletteCommand(id: "greet", title: "Say hello", subtitle: "demo")])
        #expect(manifest.permissions == [.panesRead, .tabsWrite])
        #expect(manifest.aiProvider == ExtensionAIProvider(socketTypeKey: "demo", displayName: "Demo", iconName: "sparkles"))
    }

    @Test("loads from directory and resolves entrypoint")
    func loadsFromDirectory() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "tmp-ext",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "permissions": ["panes:read"]
            }
            """,
            files: ["run.sh": "#!/bin/sh\necho hi\n"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        let ext = try ExtensionManifestLoader.load(from: directory)

        #expect(ext.id == "tmp-ext")
        #expect(ext.manifest.permissions == [.panesRead])
        #expect(ext.entrypointURL.map { FileManager.default.isExecutableFile(atPath: $0.path) } == true)
    }

    @Test("loads without an entrypoint")
    func loadsWithoutEntrypoint() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "no-entry",
                "version": "1.0.0",
                "commands": [{ "id": "ping", "title": "Ping" }]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        let ext = try ExtensionManifestLoader.load(from: directory)

        #expect(ext.manifest.entrypoint == nil)
        #expect(ext.entrypointURL == nil)
    }

    @Test("migrates legacy manifest enabled=false into ExtensionEnabledStore")
    func migratesLegacyEnabledFalse() throws {
        let extensionID = "legacy-disabled-\(UUID().uuidString)"
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "\(extensionID)",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "enabled": false
            }
            """,
            files: ["run.sh": "#!/bin/sh\n"]
        )
        defer {
            try? FileManager.default.removeItem(at: directory)
            ExtensionEnabledStore.clear(extensionID: extensionID)
        }

        _ = try ExtensionManifestLoader.load(from: directory)

        #expect(ExtensionEnabledStore.hasOverride(extensionID: extensionID))
        #expect(!ExtensionEnabledStore.isEnabled(extensionID: extensionID))
    }

    @Test("legacy migration does not overwrite an existing user override")
    func legacyMigrationRespectsExistingOverride() throws {
        let extensionID = "legacy-respect-\(UUID().uuidString)"
        ExtensionEnabledStore.setEnabled(true, extensionID: extensionID)
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "\(extensionID)",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "enabled": false
            }
            """,
            files: ["run.sh": "#!/bin/sh\n"]
        )
        defer {
            try? FileManager.default.removeItem(at: directory)
            ExtensionEnabledStore.clear(extensionID: extensionID)
        }

        _ = try ExtensionManifestLoader.load(from: directory)

        #expect(ExtensionEnabledStore.isEnabled(extensionID: extensionID))
    }

    @Test("fails when manifest missing")
    func failsWhenManifestMissing() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ext-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("fails when entrypoint not executable")
    func failsWhenEntrypointNotExecutable() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "no-exec",
                "version": "1.0.0",
                "entrypoint": "run.sh"
            }
            """,
            files: ["run.sh": "echo hi\n"],
            makeEntrypointExecutable: false
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects invalid names")
    func rejectsInvalidNames() {
        #expect(throws: ExtensionLoadError.invalidName("")) {
            try ExtensionManifestLoader.validate(name: "")
        }
        #expect(throws: ExtensionLoadError.invalidName("has space")) {
            try ExtensionManifestLoader.validate(name: "has space")
        }
        #expect(throws: ExtensionLoadError.invalidName("slash/in/name")) {
            try ExtensionManifestLoader.validate(name: "slash/in/name")
        }
    }

    @Test("accepts valid names with allowed characters")
    func acceptsValidNames() throws {
        try ExtensionManifestLoader.validate(name: "my-ext")
        try ExtensionManifestLoader.validate(name: "my_ext.123")
    }

    @Test("MuxyExtension exposes entrypoint URL and display name")
    func muxyExtensionAccessors() {
        let directory = URL(fileURLWithPath: "/tmp/example")
        let manifest = ExtensionManifest(name: "demo", version: "0.1.0", entrypoint: "bin/run")
        let ext = MuxyExtension(id: "demo", directory: directory, manifest: manifest)

        #expect(ext.entrypointURL?.path == "/tmp/example/bin/run")
        #expect(ext.displayName == "demo")
    }

    @Test("ExtensionPaletteCommand derives event name from id")
    func paletteCommandEventName() {
        let command = ExtensionPaletteCommand(id: "do-thing", title: "Do thing", subtitle: nil)
        #expect(command.eventName == "command.do-thing")
    }

    @Test("ExtensionPermission rawValues use namespace:verb form")
    func permissionRawValues() {
        #expect(ExtensionPermission.panesRead.rawValue == "panes:read")
        #expect(ExtensionPermission.panesWrite.rawValue == "panes:write")
        #expect(ExtensionPermission.tabsRead.rawValue == "tabs:read")
        #expect(ExtensionPermission.tabsWrite.rawValue == "tabs:write")
        #expect(ExtensionPermission.projectsRead.rawValue == "projects:read")
        #expect(ExtensionPermission.projectsWrite.rawValue == "projects:write")
        #expect(ExtensionPermission.worktreesRead.rawValue == "worktrees:read")
        #expect(ExtensionPermission.worktreesWrite.rawValue == "worktrees:write")
        #expect(ExtensionPermission.notificationsWrite.rawValue == "notifications:write")
    }

    @Test("ExtensionLoadError surfaces localized messages")
    func loadErrorMessages() {
        let urlError = ExtensionLoadError.manifestMissing(URL(fileURLWithPath: "/tmp/a/manifest.json"))
        #expect(urlError.errorDescription?.contains("/tmp/a/manifest.json") == true)

        let invalid = ExtensionLoadError.manifestInvalid(URL(fileURLWithPath: "/tmp/a/manifest.json"), "bad")
        #expect(invalid.errorDescription?.contains("bad") == true)

        let missing = ExtensionLoadError.entrypointMissing(URL(fileURLWithPath: "/tmp/a/run"))
        #expect(missing.errorDescription?.contains("/tmp/a/run") == true)

        let notExec = ExtensionLoadError.entrypointNotExecutable(URL(fileURLWithPath: "/tmp/a/run"))
        #expect(notExec.errorDescription?.contains("executable") == true)

        let dup = ExtensionLoadError.duplicateName("demo")
        #expect(dup.errorDescription?.contains("demo") == true)

        let invalidName = ExtensionLoadError.invalidName("bad name")
        #expect(invalidName.errorDescription?.contains("bad name") == true)
    }

    @Test("decodes topbar items, statusbar items, and settings")
    func decodesNewSurfaces() throws {
        let json = #"""
        {
            "name": "demo",
            "version": "1.0.0",
            "entrypoint": "run.sh",
            "commands": [
                { "id": "open-pr", "title": "Open PR" }
            ],
            "topbarItems": [
                { "id": "pr", "icon": { "symbol": "arrow.triangle.pull" }, "command": "open-pr" }
            ],
            "statusBarItems": [
                { "id": "build", "icon": "hammer", "side": "right", "command": "open-pr" }
            ],
            "settings": [
                { "key": "endpoint", "title": "Endpoint", "type": "string", "defaultValue": "https://x" }
            ]
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))

        #expect(manifest.topbarItems.count == 1)
        #expect(manifest.topbarItems[0].command == "open-pr")
        if case let .symbol(name) = manifest.topbarItems[0].icon {
            #expect(name == "arrow.triangle.pull")
        } else {
            Issue.record("expected symbol icon")
        }
        #expect(manifest.statusBarItems[0].side == .right)
        if case let .symbol(name) = manifest.statusBarItems[0].icon {
            #expect(name == "hammer")
        } else {
            Issue.record("expected bare string to decode as symbol icon")
        }
        #expect(manifest.settings[0].key == "endpoint")
        #expect(manifest.settings[0].type == .string)
    }

    @Test("rejects topbar item referencing unknown command")
    func rejectsTopbarUnknownCommand() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "topbar-bad",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "topbarItems": [
                    { "id": "x", "icon": "puzzlepiece.extension", "command": "missing" }
                ]
            }
            """,
            files: ["run.sh": "#!/bin/sh\n"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects topbar item with missing SVG")
    func rejectsTopbarMissingSVG() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "topbar-svg",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "commands": [ { "id": "noop", "title": "noop" } ],
                "topbarItems": [
                    { "id": "x", "icon": { "svg": "assets/missing.svg" }, "command": "noop" }
                ]
            }
            """,
            files: ["run.sh": "#!/bin/sh\n"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects duplicate setting keys")
    func rejectsDuplicateSettingKeys() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "settings-dup",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "settings": [
                    { "key": "x", "title": "X", "type": "bool" },
                    { "key": "x", "title": "X again", "type": "bool" }
                ]
            }
            """,
            files: ["run.sh": "#!/bin/sh\n"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("decodes panels with defaults and explicit values")
    func decodesPanels() throws {
        let json = #"""
        {
            "name": "panel-ext",
            "version": "1.0.0",
            "panels": [
                { "id": "minimal", "entry": "panels/a.html" },
                {
                    "id": "full",
                    "title": "Sidebar",
                    "icon": "sidebar.left",
                    "entry": "panels/b.html",
                    "position": "bottom",
                    "mode": "pinned",
                    "hiddenControls": ["pin", "position"]
                }
            ]
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))
        #expect(manifest.panels.count == 2)
        let minimal = try #require(manifest.panel(id: "minimal"))
        #expect(minimal.position == .right)
        #expect(minimal.mode == .floating)
        #expect(minimal.hiddenControls.isEmpty)
        let full = try #require(manifest.panel(id: "full"))
        #expect(full.title == "Sidebar")
        #expect(full.position == .bottom)
        #expect(full.mode == .pinned)
        #expect(full.hiddenControls == [.pin, .position])
    }

    @Test("loads an extension declaring a valid panel")
    func loadsPanel() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-ok",
                "version": "1.0.0",
                "panels": [ { "id": "side", "entry": "panels/side.html" } ]
            }
            """,
            files: ["panels/side.html": "<html></html>"],
            makeEntrypointExecutable: false
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        let ext = try ExtensionManifestLoader.load(from: directory)
        #expect(ext.manifest.panel(id: "side") != nil)
    }

    @Test("rejects a panel whose entry is missing")
    func rejectsPanelMissingEntry() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-missing",
                "version": "1.0.0",
                "panels": [ { "id": "side", "entry": "panels/side.html" } ]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects duplicate panel ids")
    func rejectsDuplicatePanels() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-dup",
                "version": "1.0.0",
                "panels": [
                    { "id": "side", "entry": "panels/side.html" },
                    { "id": "side", "entry": "panels/side.html" }
                ]
            }
            """,
            files: ["panels/side.html": "<html></html>"],
            makeEntrypointExecutable: false
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects command referencing an unknown panel")
    func rejectsCommandUnknownPanel() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-cmd-bad",
                "version": "1.0.0",
                "commands": [
                    { "id": "show", "title": "Show", "action": { "kind": "togglePanel", "panel": "ghost" } }
                ]
            }
            """,
            makeEntrypointExecutable: false
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects empty topbar item id")
    func rejectsEmptyTopbarID() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "topbar-empty",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "commands": [ { "id": "noop", "title": "noop" } ],
                "topbarItems": [
                    { "id": "", "icon": "x.circle", "command": "noop" }
                ]
            }
            """,
            files: ["run.sh": "#!/bin/sh\n"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects empty setting key")
    func rejectsEmptySettingKey() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "settings-empty",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "settings": [
                    { "key": "", "title": "X", "type": "bool" }
                ]
            }
            """,
            files: ["run.sh": "#!/bin/sh\n"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects non-svg icon path")
    func rejectsNonSVGIconPath() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "bad-icon",
                "version": "1.0.0",
                "entrypoint": "run.sh",
                "commands": [ { "id": "noop", "title": "noop" } ],
                "topbarItems": [
                    { "id": "x", "icon": { "svg": "assets/foo.png" }, "command": "noop" }
                ]
            }
            """,
            files: [
                "run.sh": "#!/bin/sh\n",
                "assets/foo.png": "PNG-not-SVG",
            ]
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    private func makeTemporaryExtension(
        manifest: String,
        files: [String: String] = [:],
        makeEntrypointExecutable: Bool = true
    ) throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ext-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)

        let manifestURL = directory.appendingPathComponent("manifest.json")
        try Data(manifest.utf8).write(to: manifestURL)

        for (path, contents) in files {
            let fileURL = directory.appendingPathComponent(path)
            try FileManager.default.createDirectory(at: fileURL.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data(contents.utf8).write(to: fileURL)
            if makeEntrypointExecutable {
                try FileManager.default.setAttributes(
                    [.posixPermissions: FilePermissions.executable],
                    ofItemAtPath: fileURL.path
                )
            }
        }
        return directory
    }
}

@Suite("ExtensionPermission.kind")
struct ExtensionPermissionKindTests {
    @Test("maps read permissions")
    func mapsReadPermissions() {
        let readPermissions: [ExtensionPermission] = [.panesRead, .tabsRead, .projectsRead, .worktreesRead]
        for permission in readPermissions {
            #expect(permission.kind == .read)
        }
    }

    @Test("maps write permissions")
    func mapsWritePermissions() {
        let writePermissions: [ExtensionPermission] = [
            .panesWrite,
            .tabsWrite,
            .projectsWrite,
            .worktreesWrite,
            .notificationsWrite,
        ]
        for permission in writePermissions {
            #expect(permission.kind == .write)
        }
    }

    @Test("maps action permissions")
    func mapsActionPermissions() {
        #expect(ExtensionPermission.commandsRunScript.kind == .action)
        #expect(ExtensionPermission.commandsExec.kind == .action)
    }

    @Test("covers every permission case")
    func coversEveryCase() {
        for permission in ExtensionPermission.allCases {
            _ = permission.kind
        }
    }
}
