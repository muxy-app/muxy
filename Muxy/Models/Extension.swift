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
    case panelsWrite = "panels:write"
    case commandsRunScript = "commands:run-script"
    case commandsExec = "commands:exec"

    enum Kind {
        case read
        case write
        case action
    }

    var kind: Kind {
        switch self {
        case .panesRead,
             .tabsRead,
             .projectsRead,
             .worktreesRead:
            .read
        case .panesWrite,
             .tabsWrite,
             .projectsWrite,
             .worktreesWrite,
             .notificationsWrite,
             .panelsWrite:
            .write
        case .commandsRunScript,
             .commandsExec:
            .action
        }
    }
}

struct ExtensionTabType: Codable, Equatable, Identifiable {
    let id: String
    let title: String
    let entry: String
    let defaultData: ExtensionJSON?
}

struct ExtensionPanel: Codable, Equatable, Identifiable {
    let id: String
    let title: String?
    let icon: ExtensionIcon?
    let entry: String
    let position: PanelPosition
    let mode: PanelMode
    let hiddenControls: [PanelHeaderControl]
    let defaultData: ExtensionJSON?

    private enum CodingKeys: String, CodingKey {
        case id
        case title
        case icon
        case entry
        case position
        case mode
        case hiddenControls
        case defaultData
    }

    init(
        id: String,
        title: String? = nil,
        icon: ExtensionIcon? = nil,
        entry: String,
        position: PanelPosition = .right,
        mode: PanelMode = .floating,
        hiddenControls: [PanelHeaderControl] = [],
        defaultData: ExtensionJSON? = nil
    ) {
        self.id = id
        self.title = title
        self.icon = icon
        self.entry = entry
        self.position = position
        self.mode = mode
        self.hiddenControls = hiddenControls
        self.defaultData = defaultData
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decodeIfPresent(String.self, forKey: .title)
        icon = try container.decodeIfPresent(ExtensionIcon.self, forKey: .icon)
        entry = try container.decode(String.self, forKey: .entry)
        position = try container.decodeIfPresent(PanelPosition.self, forKey: .position) ?? .right
        mode = try container.decodeIfPresent(PanelMode.self, forKey: .mode) ?? .floating
        hiddenControls = try container.decodeIfPresent([PanelHeaderControl].self, forKey: .hiddenControls) ?? []
        defaultData = try container.decodeIfPresent(ExtensionJSON.self, forKey: .defaultData)
    }
}

enum ExtensionIcon: Codable, Equatable {
    case symbol(String)
    case svg(String)

    private enum CodingKeys: String, CodingKey {
        case symbol
        case svg
    }

    init(from decoder: Decoder) throws {
        if let container = try? decoder.singleValueContainer(),
           let raw = try? container.decode(String.self)
        {
            self = .symbol(raw)
            return
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if let symbol = try container.decodeIfPresent(String.self, forKey: .symbol) {
            self = .symbol(symbol)
            return
        }
        if let svg = try container.decodeIfPresent(String.self, forKey: .svg) {
            self = .svg(svg)
            return
        }
        throw DecodingError.dataCorruptedError(
            forKey: CodingKeys.symbol,
            in: container,
            debugDescription: "Icon requires either a 'symbol' or 'svg' field"
        )
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .symbol(value): try container.encode(value, forKey: .symbol)
        case let .svg(value): try container.encode(value, forKey: .svg)
        }
    }
}

struct ExtensionTopbarItem: Codable, Equatable, Identifiable {
    let id: String
    let icon: ExtensionIcon
    let tooltip: String?
    let command: String
}

struct ExtensionStatusBarItem: Codable, Equatable, Identifiable {
    enum Side: String, Codable {
        case left
        case right
    }

    let id: String
    let icon: ExtensionIcon
    let text: String?
    let tooltip: String?
    let side: Side
    let command: String
}

enum ExtensionSettingType: String, Codable {
    case string
    case bool
    case number
}

struct ExtensionSettingEntry: Codable, Equatable, Identifiable {
    let key: String
    let title: String
    let description: String?
    let type: ExtensionSettingType
    let defaultValue: ExtensionJSON?

    var id: String { key }
}

enum ExtensionCommandAction: Codable, Equatable {
    case event
    case openTab(tabType: String, data: ExtensionJSON?)
    case togglePanel(panel: String)
    case runScript(script: String)

    private enum CodingKeys: String, CodingKey {
        case kind
        case tabType
        case panel
        case data
        case script
    }

