import Foundation

enum ExtensionJSON: Codable, Equatable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([ExtensionJSON])
    case object([String: ExtensionJSON])

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
            return
        }
        if let bool = try? container.decode(Bool.self) {
            self = .bool(bool)
            return
        }
        if let number = try? container.decode(Double.self) {
            self = .number(number)
            return
        }
        if let string = try? container.decode(String.self) {
            self = .string(string)
            return
        }
        if let array = try? container.decode([ExtensionJSON].self) {
            self = .array(array)
            return
        }
        if let object = try? container.decode([String: ExtensionJSON].self) {
            self = .object(object)
            return
        }
        throw DecodingError.dataCorruptedError(
            in: container,
            debugDescription: "Unsupported JSON value"
        )
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null: try container.encodeNil()
        case let .bool(value): try container.encode(value)
        case let .number(value): try container.encode(value)
        case let .string(value): try container.encode(value)
        case let .array(value): try container.encode(value)
        case let .object(value): try container.encode(value)
        }
    }
}

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
    case commandsRunScript = "commands:run-script"
    case commandsExec = "commands:exec"
}

struct ExtensionTabType: Codable, Equatable, Identifiable {
    let id: String
    let title: String
    let entry: String
    let defaultData: ExtensionJSON?
}

enum ExtensionCommandAction: Codable, Equatable {
    case event
    case openTab(tabType: String, data: ExtensionJSON?)
    case runScript(script: String)

    private enum CodingKeys: String, CodingKey {
        case kind
        case tabType
        case data
        case script
    }

    private enum Kind: String, Codable {
        case event
        case openTab
        case runScript
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let kind = try container.decode(Kind.self, forKey: .kind)
        switch kind {
        case .event:
            self = .event
        case .openTab:
            let tabType = try container.decode(String.self, forKey: .tabType)
            let data = try container.decodeIfPresent(ExtensionJSON.self, forKey: .data)
            self = .openTab(tabType: tabType, data: data)
        case .runScript:
            let script = try container.decode(String.self, forKey: .script)
            self = .runScript(script: script)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .event:
            try container.encode(Kind.event, forKey: .kind)
        case let .openTab(tabType, data):
            try container.encode(Kind.openTab, forKey: .kind)
            try container.encode(tabType, forKey: .tabType)
            try container.encodeIfPresent(data, forKey: .data)
        case let .runScript(script):
            try container.encode(Kind.runScript, forKey: .kind)
            try container.encode(script, forKey: .script)
        }
    }
}

struct ExtensionPaletteCommand: Codable, Equatable, Identifiable {
    let id: String
    let title: String
    let subtitle: String?
    let action: ExtensionCommandAction

    private enum CodingKeys: String, CodingKey {
        case id
        case title
        case subtitle
        case action
    }

    init(id: String, title: String, subtitle: String? = nil, action: ExtensionCommandAction = .event) {
        self.id = id
        self.title = title
        self.subtitle = subtitle
        self.action = action
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decode(String.self, forKey: .title)
        subtitle = try container.decodeIfPresent(String.self, forKey: .subtitle)
        action = try container.decodeIfPresent(ExtensionCommandAction.self, forKey: .action) ?? .event
    }

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
    let tabTypes: [ExtensionTabType]
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
        case tabTypes
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
        tabTypes: [ExtensionTabType] = [],
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
        self.tabTypes = tabTypes
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
        tabTypes = try container.decodeIfPresent([ExtensionTabType].self, forKey: .tabTypes) ?? []
        permissions = try container.decodeIfPresent([ExtensionPermission].self, forKey: .permissions) ?? []
        aiProvider = try container.decodeIfPresent(ExtensionAIProvider.self, forKey: .aiProvider)
        enabled = try container.decodeIfPresent(Bool.self, forKey: .enabled) ?? true
    }

    func tabType(id: String) -> ExtensionTabType? {
        tabTypes.first { $0.id == id }
    }

    func withEnabled(_ enabled: Bool) -> ExtensionManifest {
        ExtensionManifest(
            name: name,
            version: version,
            description: description,
            entrypoint: entrypoint,
            events: events,
            commands: commands,
            tabTypes: tabTypes,
            permissions: permissions,
            aiProvider: aiProvider,
            enabled: enabled
        )
    }
}

