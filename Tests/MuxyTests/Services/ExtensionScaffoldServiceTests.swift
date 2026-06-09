import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionScaffoldService")
struct ExtensionScaffoldServiceTests {
    @Test("scaffolds a complete extension directory")
    func scaffoldsCompleteDirectory() throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        let extensionURL = try ExtensionScaffoldService.create(
            ExtensionScaffoldRequest(name: "demo", version: "0.1.0", description: "A demo extension"),
            in: fixture.rootURL,
            skillSourceURL: fixture.skillSourceURL,
            kitSourceURL: fixture.kitSourceURL
        )

        #expect(extensionURL.lastPathComponent == "demo")
        try assertManifest(at: extensionURL, name: "demo", version: "0.1.0", description: "A demo extension")
        try assertNoBackground(at: extensionURL)
        try assertClaudeMarkdown(at: extensionURL, includes: "# demo")
        try assertAgentsSymlinkPointsToClaude(at: extensionURL)
        try assertGitignore(at: extensionURL)
        try assertSkillCopied(at: extensionURL)
    }

    @Test("copies kit source and excludes node_modules, dist, and the lockfile")
    func copiesKitSourceAndExcludesArtifacts() throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        let extensionURL = try ExtensionScaffoldService.create(
            ExtensionScaffoldRequest(name: "kitted", version: "0.1.0", description: ""),
            in: fixture.rootURL,
            skillSourceURL: fixture.skillSourceURL,
            kitSourceURL: fixture.kitSourceURL
        )

        let manager = FileManager.default
        #expect(manager.fileExists(atPath: extensionURL.appendingPathComponent("src/main.ts").path))
        #expect(!manager.fileExists(atPath: extensionURL.appendingPathComponent("node_modules").path))
        #expect(!manager.fileExists(atPath: extensionURL.appendingPathComponent("package-lock.json").path))
    }

    @Test("omits description from manifest when blank")
    func omitsDescriptionWhenBlank() throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        let extensionURL = try ExtensionScaffoldService.create(
            ExtensionScaffoldRequest(name: "tidy", version: "1.0.0", description: "  "),
            in: fixture.rootURL,
            skillSourceURL: fixture.skillSourceURL,
            kitSourceURL: fixture.kitSourceURL
        )

        let manifest = try loadManifest(at: extensionURL)
        #expect(manifest["description"] == nil)
    }

    @Test("rejects invalid names")
    func rejectsInvalidNames() throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        #expect(throws: ExtensionLoadError.self) {
            try ExtensionScaffoldService.create(
                ExtensionScaffoldRequest(name: "bad name!", version: "0.1.0", description: ""),
                in: fixture.rootURL,
                skillSourceURL: fixture.skillSourceURL,
            kitSourceURL: fixture.kitSourceURL
            )
        }
    }

    @Test("rejects names that escape the extensions directory")
    func rejectsPathTraversalNames() throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        for name in ["..", ".", ".hidden"] {
            #expect(throws: ExtensionLoadError.self) {
                try ExtensionScaffoldService.create(
                    ExtensionScaffoldRequest(name: name, version: "0.1.0", description: ""),
                    in: fixture.rootURL,
                    skillSourceURL: fixture.skillSourceURL,
            kitSourceURL: fixture.kitSourceURL
                )
            }
        }
    }

    @Test("rejects empty version")
    func rejectsEmptyVersion() throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        #expect(throws: ExtensionScaffoldError.self) {
            try ExtensionScaffoldService.create(
                ExtensionScaffoldRequest(name: "no-version", version: "  ", description: ""),
                in: fixture.rootURL,
                skillSourceURL: fixture.skillSourceURL,
            kitSourceURL: fixture.kitSourceURL
            )
        }
    }

    @Test("refuses to overwrite an existing extension directory")
    func refusesExistingDirectory() throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        _ = try ExtensionScaffoldService.create(
            ExtensionScaffoldRequest(name: "dup", version: "0.1.0", description: ""),
            in: fixture.rootURL,
            skillSourceURL: fixture.skillSourceURL,
            kitSourceURL: fixture.kitSourceURL
        )

        #expect(throws: ExtensionScaffoldError.self) {
            try ExtensionScaffoldService.create(
                ExtensionScaffoldRequest(name: "dup", version: "0.1.0", description: ""),
                in: fixture.rootURL,
                skillSourceURL: fixture.skillSourceURL,
            kitSourceURL: fixture.kitSourceURL
            )
        }
    }

    @Test("loads the scaffolded extension via ExtensionManifestLoader")
    func scaffoldedExtensionLoadsCleanly() throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        let extensionURL = try ExtensionScaffoldService.create(
            ExtensionScaffoldRequest(name: "loadable", version: "0.2.0", description: "Round-trip"),
            in: fixture.rootURL,
            skillSourceURL: fixture.skillSourceURL,
            kitSourceURL: fixture.kitSourceURL
        )

        let loaded = try ExtensionManifestLoader.load(from: extensionURL)
        #expect(loaded.id == "loadable")
        #expect(loaded.manifest.version == "0.2.0")
        #expect(loaded.manifest.description == "Round-trip")
        #expect(loaded.manifest.background == nil)
    }

    private struct Fixture {
        let rootURL: URL
        let skillSourceURL: URL
        let kitSourceURL: URL

        init() throws {
            let base = FileManager.default.temporaryDirectory
                .appendingPathComponent("muxy-scaffold-\(UUID().uuidString)")
            try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
            rootURL = base.appendingPathComponent("extensions")
            try FileManager.default.createDirectory(at: rootURL, withIntermediateDirectories: true)

            skillSourceURL = base.appendingPathComponent("SKILL.md")
            try Data("# Test Skill\n".utf8).write(to: skillSourceURL)

            kitSourceURL = base.appendingPathComponent("kit", isDirectory: true)
            try Fixture.writeKit(at: kitSourceURL)
        }

        func cleanup() {
            try? FileManager.default.removeItem(at: rootURL.deletingLastPathComponent())
        }

        private static func writeKit(at kitURL: URL) throws {
            let manager = FileManager.default
            try manager.createDirectory(at: kitURL.appendingPathComponent("src"), withIntermediateDirectories: true)
            let packageJSON = """
            {
              "name": "muxy-starter-kit",
              "version": "9.9.9",
              "private": true,
              "type": "module",
              "scripts": { "dev": "vite", "build": "vite build" },
              "devDependencies": { "vite": "^7.0.0" },
              "muxy": {
                "description": "Kit description",
                "permissions": ["panels:write"]
              }
            }
            """
            try Data(packageJSON.utf8).write(to: kitURL.appendingPathComponent("package.json"))
            try Data("export {};\n".utf8).write(to: kitURL.appendingPathComponent("src/main.ts"))
            try Data(".DS_Store\nnode_modules/\ndist/\n".utf8).write(to: kitURL.appendingPathComponent(".gitignore"))

            try manager.createDirectory(at: kitURL.appendingPathComponent("node_modules"), withIntermediateDirectories: true)
            try Data("ignored\n".utf8).write(to: kitURL.appendingPathComponent("node_modules/marker"))
            try Data("{}\n".utf8).write(to: kitURL.appendingPathComponent("package-lock.json"))
        }
    }

    private func loadPackage(at extensionURL: URL) throws -> [String: Any] {
        let data = try Data(contentsOf: extensionURL.appendingPathComponent("package.json"))
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            Issue.record("package.json was not a JSON object")
            return [:]
        }
        return object
    }

    private func loadManifest(at extensionURL: URL) throws -> [String: Any] {
        let package = try loadPackage(at: extensionURL)
        return package["muxy"] as? [String: Any] ?? [:]
    }

    private func assertManifest(
        at extensionURL: URL,
        name: String,
        version: String,
        description: String
    ) throws {
        let package = try loadPackage(at: extensionURL)
        #expect(package["name"] as? String == name)
        #expect(package["version"] as? String == version)
        #expect((package["scripts"] as? [String: Any])?["build"] as? String != nil)

        let muxy = try loadManifest(at: extensionURL)
        #expect(muxy["background"] == nil)
        #expect(muxy["description"] as? String == description)
    }

    private func assertNoBackground(at extensionURL: URL) throws {
        let manifest = try loadManifest(at: extensionURL)
        #expect(manifest["background"] == nil)
    }

    private func assertClaudeMarkdown(at extensionURL: URL, includes substring: String) throws {
        let url = extensionURL.appendingPathComponent("CLAUDE.md")
        let contents = try String(contentsOf: url, encoding: .utf8)
        #expect(contents.contains(substring))
    }

    private func assertAgentsSymlinkPointsToClaude(at extensionURL: URL) throws {
        let agentsURL = extensionURL.appendingPathComponent("AGENTS.md")
        let destination = try FileManager.default.destinationOfSymbolicLink(atPath: agentsURL.path)
        #expect(destination == "CLAUDE.md")
        #expect(FileManager.default.fileExists(atPath: agentsURL.path))
    }

    private func assertGitignore(at extensionURL: URL) throws {
        let url = extensionURL.appendingPathComponent(".gitignore")
        let contents = try String(contentsOf: url, encoding: .utf8)
        #expect(contents.contains(".DS_Store"))
        #expect(contents.contains("node_modules/"))
    }

    private func assertSkillCopied(at extensionURL: URL) throws {
        let claudeSkill = extensionURL.appendingPathComponent(".claude/skills/muxy-extension/SKILL.md")
        let agentsSkill = extensionURL.appendingPathComponent(".agents/skills/muxy-extension/SKILL.md")
        #expect(FileManager.default.fileExists(atPath: claudeSkill.path))
        #expect(FileManager.default.fileExists(atPath: agentsSkill.path))
        let claudeContents = try String(contentsOf: claudeSkill, encoding: .utf8)
        #expect(claudeContents.contains("Test Skill"))
    }
}