    private enum Kind: String, Codable {
        case event
        case openTab
        case togglePanel
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
        case .togglePanel:
            let panel = try container.decode(String.self, forKey: .panel)
            self = .togglePanel(panel: panel)
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
        case let .togglePanel(panel):
            try container.encode(Kind.togglePanel, forKey: .kind)
            try container.encode(panel, forKey: .panel)
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
    let entrypoint: String?
    let events: [String]
    let commands: [ExtensionPaletteCommand]
    let tabTypes: [ExtensionTabType]
    let panels: [ExtensionPanel]
    let permissions: [ExtensionPermission]
    let aiProvider: ExtensionAIProvider?
    let topbarItems: [ExtensionTopbarItem]
    let statusBarItems: [ExtensionStatusBarItem]
    let settings: [ExtensionSettingEntry]

    private enum CodingKeys: String, CodingKey {
        case name
        case version
        case description
        case entrypoint
        case events
        case commands
        case tabTypes
        case panels
        case permissions
        case aiProvider
        case topbarItems
        case statusBarItems
        case settings
    }

    init(
        name: String,
        version: String,
        description: String? = nil,
        entrypoint: String? = nil,
        events: [String] = [],
        commands: [ExtensionPaletteCommand] = [],
        tabTypes: [ExtensionTabType] = [],
        panels: [ExtensionPanel] = [],
        permissions: [ExtensionPermission] = [],
        aiProvider: ExtensionAIProvider? = nil,
        topbarItems: [ExtensionTopbarItem] = [],
        statusBarItems: [ExtensionStatusBarItem] = [],
        settings: [ExtensionSettingEntry] = []
    ) {
        self.name = name
        self.version = version
        self.description = description
        self.entrypoint = entrypoint
        self.events = events
        self.commands = commands
        self.tabTypes = tabTypes
        self.panels = panels
        self.permissions = permissions
        self.aiProvider = aiProvider
        self.topbarItems = topbarItems
        self.statusBarItems = statusBarItems
        self.settings = settings
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        name = try container.decode(String.self, forKey: .name)
        version = try container.decode(String.self, forKey: .version)
        description = try container.decodeIfPresent(String.self, forKey: .description)
        entrypoint = try container.decodeIfPresent(String.self, forKey: .entrypoint)
        events = try container.decodeIfPresent([String].self, forKey: .events) ?? []
        commands = try container.decodeIfPresent([ExtensionPaletteCommand].self, forKey: .commands) ?? []
        tabTypes = try container.decodeIfPresent([ExtensionTabType].self, forKey: .tabTypes) ?? []
        panels = try container.decodeIfPresent([ExtensionPanel].self, forKey: .panels) ?? []
        permissions = try container.decodeIfPresent([ExtensionPermission].self, forKey: .permissions) ?? []
        aiProvider = try container.decodeIfPresent(ExtensionAIProvider.self, forKey: .aiProvider)
        topbarItems = try container.decodeIfPresent([ExtensionTopbarItem].self, forKey: .topbarItems) ?? []
        statusBarItems = try container.decodeIfPresent([ExtensionStatusBarItem].self, forKey: .statusBarItems) ?? []
        settings = try container.decodeIfPresent([ExtensionSettingEntry].self, forKey: .settings) ?? []
    }

    func tabType(id: String) -> ExtensionTabType? {
        tabTypes.first { $0.id == id }
    }

    func panel(id: String) -> ExtensionPanel? {
        panels.first { $0.id == id }
    }

    func setting(key: String) -> ExtensionSettingEntry? {
        settings.first { $0.key == key }
    }

