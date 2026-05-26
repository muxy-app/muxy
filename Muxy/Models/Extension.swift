import Foundation

enum ExtensionPermission: String, Codable, CaseIterable {
    case panesRead = "panes:read"
    case panesWrite = "panes:write"
    case tabsRead = "tabs:read"
    case tabsWrite = "tabs:write"
    case projectsRead = "projects:read"
    case projectsWrite = "projects:write"
    case worktreesRead = "worktrees:read"
    case worktreesWrite = "worktrees:write"
    case notificationsWrite = "notifications:write"
}

struct ExtensionPaletteCommand: Codable, Equatable, Identifiable {
    let id: String
    let title: String
    let subtitle: String?

    var eventName: String { "command.\(id)" }
}

struct ExtensionAIProvider: Codable, Equatable {
    let socketTypeKey: String
    let displayName: String
    let iconName: String
}

struct ExtensionManifest: Codable, Equatable {
    let name: String
    let version: String
    let description: String?
    let entrypoint: String
    let events: [String]
    let commands: [ExtensionPaletteCommand]
    let permissions: [ExtensionPermission]
    let aiProvider: ExtensionAIProvider?
    let enabled: Bool

    private enum CodingKeys: String, CodingKey {
        case name
        case version
        case description
        case entrypoint
        case events
        case commands
        case permissions
        case aiProvider
        case enabled
    }

    init(
        name: String,
        version: String,
        description: String? = nil,
        entrypoint: String,
        events: [String] = [],
        commands: [ExtensionPaletteCommand] = [],
        permissions: [ExtensionPermission] = [],
        aiProvider: ExtensionAIProvider? = nil,
        enabled: Bool = true
    ) {
        self.name = name
        self.version = version
        self.description = description
        self.entrypoint = entrypoint
        self.events = events
        self.commands = commands
        self.permissions = permissions
        self.aiProvider = aiProvider
        self.enabled = enabled
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        name = try container.decode(String.self, forKey: .name)
        version = try container.decode(String.self, forKey: .version)
        description = try container.decodeIfPresent(String.self, forKey: .description)
        entrypoint = try container.decode(String.self, forKey: .entrypoint)
        events = try container.decodeIfPresent([String].self, forKey: .events) ?? []
        commands = try container.decodeIfPresent([ExtensionPaletteCommand].self, forKey: .commands) ?? []
        permissions = try container.decodeIfPresent([ExtensionPermission].self, forKey: .permissions) ?? []
        aiProvider = try container.decodeIfPresent(ExtensionAIProvider.self, forKey: .aiProvider)
        enabled = try container.decodeIfPresent(Bool.self, forKey: .enabled) ?? true
    }
}

enum ExtensionLoadError: LocalizedError, Equatable {
    case manifestMissing(URL)
    case manifestInvalid(URL, String)
    case entrypointMissing(URL)
    case entrypointNotExecutable(URL)
    case invalidName(String)
    case duplicateName(String)

    var errorDescription: String? {
        switch self {
        case let .manifestMissing(url):
            "Manifest not found at \(url.path)"
        case let .manifestInvalid(url, reason):
            "Invalid manifest at \(url.path): \(reason)"
        case let .entrypointMissing(url):
            "Entrypoint not found at \(url.path)"
        case let .entrypointNotExecutable(url):
            "Entrypoint at \(url.path) is not executable"
        case let .invalidName(name):
            "Extension name '\(name)' contains invalid characters (use letters, digits, dash, underscore, dot)"
        case let .duplicateName(name):
            "Duplicate extension name '\(name)'"
        }
    }
}

struct MuxyExtension: Identifiable, Equatable {
    let id: String
    let directory: URL
    let manifest: ExtensionManifest

    var entrypointURL: URL {
        directory.appendingPathComponent(manifest.entrypoint)
    }

    var displayName: String { manifest.name }
}

enum ExtensionManifestLoader {
    private static let allowedNameCharacters: CharacterSet = {
        var set = CharacterSet.alphanumerics
        set.insert(charactersIn: "-_.")
        return set
    }()

    static func load(from directory: URL) throws -> MuxyExtension {
        let manifestURL = directory.appendingPathComponent("manifest.json")
        guard FileManager.default.fileExists(atPath: manifestURL.path) else {
            throw ExtensionLoadError.manifestMissing(manifestURL)
        }

        let data: Data
        do {
            data = try Data(contentsOf: manifestURL)
        } catch {
            throw ExtensionLoadError.manifestInvalid(manifestURL, error.localizedDescription)
        }

        let manifest: ExtensionManifest
        do {
            manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)
        } catch {
            throw ExtensionLoadError.manifestInvalid(manifestURL, error.localizedDescription)
        }

        try validate(name: manifest.name)

        let entrypoint = directory.appendingPathComponent(manifest.entrypoint)
        guard FileManager.default.fileExists(atPath: entrypoint.path) else {
            throw ExtensionLoadError.entrypointMissing(entrypoint)
        }
        guard FileManager.default.isExecutableFile(atPath: entrypoint.path) else {
            throw ExtensionLoadError.entrypointNotExecutable(entrypoint)
        }

        return MuxyExtension(id: manifest.name, directory: directory, manifest: manifest)
    }

    static func validate(name: String) throws {
        guard !name.isEmpty else { throw ExtensionLoadError.invalidName(name) }
        for scalar in name.unicodeScalars where !allowedNameCharacters.contains(scalar) {
            throw ExtensionLoadError.invalidName(name)
        }
    }
}
