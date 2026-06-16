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
    case projectsDelete = "projects:delete"
    case worktreesRead = "worktrees:read"
    case worktreesWrite = "worktrees:write"
    case gitRead = "git:read"
    case gitWrite = "git:write"
    case filesRead = "files:read"
    case filesWrite = "files:write"
    case notificationsWrite = "notifications:write"
    case panelsWrite = "panels:write"
    case commandsRunScript = "commands:run-script"
    case commandsExec = "commands:exec"
    case remoteServe = "remote:serve"

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
             .worktreesRead,
             .gitRead,
             .filesRead:
            .read
        case .panesWrite,
             .tabsWrite,
             .projectsWrite,
             .projectsDelete,
             .worktreesWrite,
             .gitWrite,
             .filesWrite,
             .notificationsWrite,
             .panelsWrite:
            .write
        case .commandsRunScript,
             .commandsExec,
             .remoteServe:
            .action
        }
    }

    var displayName: String {
        switch self {
        case .remoteServe: "remote-api"
        default: rawValue
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
    let headerButtons: [ExtensionPanelHeaderButton]
    let hideTopbar: Bool
    let defaultData: ExtensionJSON?

    private enum CodingKeys: String, CodingKey {
        case id
        case title
        case icon
        case entry
        case position
        case mode
        case hiddenControls
        case headerButtons
        case hideTopbar
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
        headerButtons: [ExtensionPanelHeaderButton] = [],
        hideTopbar: Bool = false,
        defaultData: ExtensionJSON? = nil
    ) {
        self.id = id
        self.title = title
        self.icon = icon
        self.entry = entry
        self.position = position
        self.mode = mode
        self.hiddenControls = hiddenControls
        self.headerButtons = headerButtons
        self.hideTopbar = hideTopbar
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
        headerButtons = try container.decodeIfPresent([ExtensionPanelHeaderButton].self, forKey: .headerButtons) ?? []
        hideTopbar = try container.decodeIfPresent(Bool.self, forKey: .hideTopbar) ?? false
        defaultData = try container.decodeIfPresent(ExtensionJSON.self, forKey: .defaultData)
    }
}

struct ExtensionPopover: Codable, Equatable, Identifiable {
    static let defaultWidth: Double = 320
    static let defaultHeight: Double = 360

    let id: String
    let title: String?
    let entry: String
    let width: Double
    let height: Double
    let defaultData: ExtensionJSON?

    private enum CodingKeys: String, CodingKey {
        case id
        case title
        case entry
        case width
        case height
        case defaultData
    }

    init(
        id: String,
        title: String? = nil,
        entry: String,
        width: Double = ExtensionPopover.defaultWidth,
        height: Double = ExtensionPopover.defaultHeight,
        defaultData: ExtensionJSON? = nil
    ) {
        self.id = id
        self.title = title
        self.entry = entry
        self.width = width
        self.height = height
        self.defaultData = defaultData
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decodeIfPresent(String.self, forKey: .title)
        entry = try container.decode(String.self, forKey: .entry)
        width = try container.decodeIfPresent(Double.self, forKey: .width) ?? ExtensionPopover.defaultWidth
        height = try container.decodeIfPresent(Double.self, forKey: .height) ?? ExtensionPopover.defaultHeight
        defaultData = try container.decodeIfPresent(ExtensionJSON.self, forKey: .defaultData)
    }
}

struct ExtensionSidebar: Codable, Equatable, Identifiable {
    let id: String
    let title: String?
    let icon: ExtensionIcon?
    let entry: String
    let defaultData: ExtensionJSON?

    private enum CodingKeys: String, CodingKey {
        case id
        case title
        case icon
        case entry
        case defaultData
    }

    init(
        id: String,
        title: String? = nil,
        icon: ExtensionIcon? = nil,
        entry: String,
        defaultData: ExtensionJSON? = nil
    ) {
        self.id = id
        self.title = title
        self.icon = icon
        self.entry = entry
        self.defaultData = defaultData
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decodeIfPresent(String.self, forKey: .title)
        icon = try container.decodeIfPresent(ExtensionIcon.self, forKey: .icon)
        entry = try container.decode(String.self, forKey: .entry)
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

    static func parse(_ value: Any?) -> ExtensionIcon? {
        if let raw = value as? String {
            return raw.isEmpty ? nil : .symbol(raw)
        }
        guard let dict = value as? [String: Any] else { return nil }
        if let symbol = dict["symbol"] as? String, !symbol.isEmpty {
            return .symbol(symbol)
        }
        if let svg = dict["svg"] as? String, !svg.isEmpty {
            return .svg(svg)
        }
        return nil
    }
}

struct ExtensionTopbarItem: Codable, Equatable, Identifiable {
    let id: String
    let icon: ExtensionIcon
    let tooltip: String?
    let command: String
    let visible: Bool

    init(id: String, icon: ExtensionIcon, tooltip: String?, command: String, visible: Bool = true) {
        self.id = id
        self.icon = icon
        self.tooltip = tooltip
        self.command = command
        self.visible = visible
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        icon = try container.decode(ExtensionIcon.self, forKey: .icon)
        tooltip = try container.decodeIfPresent(String.self, forKey: .tooltip)
        command = try container.decode(String.self, forKey: .command)
        visible = try container.decodeIfPresent(Bool.self, forKey: .visible) ?? true
    }
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
    let visible: Bool

    init(
        id: String,
        icon: ExtensionIcon,
        text: String?,
        tooltip: String?,
        side: Side,
        command: String,
        visible: Bool = true
    ) {
        self.id = id
        self.icon = icon
        self.text = text
        self.tooltip = tooltip
        self.side = side
        self.command = command
        self.visible = visible
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        icon = try container.decode(ExtensionIcon.self, forKey: .icon)
        text = try container.decodeIfPresent(String.self, forKey: .text)
        tooltip = try container.decodeIfPresent(String.self, forKey: .tooltip)
        side = try container.decode(Side.self, forKey: .side)
        command = try container.decode(String.self, forKey: .command)
        visible = try container.decodeIfPresent(Bool.self, forKey: .visible) ?? true
    }
}

struct ExtensionPanelHeaderButton: Codable, Equatable, Identifiable {
    let id: String
    let icon: ExtensionIcon
    let tooltip: String?
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
    case openPopover(popover: String)
    case runScript(script: String)

    var isAnchored: Bool {
        if case .openPopover = self { return true }
        return false
    }

    var requiredPermission: ExtensionPermission? {
        switch self {
        case .event:
            nil
        case .openTab:
            .tabsWrite
        case .togglePanel,
             .openPopover:
            .panelsWrite
        case .runScript:
            .commandsRunScript
        }
    }

    private enum CodingKeys: String, CodingKey {
        case kind
        case tabType
        case panel
        case popover
        case data
        case script
    }

    private enum Kind: String, Codable {
        case event
        case openTab
        case togglePanel
        case openPopover
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
        case .openPopover:
            let popover = try container.decode(String.self, forKey: .popover)
            self = .openPopover(popover: popover)
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
        case let .openPopover(popover):
            try container.encode(Kind.openPopover, forKey: .kind)
            try container.encode(popover, forKey: .popover)
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
    let defaultShortcut: String?

    private enum CodingKeys: String, CodingKey {
        case id
        case title
        case subtitle
        case action
        case defaultShortcut
    }

    init(
        id: String,
        title: String,
        subtitle: String? = nil,
        action: ExtensionCommandAction = .event,
        defaultShortcut: String? = nil
    ) {
        self.id = id
        self.title = title
        self.subtitle = subtitle
        self.action = action
        self.defaultShortcut = defaultShortcut
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decode(String.self, forKey: .title)
        subtitle = try container.decodeIfPresent(String.self, forKey: .subtitle)
        action = try container.decodeIfPresent(ExtensionCommandAction.self, forKey: .action) ?? .event
        defaultShortcut = try container.decodeIfPresent(String.self, forKey: .defaultShortcut)
    }

    var eventName: String { "command.\(id)" }

    var defaultCombo: KeyCombo? {
        defaultShortcut.flatMap(KeyCombo.init(parsing:))
    }
}

struct ExtensionRemoteMethod: Codable, Equatable, Identifiable {
    let id: String
    let description: String?
}

struct ExtensionManifest: Codable, Equatable {
    let name: String
    let version: String
    let description: String?
    let background: String?
    let events: [String]
    let commands: [ExtensionPaletteCommand]
    let tabTypes: [ExtensionTabType]
    let panels: [ExtensionPanel]
    let popovers: [ExtensionPopover]
    let sidebar: ExtensionSidebar?
    let permissions: [ExtensionPermission]
    let topbarItems: [ExtensionTopbarItem]
    let statusBarItems: [ExtensionStatusBarItem]
    let settings: [ExtensionSettingEntry]
    let remoteMethods: [ExtensionRemoteMethod]

    private enum CodingKeys: String, CodingKey {
        case name
        case version
        case description
        case background
        case events
        case commands
        case tabTypes
        case panels
        case popovers
        case sidebar
        case permissions
        case topbarItems
        case statusBarItems
        case settings
        case remoteMethods
    }

    init(
        name: String,
        version: String,
        description: String? = nil,
        background: String? = nil,
        events: [String] = [],
        commands: [ExtensionPaletteCommand] = [],
        tabTypes: [ExtensionTabType] = [],
        panels: [ExtensionPanel] = [],
        popovers: [ExtensionPopover] = [],
        sidebar: ExtensionSidebar? = nil,
        permissions: [ExtensionPermission] = [],
        topbarItems: [ExtensionTopbarItem] = [],
        statusBarItems: [ExtensionStatusBarItem] = [],
        settings: [ExtensionSettingEntry] = [],
        remoteMethods: [ExtensionRemoteMethod] = []
    ) {
        self.name = name
        self.version = version
        self.description = description
        self.background = background
        self.events = events
        self.commands = commands
        self.tabTypes = tabTypes
        self.panels = panels
        self.popovers = popovers
        self.sidebar = sidebar
        self.permissions = permissions
        self.topbarItems = topbarItems
        self.statusBarItems = statusBarItems
        self.settings = settings
        self.remoteMethods = remoteMethods
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        name = try container.decode(String.self, forKey: .name)
        version = try container.decode(String.self, forKey: .version)
        description = try container.decodeIfPresent(String.self, forKey: .description)
        background = try container.decodeIfPresent(String.self, forKey: .background)
        events = try container.decodeIfPresent([String].self, forKey: .events) ?? []
        commands = try container.decodeIfPresent([ExtensionPaletteCommand].self, forKey: .commands) ?? []
        tabTypes = try container.decodeIfPresent([ExtensionTabType].self, forKey: .tabTypes) ?? []
        panels = try container.decodeIfPresent([ExtensionPanel].self, forKey: .panels) ?? []
        popovers = try container.decodeIfPresent([ExtensionPopover].self, forKey: .popovers) ?? []
        sidebar = try container.decodeIfPresent(ExtensionSidebar.self, forKey: .sidebar)
        permissions = try container.decodeIfPresent([ExtensionPermission].self, forKey: .permissions) ?? []
        topbarItems = try container.decodeIfPresent([ExtensionTopbarItem].self, forKey: .topbarItems) ?? []
        statusBarItems = try container.decodeIfPresent([ExtensionStatusBarItem].self, forKey: .statusBarItems) ?? []
        settings = try container.decodeIfPresent([ExtensionSettingEntry].self, forKey: .settings) ?? []
        remoteMethods = try container.decodeIfPresent([ExtensionRemoteMethod].self, forKey: .remoteMethods) ?? []
    }

    init(package: PackageManifest) {
        let muxy = package.muxy
        name = package.name
        version = package.version
        description = muxy.description
        background = muxy.background
        events = muxy.events
        commands = muxy.commands
        tabTypes = muxy.tabTypes
        panels = muxy.panels
        popovers = muxy.popovers
        sidebar = muxy.sidebar
        permissions = muxy.permissions
        topbarItems = muxy.topbarItems
        statusBarItems = muxy.statusBarItems
        settings = muxy.settings
        remoteMethods = muxy.remoteMethods
    }

    func tabType(id: String) -> ExtensionTabType? {
        tabTypes.first { $0.id == id }
    }

    func panel(id: String) -> ExtensionPanel? {
        panels.first { $0.id == id }
    }

    func popover(id: String) -> ExtensionPopover? {
        popovers.first { $0.id == id }
    }

    func setting(key: String) -> ExtensionSettingEntry? {
        settings.first { $0.key == key }
    }

    func statusBarItem(id: String) -> ExtensionStatusBarItem? {
        statusBarItems.first { $0.id == id }
    }

    func topbarItem(id: String) -> ExtensionTopbarItem? {
        topbarItems.first { $0.id == id }
    }

    func remoteMethod(id: String) -> ExtensionRemoteMethod? {
        remoteMethods.first { $0.id == id }
    }
}

struct PackageManifest: Codable, Equatable {
    let name: String
    let version: String
    let muxy: MuxyManifestBody

    private enum CodingKeys: String, CodingKey {
        case name
        case version
        case muxy
    }
}

struct MuxyManifestBody: Codable, Equatable {
    let description: String?
    let background: String?
    let events: [String]
    let commands: [ExtensionPaletteCommand]
    let tabTypes: [ExtensionTabType]
    let panels: [ExtensionPanel]
    let popovers: [ExtensionPopover]
    let sidebar: ExtensionSidebar?
    let permissions: [ExtensionPermission]
    let topbarItems: [ExtensionTopbarItem]
    let statusBarItems: [ExtensionStatusBarItem]
    let settings: [ExtensionSettingEntry]
    let remoteMethods: [ExtensionRemoteMethod]

    private enum CodingKeys: String, CodingKey {
        case description
        case background
        case events
        case commands
        case tabTypes
        case panels
        case popovers
        case sidebar
        case permissions
        case topbarItems
        case statusBarItems
        case settings
        case remoteMethods
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        description = try container.decodeIfPresent(String.self, forKey: .description)
        background = try container.decodeIfPresent(String.self, forKey: .background)
        events = try container.decodeIfPresent([String].self, forKey: .events) ?? []
        commands = try container.decodeIfPresent([ExtensionPaletteCommand].self, forKey: .commands) ?? []
        tabTypes = try container.decodeIfPresent([ExtensionTabType].self, forKey: .tabTypes) ?? []
        panels = try container.decodeIfPresent([ExtensionPanel].self, forKey: .panels) ?? []
        popovers = try container.decodeIfPresent([ExtensionPopover].self, forKey: .popovers) ?? []
        sidebar = try container.decodeIfPresent(ExtensionSidebar.self, forKey: .sidebar)
        permissions = try container.decodeIfPresent([ExtensionPermission].self, forKey: .permissions) ?? []
        topbarItems = try container.decodeIfPresent([ExtensionTopbarItem].self, forKey: .topbarItems) ?? []
        statusBarItems = try container.decodeIfPresent([ExtensionStatusBarItem].self, forKey: .statusBarItems) ?? []
        settings = try container.decodeIfPresent([ExtensionSettingEntry].self, forKey: .settings) ?? []
        remoteMethods = try container.decodeIfPresent([ExtensionRemoteMethod].self, forKey: .remoteMethods) ?? []
    }
}

enum ExtensionLoadError: LocalizedError, Equatable {
    case manifestMissing(URL)
    case manifestInvalid(URL, String)
    case backgroundScriptMissing(URL)
    case backgroundScriptOutsideDirectory(URL)
    case invalidName(String)
    case nameDirectoryMismatch(name: String, directory: String)
    case duplicateName(String)
    case tabTypeEntryMissing(tabTypeID: String, url: URL)
    case tabTypeEntryOutsideDirectory(tabTypeID: String, url: URL)
    case duplicateTabType(String)
    case panelEntryMissing(panelID: String, url: URL)
    case panelEntryOutsideDirectory(panelID: String, url: URL)
    case duplicatePanel(String)
    case panelSVGMissing(panelID: String, url: URL)
    case panelSVGOutsideDirectory(panelID: String, url: URL)
    case panelHeaderButtonEmptyID(panelID: String)
    case duplicatePanelHeaderButton(panelID: String, buttonID: String)
    case panelHeaderButtonReferencesUnknownCommand(panelID: String, buttonID: String, command: String)
    case panelHeaderButtonSVGMissing(panelID: String, buttonID: String, url: URL)
    case panelHeaderButtonSVGOutsideDirectory(panelID: String, buttonID: String, url: URL)
    case popoverEntryMissing(popoverID: String, url: URL)
    case popoverEntryOutsideDirectory(popoverID: String, url: URL)
    case duplicatePopover(String)
    case sidebarEmptyID
    case sidebarEntryEmpty(sidebarID: String)
    case sidebarEntryMissing(sidebarID: String, url: URL)
    case sidebarEntryOutsideDirectory(sidebarID: String, url: URL)
    case sidebarSVGMissing(sidebarID: String, url: URL)
    case sidebarSVGOutsideDirectory(sidebarID: String, url: URL)
    case commandReferencesUnknownTabType(commandID: String, tabType: String)
    case commandReferencesUnknownPanel(commandID: String, panel: String)
    case commandReferencesUnknownPopover(commandID: String, popover: String)
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
    case remoteMethodEmptyID
    case remoteMethodInvalidID(String)
    case duplicateRemoteMethod(String)

    var errorDescription: String? {
        switch self {
        case let .manifestMissing(url):
            "Manifest not found at \(url.path)"
        case let .manifestInvalid(url, reason):
            "Invalid manifest at \(url.path): \(reason)"
        case let .backgroundScriptMissing(url):
            "Background script not found at \(url.path)"
        case let .backgroundScriptOutsideDirectory(url):
            "Background script at \(url.path) escapes the extension directory"
        case let .invalidName(name):
            "Extension name '\(name)' contains invalid characters (use letters, digits, dash, underscore, dot)"
        case let .nameDirectoryMismatch(name, directory):
            "Extension name '\(name)' must match its directory name '\(directory)'"
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
        case let .panelHeaderButtonEmptyID(panelID):
            "Panel '\(panelID)' has a header button with an empty id"
        case let .duplicatePanelHeaderButton(panelID, buttonID):
            "Panel '\(panelID)' has a duplicate header button '\(buttonID)'"
        case let .panelHeaderButtonReferencesUnknownCommand(panelID, buttonID, command):
            "Panel '\(panelID)' header button '\(buttonID)' references unknown command '\(command)'"
        case let .panelHeaderButtonSVGMissing(panelID, buttonID, url):
            "Panel '\(panelID)' header button '\(buttonID)' icon SVG not found at \(url.path)"
        case let .panelHeaderButtonSVGOutsideDirectory(panelID, buttonID, url):
            "Panel '\(panelID)' header button '\(buttonID)' icon SVG at \(url.path) escapes the extension directory"
        case let .popoverEntryMissing(popoverID, url):
            "Popover '\(popoverID)' entry not found at \(url.path)"
        case let .popoverEntryOutsideDirectory(popoverID, url):
            "Popover '\(popoverID)' entry at \(url.path) escapes the extension directory"
        case let .duplicatePopover(id):
            "Duplicate popover '\(id)'"
        case .sidebarEmptyID:
            "Sidebar id must not be empty"
        case let .sidebarEntryEmpty(sidebarID):
            "Sidebar '\(sidebarID)' entry must not be empty"
        case let .sidebarEntryMissing(sidebarID, url):
            "Sidebar '\(sidebarID)' entry not found at \(url.path)"
        case let .sidebarEntryOutsideDirectory(sidebarID, url):
            "Sidebar '\(sidebarID)' entry at \(url.path) escapes the extension directory"
        case let .sidebarSVGMissing(sidebarID, url):
            "Sidebar '\(sidebarID)' icon SVG not found at \(url.path)"
        case let .sidebarSVGOutsideDirectory(sidebarID, url):
            "Sidebar '\(sidebarID)' icon SVG at \(url.path) escapes the extension directory"
        case let .commandReferencesUnknownTabType(commandID, tabType):
            "Command '\(commandID)' references unknown tab type '\(tabType)'"
        case let .commandReferencesUnknownPanel(commandID, panel):
            "Command '\(commandID)' references unknown panel '\(panel)'"
        case let .commandReferencesUnknownPopover(commandID, popover):
            "Command '\(commandID)' references unknown popover '\(popover)'"
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
        case .remoteMethodEmptyID:
            "Remote method id must not be empty"
        case let .remoteMethodInvalidID(id):
            "Remote method id '\(id)' must not contain control characters or '|'"
        case let .duplicateRemoteMethod(id):
            "Duplicate remote method '\(id)'"
        }
    }
}

struct MuxyExtension: Identifiable, Equatable {
    let id: String
    let directory: URL
    let manifest: ExtensionManifest

    var backgroundScriptURL: URL? {
        guard let background = manifest.background else { return nil }
        return resolveResource(background)
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

    static let manifestFileName = "package.json"
    static let buildOutputDirectoryName = "dist"

    static func load(from directory: URL) throws -> MuxyExtension {
        let manifestURL = resolveManifestURL(in: directory)
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
            let package = try JSONDecoder().decode(PackageManifest.self, from: data)
            manifest = ExtensionManifest(package: package)
        } catch {
            throw ExtensionLoadError.manifestInvalid(manifestURL, error.localizedDescription)
        }

        try validate(name: manifest.name)

        let resourceRoot = resolveResourceRoot(for: directory)
        let muxyExtension = MuxyExtension(id: manifest.name, directory: resourceRoot, manifest: manifest)

        if let background = manifest.background {
            guard let backgroundURL = muxyExtension.resolveResource(background) else {
                throw ExtensionLoadError.backgroundScriptOutsideDirectory(
                    resourceRoot.appendingPathComponent(background)
                )
            }
            guard FileManager.default.fileExists(atPath: backgroundURL.path) else {
                throw ExtensionLoadError.backgroundScriptMissing(backgroundURL)
            }
        }

        try validateTabTypes(manifest: manifest, in: muxyExtension)
        try validatePanels(manifest: manifest, in: muxyExtension)
        try validatePopovers(manifest: manifest, in: muxyExtension)
        try validateSidebar(manifest: manifest, in: muxyExtension)
        try validateCommands(manifest: manifest, in: muxyExtension)
        try validateTopbarItems(manifest: manifest, in: muxyExtension)
        try validateStatusBarItems(manifest: manifest, in: muxyExtension)
        try validateSettings(manifest: manifest)
        try validateRemoteMethods(manifest: manifest)

        migrateLegacyEnabledFlag(rawManifest: data, extensionID: manifest.name)

        return muxyExtension
    }

    static func resolveResourceRoot(for directory: URL) -> URL {
        let buildOutput = directory.appendingPathComponent(buildOutputDirectoryName)
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: buildOutput.path, isDirectory: &isDirectory),
              isDirectory.boolValue
        else { return directory }
        return buildOutput
    }

    private static func resolveManifestURL(in directory: URL) -> URL {
        let buildManifest = directory
            .appendingPathComponent(buildOutputDirectoryName)
            .appendingPathComponent(manifestFileName)
        guard FileManager.default.fileExists(atPath: buildManifest.path) else {
            return directory.appendingPathComponent(manifestFileName)
        }
        return buildManifest
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
        let commandIDs = Set(manifest.commands.map(\.id))
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
            var seenButtons = Set<String>()
            for button in panel.headerButtons {
                guard !button.id.isEmpty else {
                    throw ExtensionLoadError.panelHeaderButtonEmptyID(panelID: panel.id)
                }
                guard seenButtons.insert(button.id).inserted else {
                    throw ExtensionLoadError.duplicatePanelHeaderButton(panelID: panel.id, buttonID: button.id)
                }
                guard commandIDs.contains(button.command) else {
                    throw ExtensionLoadError.panelHeaderButtonReferencesUnknownCommand(
                        panelID: panel.id,
                        buttonID: button.id,
                        command: button.command
                    )
                }
                try validateIcon(
                    button.icon,
                    in: muxyExtension,
                    missing: { ExtensionLoadError.panelHeaderButtonSVGMissing(panelID: panel.id, buttonID: button.id, url: $0) },
                    outside: { ExtensionLoadError.panelHeaderButtonSVGOutsideDirectory(panelID: panel.id, buttonID: button.id, url: $0) }
                )
            }
        }
    }

    private static func validateSidebar(manifest: ExtensionManifest, in muxyExtension: MuxyExtension) throws {
        guard let sidebar = manifest.sidebar else { return }
        guard !sidebar.id.isEmpty else { throw ExtensionLoadError.sidebarEmptyID }
        guard !sidebar.entry.isEmpty else {
            throw ExtensionLoadError.sidebarEntryEmpty(sidebarID: sidebar.id)
        }
        guard let url = muxyExtension.resolveResource(sidebar.entry) else {
            throw ExtensionLoadError.sidebarEntryOutsideDirectory(
                sidebarID: sidebar.id,
                url: muxyExtension.directory.appendingPathComponent(sidebar.entry)
            )
        }
        guard FileManager.default.fileExists(atPath: url.path) else {
            throw ExtensionLoadError.sidebarEntryMissing(sidebarID: sidebar.id, url: url)
        }
        if let icon = sidebar.icon {
            try validateIcon(
                icon,
                in: muxyExtension,
                missing: { ExtensionLoadError.sidebarSVGMissing(sidebarID: sidebar.id, url: $0) },
                outside: { ExtensionLoadError.sidebarSVGOutsideDirectory(sidebarID: sidebar.id, url: $0) }
            )
        }
    }

    private static func validatePopovers(manifest: ExtensionManifest, in muxyExtension: MuxyExtension) throws {
        var seen = Set<String>()
        for popover in manifest.popovers {
            guard seen.insert(popover.id).inserted else {
                throw ExtensionLoadError.duplicatePopover(popover.id)
            }
            guard let url = muxyExtension.resolveResource(popover.entry) else {
                throw ExtensionLoadError.popoverEntryOutsideDirectory(
                    popoverID: popover.id,
                    url: muxyExtension.directory.appendingPathComponent(popover.entry)
                )
            }
            guard FileManager.default.fileExists(atPath: url.path) else {
                throw ExtensionLoadError.popoverEntryMissing(popoverID: popover.id, url: url)
            }
        }
    }

    private static func validateCommands(manifest: ExtensionManifest, in muxyExtension: MuxyExtension) throws {
        let tabTypeIDs = Set(manifest.tabTypes.map(\.id))
        let panelIDs = Set(manifest.panels.map(\.id))
        let popoverIDs = Set(manifest.popovers.map(\.id))
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
            case let .openPopover(popover):
                guard popoverIDs.contains(popover) else {
                    throw ExtensionLoadError.commandReferencesUnknownPopover(
                        commandID: command.id,
                        popover: popover
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

    private static func validateRemoteMethods(manifest: ExtensionManifest) throws {
        var seen = Set<String>()
        for method in manifest.remoteMethods {
            guard !method.id.isEmpty else { throw ExtensionLoadError.remoteMethodEmptyID }
            guard !method.id.unicodeScalars.contains(where: { $0 == "|" || CharacterSet.controlCharacters.contains($0) }) else {
                throw ExtensionLoadError.remoteMethodInvalidID(method.id)
            }
            guard seen.insert(method.id).inserted else {
                throw ExtensionLoadError.duplicateRemoteMethod(method.id)
            }
        }
    }
}
