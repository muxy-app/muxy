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
    case kitResourceMissing
    case invalidKitManifest
    case fileSystem(String)

    var errorDescription: String? {
        switch self {
        case let .invalidVersion(version):
            "Extension version '\(version)' is empty"
        case let .directoryAlreadyExists(url):
            "An extension already exists at \(url.path)"
        case .skillResourceMissing:
            "Could not locate the bundled muxy-extension skill resource"
        case .kitResourceMissing:
            "Could not locate the bundled starter kit"
        case .invalidKitManifest:
            "The starter kit package.json could not be read"
        case let .fileSystem(message):
            message
        }
    }
}

enum ExtensionScaffoldService {
    private static let kitName = "vanilla"
    private static let excludedKitEntries: Set<String> = ["node_modules", "dist", "package-lock.json"]

    static func create(
        _ request: ExtensionScaffoldRequest,
        in rootDirectory: URL,
        skillSourceURL: URL? = bundledSkillSourceURL(),
        kitSourceURL: URL? = nil
    ) throws -> URL {
        let name = request.trimmedName
        let version = request.trimmedVersion
        let description = request.trimmedDescription

        try ExtensionManifestLoader.validate(name: name)
        guard !version.isEmpty else { throw ExtensionScaffoldError.invalidVersion(version) }

        let kitSource = kitSourceURL ?? bundledKitSourceURL()
        guard let kitSource, FileManager.default.fileExists(atPath: kitSource.path) else {
            throw ExtensionScaffoldError.kitResourceMissing
        }
        guard let skillSourceURL, FileManager.default.fileExists(atPath: skillSourceURL.path) else {
            throw ExtensionScaffoldError.skillResourceMissing
        }

        try FileManager.default.createDirectory(
            at: rootDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )

        let extensionDirectory = rootDirectory.appendingPathComponent(name, isDirectory: true)
        guard !FileManager.default.fileExists(atPath: extensionDirectory.path) else {
            throw ExtensionScaffoldError.directoryAlreadyExists(extensionDirectory)
        }

        do {
            try copyKit(from: kitSource, into: extensionDirectory)
            try rewritePackageManifest(name: name, version: version, description: description, in: extensionDirectory)
            try writeClaudeMarkdown(name: name, description: description, in: extensionDirectory)
            try writeAgentsSymlink(in: extensionDirectory)
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

    static func bundledKitSourceURL() -> URL? {
        if let url = Bundle.appResources.url(forResource: kitName, withExtension: nil, subdirectory: "starter-kits") {
            return url
        }
        return Bundle.appResources.resourceURL?
            .appendingPathComponent("starter-kits/\(kitName)", isDirectory: true)
    }

    private static func copyKit(from source: URL, into destination: URL) throws {
        try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: false)
        let entries = try FileManager.default.contentsOfDirectory(atPath: source.path)
        for entry in entries where !excludedKitEntries.contains(entry) {
            try FileManager.default.copyItem(
                at: source.appendingPathComponent(entry),
                to: destination.appendingPathComponent(entry)
            )
        }
    }

    private static func rewritePackageManifest(
        name: String,
        version: String,
        description: String,
        in directory: URL
    ) throws {
        let packageURL = directory.appendingPathComponent("package.json")
        let data = try Data(contentsOf: packageURL)
        guard var package = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw ExtensionScaffoldError.invalidKitManifest
        }

        package["name"] = name
        package["version"] = version

        var muxy = package["muxy"] as? [String: Any] ?? [:]
        if description.isEmpty {
            muxy["description"] = nil
        } else {
            muxy["description"] = description
        }
        package["muxy"] = muxy

        let updated = try JSONSerialization.data(
            withJSONObject: package,
            options: [.prettyPrinted, .sortedKeys]
        )
        try updated.write(to: packageURL)
    }

    private static func writeClaudeMarkdown(
        name: String,
        description: String,
        in directory: URL
    ) throws {
        let header = description.isEmpty ? "" : "\n\n\(description)"
        let contents = """
        # \(name)\(header)

        Muxy extension scaffolded from a starter kit. This is an npm + Vite project.

        ## Layout

        - `package.json` — npm manifest. Identity (`name`, `version`) is at the
          top level; all Muxy fields live under the `muxy` key. A `build` script
          (Vite) is required.
        - `vite.config.js` — builds to `dist/`, the directory Muxy installs.
        - `panel/` + `src/` — your source. The kit ships a working panel, a topbar
          item, and a command; edit them or add your own.

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