    func statusBarItem(id: String) -> ExtensionStatusBarItem? {
        statusBarItems.first { $0.id == id }
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
    case panelEntryMissing(panelID: String, url: URL)
    case panelEntryOutsideDirectory(panelID: String, url: URL)
    case duplicatePanel(String)
    case panelSVGMissing(panelID: String, url: URL)
    case panelSVGOutsideDirectory(panelID: String, url: URL)
    case commandReferencesUnknownTabType(commandID: String, tabType: String)
    case commandReferencesUnknownPanel(commandID: String, panel: String)
    case scriptMissing(commandID: String, url: URL)
    case scriptOutsideDirectory(commandID: String, url: URL)
    case topbarItemEmptyID
    case duplicateTopbarItem(String)
    case topbarItemReferencesUnknownCommand(itemID: String, command: String)
    case topbarItemSVGMissing(itemID: String, url: URL)
    case topbarItemSVGOutsideDirectory(itemID: String, url: URL)
    case statusBarItemEmptyID
    case duplicateStatusBarItem(String)
    case statusBarItemReferencesUnknownCommand(itemID: String, command: String)
    case statusBarItemSVGMissing(itemID: String, url: URL)
    case statusBarItemSVGOutsideDirectory(itemID: String, url: URL)
    case settingEmptyKey
    case duplicateSettingKey(String)

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
        case let .panelEntryMissing(panelID, url):
            "Panel '\(panelID)' entry not found at \(url.path)"
        case let .panelEntryOutsideDirectory(panelID, url):
            "Panel '\(panelID)' entry at \(url.path) escapes the extension directory"
        case let .duplicatePanel(id):
            "Duplicate panel '\(id)'"
        case let .panelSVGMissing(panelID, url):
            "Panel '\(panelID)' icon SVG not found at \(url.path)"
        case let .panelSVGOutsideDirectory(panelID, url):
            "Panel '\(panelID)' icon SVG at \(url.path) escapes the extension directory"
        case let .commandReferencesUnknownTabType(commandID, tabType):
            "Command '\(commandID)' references unknown tab type '\(tabType)'"
        case let .commandReferencesUnknownPanel(commandID, panel):
            "Command '\(commandID)' references unknown panel '\(panel)'"
        case let .scriptMissing(commandID, url):
            "Command '\(commandID)' script not found at \(url.path)"
        case let .scriptOutsideDirectory(commandID, url):
            "Command '\(commandID)' script at \(url.path) escapes the extension directory"
        case .topbarItemEmptyID:
            "Topbar item id must not be empty"
        case let .duplicateTopbarItem(id):
            "Duplicate topbar item '\(id)'"
        case let .topbarItemReferencesUnknownCommand(itemID, command):
            "Topbar item '\(itemID)' references unknown command '\(command)'"
        case let .topbarItemSVGMissing(itemID, url):
            "Topbar item '\(itemID)' icon SVG not found at \(url.path)"
        case let .topbarItemSVGOutsideDirectory(itemID, url):
            "Topbar item '\(itemID)' icon SVG at \(url.path) escapes the extension directory"
        case .statusBarItemEmptyID:
            "Status bar item id must not be empty"
        case let .duplicateStatusBarItem(id):
            "Duplicate status bar item '\(id)'"
        case let .statusBarItemReferencesUnknownCommand(itemID, command):
            "Status bar item '\(itemID)' references unknown command '\(command)'"
        case let .statusBarItemSVGMissing(itemID, url):
            "Status bar item '\(itemID)' icon SVG not found at \(url.path)"
        case let .statusBarItemSVGOutsideDirectory(itemID, url):
            "Status bar item '\(itemID)' icon SVG at \(url.path) escapes the extension directory"
        case .settingEmptyKey:
            "Setting key must not be empty"
        case let .duplicateSettingKey(key):
            "Duplicate setting key '\(key)'"
        }
    }
}

struct MuxyExtension: Identifiable, Equatable {
    let id: String
    let directory: URL
    let manifest: ExtensionManifest

    var entrypointURL: URL? {
        guard let entrypoint = manifest.entrypoint else { return nil }
        return directory.appendingPathComponent(entrypoint)
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

        if let entrypoint = manifest.entrypoint.map(directory.appendingPathComponent) {
            guard FileManager.default.fileExists(atPath: entrypoint.path) else {
                throw ExtensionLoadError.entrypointMissing(entrypoint)
            }
            guard FileManager.default.isExecutableFile(atPath: entrypoint.path) else {
                throw ExtensionLoadError.entrypointNotExecutable(entrypoint)
            }
        }

        let muxyExtension = MuxyExtension(id: manifest.name, directory: directory, manifest: manifest)
        try validateTabTypes(manifest: manifest, in: muxyExtension)
        try validatePanels(manifest: manifest, in: muxyExtension)
        try validateCommands(manifest: manifest, in: muxyExtension)
        try validateTopbarItems(manifest: manifest, in: muxyExtension)
        try validateStatusBarItems(manifest: manifest, in: muxyExtension)
        try validateSettings(manifest: manifest)

        migrateLegacyEnabledFlag(rawManifest: data, extensionID: manifest.name)

        return muxyExtension
    }

    private static func migrateLegacyEnabledFlag(rawManifest: Data, extensionID: String) {
        guard !ExtensionEnabledStore.hasOverride(extensionID: extensionID) else { return }
        guard let object = try? JSONSerialization.jsonObject(with: rawManifest) as? [String: Any],
              let legacyValue = object["enabled"] as? Bool
        else { return }
        ExtensionEnabledStore.setEnabled(legacyValue, extensionID: extensionID)
    }

