import Foundation

struct ExtensionScaffoldRequest: Equatable {
    let name: String
    let version: String
    let description: String

    var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
    var trimmedVersion: String { version.trimmingCharacters(in: .whitespacesAndNewlines) }
    var trimmedDescription: String { description.trimmingCharacters(in: .whitespacesAndNewlines) }
}

enum ExtensionScaffoldError: LocalizedError, Equatable {
    case invalidVersion(String)
    case directoryAlreadyExists(URL)
    case skillResourceMissing
    case fileSystem(String)

    var errorDescription: String? {
        switch self {
        case let .invalidVersion(version):
            "Extension version '\(version)' is empty"
        case let .directoryAlreadyExists(url):
            "An extension already exists at \(url.path)"
        case .skillResourceMissing:
            "Could not locate the bundled muxy-extension skill resource"
        case let .fileSystem(message):
            message
        }
    }
}

enum ExtensionScaffoldService {
    static func create(
        _ request: ExtensionScaffoldRequest,
        in rootDirectory: URL,
        skillSourceURL: URL? = bundledSkillSourceURL()
    ) throws -> URL {
        let name = request.trimmedName
        let version = request.trimmedVersion
        let description = request.trimmedDescription

        try ExtensionManifestLoader.validate(name: name)
        guard !version.isEmpty else { throw ExtensionScaffoldError.invalidVersion(version) }

        try FileManager.default.createDirectory(
            at: rootDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )

        let extensionDirectory = rootDirectory.appendingPathComponent(name, isDirectory: true)
        guard !FileManager.default.fileExists(atPath: extensionDirectory.path) else {
            throw ExtensionScaffoldError.directoryAlreadyExists(extensionDirectory)
        }

        guard let skillSourceURL else { throw ExtensionScaffoldError.skillResourceMissing }
        guard FileManager.default.fileExists(atPath: skillSourceURL.path) else {
            throw ExtensionScaffoldError.skillResourceMissing
        }

        do {
            try FileManager.default.createDirectory(at: extensionDirectory, withIntermediateDirectories: false)
            try writePackageJSON(name: name, version: version, description: description, in: extensionDirectory)
            try writeViteConfig(in: extensionDirectory)
            try writeIndexHTML(name: name, in: extensionDirectory)
            try writeClaudeMarkdown(name: name, description: description, in: extensionDirectory)
            try writeAgentsSymlink(in: extensionDirectory)
            try writeGitignore(in: extensionDirectory)
            try copySkill(from: skillSourceURL, into: extensionDirectory)
        } catch let error as ExtensionScaffoldError {
            try? FileManager.default.removeItem(at: extensionDirectory)
            throw error
        } catch {
            try? FileManager.default.removeItem(at: extensionDirectory)
            throw ExtensionScaffoldError.fileSystem(error.localizedDescription)
        }

        return extensionDirectory
    }

    static func bundledSkillSourceURL() -> URL? {
        if let url = Bundle.appResources.url(forResource: "SKILL", withExtension: "md", subdirectory: "skills/muxy-extension") {
            return url
        }
        return Bundle.appResources.resourceURL?
            .appendingPathComponent("skills/muxy-extension/SKILL.md")
    }

    private static func writePackageJSON(
        name: String,
        version: String,
        description: String,
        in directory: URL
    ) throws {
        var muxy: [String: Any] = [
            "events": [],
            "commands": [],
            "permissions": [],
        ]
        if !description.isEmpty {
            muxy["description"] = description
        }
        let packageJSON: [String: Any] = [
            "name": name,
            "version": version,
            "private": true,
            "type": "module",
            "scripts": [
                "dev": "vite",
                "build": "vite build",
            ],
            "devDependencies": [
                "vite": "^5.0.0",
            ],
            "muxy": muxy,
        ]
        let data = try JSONSerialization.data(
            withJSONObject: packageJSON,
            options: [.prettyPrinted, .sortedKeys]
        )
        try data.write(to: directory.appendingPathComponent("package.json"))
    }