enum ExtensionLoadError: LocalizedError, Equatable {
    case manifestMissing(URL)
    case manifestInvalid(URL, String)
    case entrypointMissing(URL)
    case entrypointNotExecutable(URL)
    case invalidName(String)
    case duplicateName(String)
    case tabTypeEntryMissing(tabTypeID: String, url: URL)
    case tabTypeEntryOutsideDirectory(tabTypeID: String, url: URL)
    case duplicateTabType(String)
    case commandReferencesUnknownTabType(commandID: String, tabType: String)
    case scriptMissing(commandID: String, url: URL)
    case scriptOutsideDirectory(commandID: String, url: URL)

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
        case let .tabTypeEntryMissing(tabTypeID, url):
            "Tab type '\(tabTypeID)' entry not found at \(url.path)"
        case let .tabTypeEntryOutsideDirectory(tabTypeID, url):
            "Tab type '\(tabTypeID)' entry at \(url.path) escapes the extension directory"
        case let .duplicateTabType(id):
            "Duplicate tab type '\(id)'"
        case let .commandReferencesUnknownTabType(commandID, tabType):
            "Command '\(commandID)' references unknown tab type '\(tabType)'"
        case let .scriptMissing(commandID, url):
            "Command '\(commandID)' script not found at \(url.path)"
        case let .scriptOutsideDirectory(commandID, url):
            "Command '\(commandID)' script at \(url.path) escapes the extension directory"
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

    func resolveResource(_ relativePath: String) -> URL? {
        let url = directory
            .appendingPathComponent(relativePath)
            .standardizedFileURL
            .resolvingSymlinksInPath()
        let base = directory.resolvingSymlinksInPath()
        guard url.path == base.path || url.path.hasPrefix(base.path + "/") else {
            return nil
        }
        return url
    }
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

        let muxyExtension = MuxyExtension(id: manifest.name, directory: directory, manifest: manifest)
        try validateTabTypes(manifest: manifest, in: muxyExtension)
        try validateCommands(manifest: manifest, in: muxyExtension)

        return muxyExtension
    }

    static func validate(name: String) throws {
        guard !name.isEmpty else { throw ExtensionLoadError.invalidName(name) }
        for scalar in name.unicodeScalars where !allowedNameCharacters.contains(scalar) {
            throw ExtensionLoadError.invalidName(name)
        }
    }

    private static func validateTabTypes(manifest: ExtensionManifest, in muxyExtension: MuxyExtension) throws {
        var seen = Set<String>()
        for tabType in manifest.tabTypes {
            guard seen.insert(tabType.id).inserted else {
                throw ExtensionLoadError.duplicateTabType(tabType.id)
            }
            guard let url = muxyExtension.resolveResource(tabType.entry) else {
                throw ExtensionLoadError.tabTypeEntryOutsideDirectory(
                    tabTypeID: tabType.id,
                    url: muxyExtension.directory.appendingPathComponent(tabType.entry)
                )
            }
            guard FileManager.default.fileExists(atPath: url.path) else {
                throw ExtensionLoadError.tabTypeEntryMissing(tabTypeID: tabType.id, url: url)
            }
        }
    }

    private static func validateCommands(manifest: ExtensionManifest, in muxyExtension: MuxyExtension) throws {
        let tabTypeIDs = Set(manifest.tabTypes.map(\.id))
        for command in manifest.commands {
            switch command.action {
            case .event:
                continue
            case let .openTab(tabType, _):
                guard tabTypeIDs.contains(tabType) else {
                    throw ExtensionLoadError.commandReferencesUnknownTabType(
                        commandID: command.id,
                        tabType: tabType
                    )
                }
            case let .runScript(script):
                guard let url = muxyExtension.resolveResource(script) else {
                    throw ExtensionLoadError.scriptOutsideDirectory(
                        commandID: command.id,
                        url: muxyExtension.directory.appendingPathComponent(script)
                    )
                }
                guard FileManager.default.fileExists(atPath: url.path) else {
                    throw ExtensionLoadError.scriptMissing(commandID: command.id, url: url)
                }
            }
        }
    }
}