    static func validate(name: String) throws {
        guard !name.isEmpty else { throw ExtensionLoadError.invalidName(name) }
        guard !name.hasPrefix(".") else { throw ExtensionLoadError.invalidName(name) }
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

    private static func validatePanels(manifest: ExtensionManifest, in muxyExtension: MuxyExtension) throws {
        var seen = Set<String>()
        for panel in manifest.panels {
            guard seen.insert(panel.id).inserted else {
                throw ExtensionLoadError.duplicatePanel(panel.id)
            }
            guard let url = muxyExtension.resolveResource(panel.entry) else {
                throw ExtensionLoadError.panelEntryOutsideDirectory(
                    panelID: panel.id,
                    url: muxyExtension.directory.appendingPathComponent(panel.entry)
                )
            }
            guard FileManager.default.fileExists(atPath: url.path) else {
                throw ExtensionLoadError.panelEntryMissing(panelID: panel.id, url: url)
            }
            if let icon = panel.icon {
                try validateIcon(
                    icon,
                    in: muxyExtension,
                    missing: { ExtensionLoadError.panelSVGMissing(panelID: panel.id, url: $0) },
                    outside: { ExtensionLoadError.panelSVGOutsideDirectory(panelID: panel.id, url: $0) }
                )
            }
        }
    }

    private static func validateCommands(manifest: ExtensionManifest, in muxyExtension: MuxyExtension) throws {
        let tabTypeIDs = Set(manifest.tabTypes.map(\.id))
        let panelIDs = Set(manifest.panels.map(\.id))
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
            case let .togglePanel(panel):
                guard panelIDs.contains(panel) else {
                    throw ExtensionLoadError.commandReferencesUnknownPanel(
                        commandID: command.id,
                        panel: panel
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

    static let maxIconSVGBytes = 256 * 1024

    private static func validateTopbarItems(manifest: ExtensionManifest, in muxyExtension: MuxyExtension) throws {
        let commandIDs = Set(manifest.commands.map(\.id))
        var seen = Set<String>()
        for item in manifest.topbarItems {
            guard !item.id.isEmpty else { throw ExtensionLoadError.topbarItemEmptyID }
            guard seen.insert(item.id).inserted else {
                throw ExtensionLoadError.duplicateTopbarItem(item.id)
            }
            guard commandIDs.contains(item.command) else {
                throw ExtensionLoadError.topbarItemReferencesUnknownCommand(
                    itemID: item.id,
                    command: item.command
                )
            }
            try validateIcon(
                item.icon,
                in: muxyExtension,
                missing: { ExtensionLoadError.topbarItemSVGMissing(itemID: item.id, url: $0) },
                outside: { ExtensionLoadError.topbarItemSVGOutsideDirectory(itemID: item.id, url: $0) }
            )
        }
    }

    private static func validateStatusBarItems(manifest: ExtensionManifest, in muxyExtension: MuxyExtension) throws {
        let commandIDs = Set(manifest.commands.map(\.id))
        var seen = Set<String>()
        for item in manifest.statusBarItems {
            guard !item.id.isEmpty else { throw ExtensionLoadError.statusBarItemEmptyID }
            guard seen.insert(item.id).inserted else {
                throw ExtensionLoadError.duplicateStatusBarItem(item.id)
            }
            guard commandIDs.contains(item.command) else {
                throw ExtensionLoadError.statusBarItemReferencesUnknownCommand(
                    itemID: item.id,
                    command: item.command
                )
            }
            try validateIcon(
                item.icon,
                in: muxyExtension,
                missing: { ExtensionLoadError.statusBarItemSVGMissing(itemID: item.id, url: $0) },
                outside: { ExtensionLoadError.statusBarItemSVGOutsideDirectory(itemID: item.id, url: $0) }
            )
        }
    }

    private static func validateIcon(
        _ icon: ExtensionIcon,
        in muxyExtension: MuxyExtension,
        missing: (URL) -> ExtensionLoadError,
        outside: (URL) -> ExtensionLoadError
    ) throws {
        guard case let .svg(path) = icon else { return }
        guard path.lowercased().hasSuffix(".svg") else {
            throw outside(muxyExtension.directory.appendingPathComponent(path))
        }
        guard let url = muxyExtension.resolveResource(path) else {
            throw outside(muxyExtension.directory.appendingPathComponent(path))
        }
        guard FileManager.default.fileExists(atPath: url.path) else {
            throw missing(url)
        }
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        if let size = attributes[.size] as? Int, size > maxIconSVGBytes {
            throw missing(url)
        }
    }

    private static func validateSettings(manifest: ExtensionManifest) throws {
        var seen = Set<String>()
        for entry in manifest.settings {
            guard !entry.key.isEmpty else { throw ExtensionLoadError.settingEmptyKey }
            guard seen.insert(entry.key).inserted else {
                throw ExtensionLoadError.duplicateSettingKey(entry.key)
            }
        }
    }
}