    private static func writeViteConfig(in directory: URL) throws {
        let contents = """
        import { defineConfig } from "vite";

        export default defineConfig({
          build: {
            outDir: "dist",
            emptyOutDir: true,
            minify: false,
          },
        });
        """
        try Data(contents.utf8).write(to: directory.appendingPathComponent("vite.config.js"))
    }

    private static func writeIndexHTML(name: String, in directory: URL) throws {
        let contents = """
        <!doctype html>
        <html lang="en">
          <head>
            <meta charset="utf-8" />
            <title>\(name)</title>
          </head>
          <body>
            <div id="app"></div>
            <script type="module" src="/src/main.js"></script>
          </body>
        </html>
        """
        try Data(contents.utf8).write(to: directory.appendingPathComponent("index.html"))

        let srcDirectory = directory.appendingPathComponent("src", isDirectory: true)
        try FileManager.default.createDirectory(at: srcDirectory, withIntermediateDirectories: true)
        let main = """
        // Entry point. Use any npm packages or framework you like (React, Vue,
        // Svelte, …); Vite bundles them into dist/. Talk to the host via the
        // injected `window.muxy` API.
        document.querySelector("#app").textContent = "Hello from \(name)";
        """
        try Data(main.utf8).write(to: srcDirectory.appendingPathComponent("main.js"))
    }

    private static func writeClaudeMarkdown(
        name: String,
        description: String,
        in directory: URL
    ) throws {
        let header = description.isEmpty ? "" : "\n\n\(description)"
        let contents = """
        # \(name)\(header)

        Muxy extension scaffolded by Muxy. This is an npm + Vite project.

        ## Layout

        - `package.json` — npm manifest. Identity (`name`, `version`) is at the
          top level; all Muxy fields live under the `muxy` key. A `build` script
          (Vite) is required.
        - `vite.config.js` — builds to `dist/`, the directory Muxy installs.
        - `index.html` + `src/` — your source. Use any npm packages or framework
          (React, Vue, Svelte, …); Vite bundles them into `dist/`.

        Add a `"background"` script (e.g. `background.js`) under the `muxy` key
        only if the extension needs to receive pushed workspace events or run
        shell commands in the background. Muxy runs it as a long-lived process
        that subscribes to events with `muxy.events.subscribe` and runs commands
        with `muxy.exec`. Command, topbar, status bar, tab, and runScript
        extensions need no background script.

        ## Building & editing

        Install deps with `npm install`, then `npm run build` to produce
        `dist/`. After rebuilding, click "Reload" in the Muxy Extensions modal to
        pick up the changes. (`npm run dev` runs Vite's dev server for fast
        iteration.)

        ## Skill

        Coding agents in this directory should consult the `muxy-extension`
        skill in `.claude/skills/` or `.agents/skills/` before generating
        manifest or runtime changes.
        """
        try Data(contents.utf8).write(to: directory.appendingPathComponent("CLAUDE.md"))
    }

    private static func writeAgentsSymlink(in directory: URL) throws {
        let symlinkURL = directory.appendingPathComponent("AGENTS.md")
        try FileManager.default.createSymbolicLink(
            atPath: symlinkURL.path,
            withDestinationPath: "CLAUDE.md"
        )
    }

    private static func writeGitignore(in directory: URL) throws {
        let contents = """
        .DS_Store
        node_modules/
        dist/
        build/
        *.log
        """
        try Data(contents.utf8).write(to: directory.appendingPathComponent(".gitignore"))
    }

    private static func copySkill(from source: URL, into directory: URL) throws {
        for parent in [".claude", ".agents"] {
            let skillDirectory = directory
                .appendingPathComponent(parent, isDirectory: true)
                .appendingPathComponent("skills", isDirectory: true)
                .appendingPathComponent("muxy-extension", isDirectory: true)
            try FileManager.default.createDirectory(at: skillDirectory, withIntermediateDirectories: true)
            try FileManager.default.copyItem(
                at: source,
                to: skillDirectory.appendingPathComponent("SKILL.md")
            )
        }
    }
}
