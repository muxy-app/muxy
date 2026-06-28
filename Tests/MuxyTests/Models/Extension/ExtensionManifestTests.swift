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
            "background": "background.js"
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))

        #expect(manifest.name == "hello")
        #expect(manifest.version == "1.0.0")
        #expect(manifest.background == "background.js")
        #expect(manifest.events.isEmpty)
        #expect(manifest.commands.isEmpty)
        #expect(manifest.permissions.isEmpty)
    }

    @Test("decodes full manifest with permissions, events and commands")
    func decodesFullManifest() throws {
        let json = #"""
        {
            "name": "demo",
            "version": "2.1",
            "description": "Test extension",
            "background": "background.js",
            "events": ["pane.created", "tab.focused"],
            "commands": [
                { "id": "greet", "title": "Say hello", "subtitle": "demo" }
            ],
            "permissions": ["panes:read", "tabs:write", "notifications:write"]
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))

        #expect(manifest.description == "Test extension")
        #expect(manifest.events == ["pane.created", "tab.focused"])
        #expect(manifest.commands == [ExtensionPaletteCommand(id: "greet", title: "Say hello", subtitle: "demo")])
        #expect(manifest.permissions == [.panesRead, .tabsWrite, .notificationsWrite])
    }

    @Test("decodes file openers with defaults")
    func decodesFileOpeners() throws {
        let json = #"""
        {
            "name": "demo",
            "version": "2.1",
            "tabTypes": [
                { "id": "editor", "title": "Editor", "entry": "editor.html" }
            ],
            "fileOpeners": [
                { "id": "default", "tabType": "editor" },
                {
                    "id": "swift",
                    "title": "Swift Editor",
                    "tabType": "editor",
                    "patterns": ["*.swift"],
                    "singleton": false
                }
            ]
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))

        #expect(manifest.fileOpeners == [
            ExtensionFileOpener(id: "default", tabType: "editor"),
            ExtensionFileOpener(
                id: "swift",
                title: "Swift Editor",
                tabType: "editor",
                patterns: ["*.swift"],
                singleton: false
            ),
        ])
    }

    @Test("file opener patterns support wildcards")
    func fileOpenerPatternsSupportWildcards() {
        let opener = ExtensionFileOpener(id: "swift", tabType: "editor", patterns: ["Sources/*.swift", "Package.*"])

        #expect(opener.matches(relativePath: "Sources/main.swift"))
        #expect(opener.matches(relativePath: "Package.swift"))
        #expect(!opener.matches(relativePath: "README.md"))
    }

    @Test("loads from directory and resolves background script")
    func loadsFromDirectory() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "tmp-ext",
                "version": "1.0.0",
                "background": "background.js",
                "permissions": ["panes:read"]
            }
            """,
            files: ["background.js": "console.log('hi')\n"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        let ext = try ExtensionManifestLoader.load(from: directory)

        #expect(ext.id == "tmp-ext")
        #expect(ext.manifest.permissions == [.panesRead])
        #expect(ext.backgroundScriptURL != nil)
    }

    @Test("loads file openers that reference local tab types")
    func loadsFileOpeners() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "openers",
                "version": "1.0.0",
                "tabTypes": [
                    { "id": "editor", "title": "Editor", "entry": "editor.html" }
                ],
                "fileOpeners": [
                    { "id": "default", "tabType": "editor", "patterns": ["*"] }
                ]
            }
            """,
            files: ["editor.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        let ext = try ExtensionManifestLoader.load(from: directory)

        #expect(ext.manifest.fileOpener(id: "default")?.tabType == "editor")
    }

    @Test("rejects file opener that references an unknown tab type")
    func rejectsFileOpenerUnknownTabType() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "bad-openers",
                "version": "1.0.0",
                "fileOpeners": [
                    { "id": "default", "tabType": "missing" }
                ]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.fileOpenerReferencesUnknownTabType(openerID: "default", tabType: "missing")) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects duplicate file opener ids")
    func rejectsDuplicateFileOpeners() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "duplicate-openers",
                "version": "1.0.0",
                "tabTypes": [
                    { "id": "editor", "title": "Editor", "entry": "editor.html" }
                ],
                "fileOpeners": [
                    { "id": "default", "tabType": "editor" },
                    { "id": "default", "tabType": "editor" }
                ]
            }
            """,
            files: ["editor.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.duplicateFileOpener("default")) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("loads from the dist build output when present")
    func loadsFromBuildOutput() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ext-\(UUID().uuidString)")
        let distDirectory = directory.appendingPathComponent("dist")
        try FileManager.default.createDirectory(at: distDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        try ExtensionManifestFixture.write(
            flatManifest: """
            { "name": "built-ext", "version": "1.0.0" }
            """,
            to: directory
        )
        try ExtensionManifestFixture.write(
            flatManifest: """
            {
                "name": "built-ext",
                "version": "1.0.0",
                "background": "background.js"
            }
            """,
            to: distDirectory
        )
        try Data("console.log('hi')\n".utf8)
            .write(to: distDirectory.appendingPathComponent("background.js"))

        let ext = try ExtensionManifestLoader.load(from: directory)

        #expect(ext.directory.path == distDirectory.path)
        #expect(ext.backgroundScriptURL?.path == distDirectory.appendingPathComponent("background.js").path)
    }

    @Test("resolves resources from dist when present but the manifest stays at the root")
    func resolvesResourcesFromBuildOutputWithoutDistManifest() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ext-\(UUID().uuidString)")
        let distDirectory = directory.appendingPathComponent("dist")
        try FileManager.default.createDirectory(at: distDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        try ExtensionManifestFixture.write(
            flatManifest: """
            {
                "name": "built-ext",
                "version": "1.0.0",
                "background": "background.js"
            }
            """,
            to: directory
        )
        try Data("console.log('hi')\n".utf8)
            .write(to: distDirectory.appendingPathComponent("background.js"))

        let ext = try ExtensionManifestLoader.load(from: directory)

        #expect(ext.directory.path == distDirectory.path)
        #expect(ext.backgroundScriptURL?.path == distDirectory.appendingPathComponent("background.js").path)
    }

    @Test("loads from the directory root when no dist build output exists")
    func loadsFromRootWithoutBuildOutput() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            { "name": "root-ext", "version": "1.0.0" }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        let ext = try ExtensionManifestLoader.load(from: directory)

        #expect(ext.directory == directory)
    }

    @Test("loads without a background script")
    func loadsWithoutBackground() throws {
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

        #expect(ext.manifest.background == nil)
        #expect(ext.backgroundScriptURL == nil)
    }

    @Test("migrates legacy manifest enabled=false into ExtensionEnabledStore")
    func migratesLegacyEnabledFalse() throws {
        let extensionID = "legacy-disabled-\(UUID().uuidString)"
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "\(extensionID)",
                "version": "1.0.0",
                "background": "background.js",
                "enabled": false
            }
            """,
            files: ["background.js": "console.log('hi')\n"]
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
                "background": "background.js",
                "enabled": false
            }
            """,
            files: ["background.js": "console.log('hi')\n"]
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

    @Test("fails when background script missing")
    func failsWhenBackgroundScriptMissing() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "no-bg",
                "version": "1.0.0",
                "background": "background.js"
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects a background script that escapes the extension directory")
    func rejectsBackgroundScriptOutsideDirectory() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "escape-bg",
                "version": "1.0.0",
                "background": "../escape.js"
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.backgroundScriptOutsideDirectory(directory.appendingPathComponent("../escape.js"))) {
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

    @Test("MuxyExtension exposes background script URL and display name")
    func muxyExtensionAccessors() {
        let directory = URL(fileURLWithPath: "/tmp/example")
        let manifest = ExtensionManifest(name: "demo", version: "0.1.0", background: "bin/run")
        let ext = MuxyExtension(id: "demo", directory: directory, manifest: manifest)

        #expect(ext.backgroundScriptURL?.path.hasSuffix("/bin/run") == true)
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
        #expect(ExtensionPermission.browserRead.rawValue == "browser:read")
        #expect(ExtensionPermission.browserWrite.rawValue == "browser:write")
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

        let missing = ExtensionLoadError.backgroundScriptMissing(URL(fileURLWithPath: "/tmp/a/run"))
        #expect(missing.errorDescription?.contains("/tmp/a/run") == true)

        let outside = ExtensionLoadError.backgroundScriptOutsideDirectory(URL(fileURLWithPath: "/tmp/a/run"))
        #expect(outside.errorDescription?.contains("escapes") == true)

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
            "background": "background.js",
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
                "topbarItems": [
                    { "id": "x", "icon": "puzzlepiece.extension", "command": "missing" }
                ]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects panel header button referencing unknown command")
    func rejectsPanelHeaderButtonUnknownCommand() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-hb-bad",
                "version": "1.0.0",
                "panels": [
                    {
                        "id": "side",
                        "entry": "panels/side.html",
                        "headerButtons": [
                            { "id": "prs", "icon": "arrow.triangle.pull", "command": "missing" }
                        ]
                    }
                ]
            }
            """,
            files: ["panels/side.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("loads a panel with a valid header button")
    func loadsPanelHeaderButton() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-hb-ok",
                "version": "1.0.0",
                "commands": [ { "id": "show-prs", "title": "PRs" } ],
                "panels": [
                    {
                        "id": "side",
                        "entry": "panels/side.html",
                        "headerButtons": [
                            { "id": "prs", "icon": "arrow.triangle.pull", "command": "show-prs" }
                        ]
                    }
                ]
            }
            """,
            files: ["panels/side.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        let ext = try ExtensionManifestLoader.load(from: directory)
        let panel = try #require(ext.manifest.panel(id: "side"))
        #expect(panel.headerButtons.map(\.id) == ["prs"])
    }

    @Test("rejects panel header button with empty id")
    func rejectsPanelHeaderButtonEmptyID() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-hb-empty",
                "version": "1.0.0",
                "commands": [ { "id": "show-prs", "title": "PRs" } ],
                "panels": [
                    {
                        "id": "side",
                        "entry": "panels/side.html",
                        "headerButtons": [
                            { "id": "", "icon": "arrow.triangle.pull", "command": "show-prs" }
                        ]
                    }
                ]
            }
            """,
            files: ["panels/side.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects duplicate panel header button ids")
    func rejectsDuplicatePanelHeaderButton() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-hb-dup",
                "version": "1.0.0",
                "commands": [ { "id": "show-prs", "title": "PRs" } ],
                "panels": [
                    {
                        "id": "side",
                        "entry": "panels/side.html",
                        "headerButtons": [
                            { "id": "prs", "icon": "arrow.triangle.pull", "command": "show-prs" },
                            { "id": "prs", "icon": "pencil", "command": "show-prs" }
                        ]
                    }
                ]
            }
            """,
            files: ["panels/side.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects panel header button with missing SVG")
    func rejectsPanelHeaderButtonMissingSVG() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-hb-svg",
                "version": "1.0.0",
                "commands": [ { "id": "show-prs", "title": "PRs" } ],
                "panels": [
                    {
                        "id": "side",
                        "entry": "panels/side.html",
                        "headerButtons": [
                            { "id": "prs", "icon": { "svg": "assets/missing.svg" }, "command": "show-prs" }
                        ]
                    }
                ]
            }
            """,
            files: ["panels/side.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("loads a panel header button with an SVG icon")
    func loadsPanelHeaderButtonSVG() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "panel-hb-svg-ok",
                "version": "1.0.0",
                "commands": [ { "id": "show-prs", "title": "PRs" } ],
                "panels": [
                    {
                        "id": "side",
                        "entry": "panels/side.html",
                        "headerButtons": [
                            { "id": "prs", "icon": { "svg": "assets/prs.svg" }, "command": "show-prs" }
                        ]
                    }
                ]
            }
            """,
            files: [
                "panels/side.html": "<html></html>",
                "assets/prs.svg": "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
            ]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        let ext = try ExtensionManifestLoader.load(from: directory)
        let panel = try #require(ext.manifest.panel(id: "side"))
        #expect(panel.headerButtons.first?.icon == .svg("assets/prs.svg"))
    }

    @Test("rejects topbar item with missing SVG")
    func rejectsTopbarMissingSVG() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "topbar-svg",
                "version": "1.0.0",
                "commands": [ { "id": "noop", "title": "noop" } ],
                "topbarItems": [
                    { "id": "x", "icon": { "svg": "assets/missing.svg" }, "command": "noop" }
                ]
            }
            """
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
                "settings": [
                    { "key": "x", "title": "X", "type": "bool" },
                    { "key": "x", "title": "X again", "type": "bool" }
                ]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("defaults remoteMethods to empty when absent")
    func defaultsRemoteMethodsEmpty() throws {
        let json = #"""
        {
            "name": "hello",
            "version": "1.0.0"
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))
        #expect(manifest.remoteMethods.isEmpty)
    }

    @Test("decodes remoteMethods and remote:serve permission")
    func decodesRemoteMethods() throws {
        let json = #"""
        {
            "name": "weather",
            "version": "1.0.0",
            "background": "background.js",
            "permissions": ["remote:serve"],
            "remoteMethods": [
                { "id": "forecast", "description": "Get forecast" },
                { "id": "current" }
            ]
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))
        #expect(manifest.permissions == [.remoteServe])
        #expect(manifest.remoteMethods.map(\.id) == ["forecast", "current"])
        #expect(manifest.remoteMethod(id: "forecast")?.description == "Get forecast")
        #expect(manifest.remoteMethod(id: "missing") == nil)
    }

    @Test("rejects duplicate remote method ids")
    func rejectsDuplicateRemoteMethods() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "remote-dup",
                "version": "1.0.0",
                "remoteMethods": [
                    { "id": "ping" },
                    { "id": "ping" }
                ]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects empty remote method id")
    func rejectsEmptyRemoteMethodID() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "remote-empty",
                "version": "1.0.0",
                "remoteMethods": [
                    { "id": "" }
                ]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects remote method id containing the wire delimiter")
    func rejectsDelimiterRemoteMethodID() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "remote-pipe",
                "version": "1.0.0",
                "remoteMethods": [
                    { "id": "a|b" }
                ]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.remoteMethodInvalidID("a|b")) {
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
                    "hiddenControls": ["pin", "position"],
                    "headerButtons": [
                        { "id": "prs", "icon": { "symbol": "arrow.triangle.pull" }, "tooltip": "PRs", "command": "show-prs" }
                    ],
                    "hideTopbar": true
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
        #expect(minimal.headerButtons.isEmpty)
        #expect(!minimal.hideTopbar)
        let full = try #require(manifest.panel(id: "full"))
        #expect(full.title == "Sidebar")
        #expect(full.position == .bottom)
        #expect(full.mode == .pinned)
        #expect(full.hiddenControls == [.pin, .position])
        #expect(full.headerButtons == [
            ExtensionPanelHeaderButton(id: "prs", icon: .symbol("arrow.triangle.pull"), tooltip: "PRs", command: "show-prs")
        ])
        #expect(full.hideTopbar)
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
            files: ["panels/side.html": "<html></html>"]
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
            files: ["panels/side.html": "<html></html>"]
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
            """
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
                "commands": [ { "id": "noop", "title": "noop" } ],
                "topbarItems": [
                    { "id": "", "icon": "x.circle", "command": "noop" }
                ]
            }
            """
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
                "settings": [
                    { "key": "", "title": "X", "type": "bool" }
                ]
            }
            """
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
                "commands": [ { "id": "noop", "title": "noop" } ],
                "topbarItems": [
                    { "id": "x", "icon": { "svg": "assets/foo.png" }, "command": "noop" }
                ]
            }
            """,
            files: [
                "assets/foo.png": "PNG-not-SVG",
            ]
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("decodes popovers with defaults and explicit values")
    func decodesPopovers() throws {
        let json = #"""
        {
            "name": "popover-ext",
            "version": "1.0.0",
            "popovers": [
                { "id": "minimal", "entry": "popovers/a.html" },
                {
                    "id": "full",
                    "title": "Usage",
                    "entry": "popovers/b.html",
                    "width": 280,
                    "height": 200
                }
            ]
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))
        #expect(manifest.popovers.count == 2)
        let minimal = try #require(manifest.popover(id: "minimal"))
        #expect(minimal.width == ExtensionPopover.defaultWidth)
        #expect(minimal.height == ExtensionPopover.defaultHeight)
        #expect(minimal.title == nil)
        let full = try #require(manifest.popover(id: "full"))
        #expect(full.title == "Usage")
        #expect(full.width == 280)
        #expect(full.height == 200)
    }

    @Test("loads an extension declaring a valid popover")
    func loadsPopover() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "popover-ok",
                "version": "1.0.0",
                "popovers": [ { "id": "info", "entry": "popovers/info.html" } ]
            }
            """,
            files: ["popovers/info.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        let ext = try ExtensionManifestLoader.load(from: directory)
        #expect(ext.manifest.popover(id: "info") != nil)
    }

    @Test("rejects a popover whose entry is missing")
    func rejectsPopoverMissingEntry() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "popover-missing",
                "version": "1.0.0",
                "popovers": [ { "id": "info", "entry": "popovers/info.html" } ]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects duplicate popover ids")
    func rejectsDuplicatePopovers() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "popover-dup",
                "version": "1.0.0",
                "popovers": [
                    { "id": "info", "entry": "popovers/info.html" },
                    { "id": "info", "entry": "popovers/info.html" }
                ]
            }
            """,
            files: ["popovers/info.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects command referencing an unknown popover")
    func rejectsCommandUnknownPopover() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "popover-cmd-bad",
                "version": "1.0.0",
                "commands": [
                    { "id": "show", "title": "Show", "action": { "kind": "openPopover", "popover": "ghost" } }
                ]
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("openPopover command action round-trips through Codable")
    func openPopoverActionRoundTrips() throws {
        let action = ExtensionCommandAction.openPopover(popover: "usage")
        let encoded = try JSONEncoder().encode(action)
        let decoded = try JSONDecoder().decode(ExtensionCommandAction.self, from: encoded)
        #expect(decoded == action)
    }

    @Test("only openPopover actions are anchored to a UI item")
    func anchoredActionsAreOpenPopover() {
        #expect(ExtensionCommandAction.openPopover(popover: "usage").isAnchored)
        #expect(!ExtensionCommandAction.event.isAnchored)
        #expect(!ExtensionCommandAction.togglePanel(panel: "dashboard").isAnchored)
        #expect(!ExtensionCommandAction.openTab(tabType: "logs", data: nil).isAnchored)
        #expect(!ExtensionCommandAction.runScript(script: "s.js").isAnchored)
    }

    @Test("command actions declare required manifest permissions")
    func actionRequiredPermissions() {
        #expect(ExtensionCommandAction.event.requiredPermission == nil)
        #expect(ExtensionCommandAction.openTab(tabType: "logs", data: nil).requiredPermission == .tabsWrite)
        #expect(ExtensionCommandAction.togglePanel(panel: "dashboard").requiredPermission == .panelsWrite)
        #expect(ExtensionCommandAction.openPopover(popover: "usage").requiredPermission == .panelsWrite)
        #expect(ExtensionCommandAction.runScript(script: "s.js").requiredPermission == .commandsRunScript)
    }

    @Test("decodes a sidebar declaration")
    func decodesSidebar() throws {
        let json = #"""
        {
            "name": "sb-ext",
            "version": "1.0.0",
            "sidebar": {
                "id": "main",
                "title": "My Sidebar",
                "icon": "sparkles",
                "entry": "sidebar/index.html"
            }
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))

        #expect(manifest.sidebar?.id == "main")
        #expect(manifest.sidebar?.title == "My Sidebar")
        #expect(manifest.sidebar?.icon == .symbol("sparkles"))
        #expect(manifest.sidebar?.entry == "sidebar/index.html")
    }

    @Test("loads a sidebar and resolves its entry")
    func loadsSidebar() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "sb-load",
                "version": "1.0.0",
                "sidebar": { "id": "main", "entry": "sidebar/index.html" }
            }
            """,
            files: ["sidebar/index.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        let ext = try ExtensionManifestLoader.load(from: directory)

        #expect(ext.manifest.sidebar?.id == "main")
    }

    @Test("rejects a sidebar with a missing entry")
    func rejectsSidebarMissingEntry() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "sb-missing",
                "version": "1.0.0",
                "sidebar": { "id": "main", "entry": "sidebar/index.html" }
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects a sidebar entry that escapes the extension directory")
    func rejectsSidebarEntryOutsideDirectory() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "sb-escape",
                "version": "1.0.0",
                "sidebar": { "id": "main", "entry": "../escape.html" }
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.sidebarEntryOutsideDirectory(
            sidebarID: "main",
            url: directory.appendingPathComponent("../escape.html")
        )) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects a sidebar with an empty id")
    func rejectsSidebarEmptyID() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "sb-empty-id",
                "version": "1.0.0",
                "sidebar": { "id": "", "entry": "sidebar/index.html" }
            }
            """,
            files: ["sidebar/index.html": "<html></html>"]
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.sidebarEmptyID) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    @Test("rejects a sidebar with an empty entry")
    func rejectsSidebarEmptyEntry() throws {
        let directory = try makeTemporaryExtension(
            manifest: """
            {
                "name": "sb-empty-entry",
                "version": "1.0.0",
                "sidebar": { "id": "main", "entry": "" }
            }
            """
        )
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: ExtensionLoadError.sidebarEntryEmpty(sidebarID: "main")) {
            try ExtensionManifestLoader.load(from: directory)
        }
    }

    private func makeTemporaryExtension(
        manifest: String,
        files: [String: String] = [:]
    ) throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ext-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)

        try ExtensionManifestFixture.write(flatManifest: manifest, to: directory)

        for (path, contents) in files {
            let fileURL = directory.appendingPathComponent(path)
            try FileManager.default.createDirectory(at: fileURL.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data(contents.utf8).write(to: fileURL)
        }
        return directory
    }
}

@Suite("ExtensionPermission.kind")
struct ExtensionPermissionKindTests {
    @Test("maps read permissions")
    func mapsReadPermissions() {
        let readPermissions: [ExtensionPermission] = [
            .panesRead,
            .tabsRead,
            .browserRead,
            .projectsRead,
            .worktreesRead,
        ]
        for permission in readPermissions {
            #expect(permission.kind == .read)
        }
    }

    @Test("maps write permissions")
    func mapsWritePermissions() {
        let writePermissions: [ExtensionPermission] = [
            .panesWrite,
            .tabsWrite,
            .browserWrite,
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
        #expect(ExtensionPermission.remoteServe.kind == .action)
    }

    @Test("covers every permission case")
    func coversEveryCase() {
        for permission in ExtensionPermission.allCases {
            _ = permission.kind
        }
    }

    @Test("remote:serve displays as remote-api and others use the raw value")
    func permissionDisplayName() {
        #expect(ExtensionPermission.remoteServe.displayName == "remote-api")
        #expect(ExtensionPermission.commandsExec.displayName == "commands:exec")
    }
}
