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
        #expect(manifest.enabled == true)
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
            "aiProvider": { "socketTypeKey": "demo", "displayName": "Demo", "iconName": "sparkles" },
            "enabled": false
        }
        """#
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: Data(json.utf8))

        #expect(manifest.description == "Test extension")
        #expect(manifest.events == ["pane.created", "tab.focused"])
        #expect(manifest.commands == [ExtensionPaletteCommand(id: "greet", title: "Say hello", subtitle: "demo")])
        #expect(manifest.permissions == [.panesRead, .tabsWrite])
        #expect(manifest.aiProvider == ExtensionAIProvider(socketTypeKey: "demo", displayName: "Demo", iconName: "sparkles"))
        #expect(manifest.enabled == false)
    }

    @Test("loads from directory and resolves entrypoint")
    func loadsFromDirectory() throws {
        let directory = try makeTemporaryExtension(
            name: "tmp-ext",
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
        #expect(FileManager.default.isExecutableFile(atPath: ext.entrypointURL.path))
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
            name: "no-exec",
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

        #expect(ext.entrypointURL.path == "/tmp/example/bin/run")
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

    private func makeTemporaryExtension(
        name: String,
        manifest: String,
        files: [String: String],
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
        _ = name
        return directory
    }
}
