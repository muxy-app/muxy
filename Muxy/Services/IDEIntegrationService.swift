import AppKit
import Foundation

@MainActor
final class IDEIntegrationService: ObservableObject {
    static let shared = IDEIntegrationService()

    struct IDEApplication: Identifiable, Hashable {
        enum Group: Int, Hashable {
            case editor
            case otherTool
        }

        let bundleIdentifier: String
        let displayName: String
        let appURL: URL
        let symbolName: String
        let rank: Int
        let group: Group

        var id: String {
            bundleIdentifier.isEmpty ? appURL.path : bundleIdentifier
        }
    }

    struct EditorLocation: Hashable {
        let filePath: String
        let line: Int
        let column: Int
    }

    struct AppMetadata: Hashable {
        let bundleIdentifier: String
        let displayName: String
        let executableName: String
        let category: String?
        let appURL: URL
    }

    struct MatchMetadata: Hashable {
        let symbolName: String
        let rank: Int
        let group: IDEApplication.Group
    }

    struct LaunchCommand: Hashable {
        let executablePath: String
        let arguments: [String]
    }

    struct KeywordMatch: Hashable {
        let keyword: String
        let symbolName: String
        let rank: Int
        let group: IDEApplication.Group
    }

    static let selectedBundleIdentifierKey = "muxy.ide.selectedBundleIdentifier"
    static let finderBundleIdentifier = "com.apple.finder"
    static let finderAppURL = URL(fileURLWithPath: "/System/Library/CoreServices/Finder.app")
    static let finderApplication = IDEApplication(
        bundleIdentifier: finderBundleIdentifier,
        displayName: "Finder",
        appURL: finderAppURL,
        symbolName: "folder",
        rank: 79,
        group: .otherTool
    )

    @Published private(set) var installedApps: [IDEApplication] = []
    @Published private(set) var selectedBundleIdentifier: String?

    private let workspace: NSWorkspace
    private let defaults: UserDefaults
    private let fileManager: FileManager

    init(
        workspace: NSWorkspace = .shared,
        defaults: UserDefaults = .standard,
        fileManager: FileManager = .default
    ) {
        self.workspace = workspace
        self.defaults = defaults
        self.fileManager = fileManager
        self.selectedBundleIdentifier = defaults.string(forKey: Self.selectedBundleIdentifierKey)
        refreshInstalledApps()
    }

    private func setSelectedBundleIdentifier(_ identifier: String) {
        defaults.set(identifier, forKey: Self.selectedBundleIdentifierKey)
        if selectedBundleIdentifier != identifier {
            selectedBundleIdentifier = identifier
        }
    }

    var defaultIDE: IDEApplication? {
        Self.resolveDefaultIDE(installedApps: installedApps, selectedBundleIdentifier: selectedBundleIdentifier)
    }

    func refreshInstalledApps() {
        let discoveryRoots = Self.discoveryRoots(fileManager: fileManager)
        let curatedAppURLs = Self.curatedBundleMetadata.keys.sorted().compactMap {
            workspace.urlForApplication(withBundleIdentifier: $0)
        }

        DispatchQueue.global(qos: .userInitiated).async { [weak self, discoveryRoots, curatedAppURLs] in
            let apps = Self.discoverInstalledApps(discoveryRoots: discoveryRoots, curatedAppURLs: curatedAppURLs)
            DispatchQueue.main.async {
                self?.installedApps = apps
            }
        }
    }

    @discardableResult
    func openProject(
        at path: String,
        highlightingFileAt filePath: String? = nil,
        line: Int? = nil,
        column: Int? = nil,
        in ide: IDEApplication? = nil
    ) -> Bool {
        guard let app = ide ?? defaultIDE else { return false }

        if app.bundleIdentifier == Self.finderBundleIdentifier {
            revealInFinder(at: path)
            setSelectedBundleIdentifier(app.bundleIdentifier)
            return true
        }

        let commands = Self.launchCommands(
            for: app,
            projectPath: path,
            editorLocation: editorLocation(filePath: filePath, line: line, column: column),
            availableCLICommands: availableCLICommands()
        )

        let launched = Self.launch(commands: commands)
        if launched {
            setSelectedBundleIdentifier(app.bundleIdentifier)
        }
        return launched
    }

    func revealInFinder(at path: String) {
        let url = URL(fileURLWithPath: path)
        var isDirectory: ObjCBool = false
        if fileManager.fileExists(atPath: path, isDirectory: &isDirectory), isDirectory.boolValue {
            workspace.open(url)
            return
        }
        workspace.activateFileViewerSelecting([url])
    }

    static func launchCommands(
        for ide: IDEApplication,
        projectPath: String,
        editorLocation: EditorLocation?,
        availableCLICommands: [String: String]
    ) -> [LaunchCommand] {
        switch launchStrategy(forBundleIdentifier: ide.bundleIdentifier) {
        case let .vscodeLike(commandNames):
            if let executablePath = resolveCLIPath(commandNames: commandNames, availableCLICommands: availableCLICommands) {
                var args = [projectPath]
                if let editorLocation {
                    args += ["--goto", vscodeGotoTarget(for: editorLocation)]
                }
                return [.init(executablePath: executablePath, arguments: args)]
            }
            return [genericOpenCommand(for: ide, projectPath: projectPath, filePath: editorLocation?.filePath)]

        case let .zed(commandNames):
            if let executablePath = resolveCLIPath(commandNames: commandNames, availableCLICommands: availableCLICommands) {
                var args = [projectPath]
                if let editorLocation {
                    args.append(zedTarget(for: editorLocation))
                }
                return [.init(executablePath: executablePath, arguments: args)]
            }
            return [genericOpenCommand(for: ide, projectPath: projectPath, filePath: editorLocation?.filePath)]

        case .jetbrains:
            return [genericOpenCommand(for: ide, projectPath: projectPath, filePath: editorLocation?.filePath)]

        case .generic:
            return [genericOpenCommand(for: ide, projectPath: projectPath, filePath: editorLocation?.filePath)]
        }
    }

    nonisolated static func openTargetArguments(projectPath: String, filePath: String?) -> [String] {
        var orderedPaths = [projectPath]

        if let filePath,
           !filePath.isEmpty,
           filePath != projectPath,
           !orderedPaths.contains(filePath)
        {
            orderedPaths.append(filePath)
        }

        return orderedPaths
    }

    nonisolated static func vscodeGotoTarget(for location: EditorLocation) -> String {
        let safeLine = max(1, location.line)
        let safeColumn = max(1, location.column)
        return "\(location.filePath):\(safeLine):\(safeColumn)"
    }

    nonisolated static func zedTarget(for location: EditorLocation) -> String {
        vscodeGotoTarget(for: location)
    }

    static func resolveDefaultIDE(
        installedApps: [IDEApplication],
        selectedBundleIdentifier: String?
    ) -> IDEApplication? {
        if selectedBundleIdentifier == finderBundleIdentifier {
            return finderApplication
        }
        if let selectedBundleIdentifier,
           let selected = installedApps.first(where: { $0.bundleIdentifier == selectedBundleIdentifier })
        {
            return selected
        }
        return installedApps.first
    }

    nonisolated private static func discoverInstalledApps(
        discoveryRoots: [URL],
        curatedAppURLs: [URL]
    ) -> [IDEApplication] {
        let fileManager = FileManager.default
        var discovered: [IDEApplication] = []
        var seenKeys = Set<String>()

        for root in discoveryRoots {
            for metadata in discoverAppMetadata(in: root, fileManager: fileManager) {
                guard let app = ideApplication(from: metadata) else { continue }
                let key = dedupeKey(for: app)
                guard seenKeys.insert(key).inserted else { continue }
                discovered.append(app)
            }
        }

        for appURL in curatedAppURLs {
            guard let metadata = loadMetadata(at: appURL) else { continue }
            guard let app = ideApplication(from: metadata) else { continue }
            let key = dedupeKey(for: app)
            guard seenKeys.insert(key).inserted else { continue }
            discovered.append(app)
        }

        return discovered.sorted(by: compareInstalledApps)
    }

    private static func launch(commands: [LaunchCommand]) -> Bool {
        for command in commands {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: command.executablePath)
            process.arguments = command.arguments

            do {
                try process.run()
            } catch {
                return false
            }
        }

        return true
    }

    nonisolated static func ideApplication(from metadata: AppMetadata) -> IDEApplication? {
        guard let match = matchMetadata(for: metadata) else { return nil }
        return IDEApplication(
            bundleIdentifier: metadata.bundleIdentifier,
            displayName: metadata.displayName,
            appURL: metadata.appURL,
            symbolName: match.symbolName,
            rank: match.rank,
            group: match.group
        )
    }

    nonisolated static func matchMetadata(for metadata: AppMetadata) -> MatchMetadata? {
        if let curated = curatedBundleMetadata[metadata.bundleIdentifier] {
            return curated
        }

        let loweredName = metadata.displayName.lowercased()
        let loweredExecutable = metadata.executableName.lowercased()
        let loweredIdentifier = metadata.bundleIdentifier.lowercased()
        let haystack = [loweredName, loweredExecutable, loweredIdentifier]
            .filter { !$0.isEmpty }
            .joined(separator: " ")

        if loweredIdentifier == "com.jetbrains.toolbox" || loweredName.contains("toolbox") {
            return nil
        }

        if loweredIdentifier.hasPrefix("com.jetbrains."),
           !loweredName.contains("toolbox")
        {
            return MatchMetadata(
                symbolName: jetbrainsLikeSymbolName(for: loweredIdentifier),
                rank: 40,
                group: aiCompanionBundleIdentifiers.contains(loweredIdentifier) ? .otherTool : .editor
            )
        }

        if let keywordMatch = keywordMatches.first(where: { containsKeyword($0.keyword, in: haystack) }) {
            return MatchMetadata(symbolName: keywordMatch.symbolName, rank: keywordMatch.rank, group: keywordMatch.group)
        }

        if metadata.category == developerToolsCategory,
           editorLikeNameFragments.contains(where: { containsKeyword($0, in: loweredName) || containsKeyword($0, in: loweredExecutable) })
        {
            return MatchMetadata(symbolName: "chevron.left.forwardslash.chevron.right", rank: 90, group: .editor)
        }

        return nil
    }

    nonisolated static func compareInstalledApps(_ lhs: IDEApplication, _ rhs: IDEApplication) -> Bool {
        if lhs.group != rhs.group {
            return lhs.group.rawValue < rhs.group.rawValue
        }
        if lhs.rank != rhs.rank {
            return lhs.rank < rhs.rank
        }
        let nameOrder = lhs.displayName.localizedCaseInsensitiveCompare(rhs.displayName)
        if nameOrder != .orderedSame {
            return nameOrder == .orderedAscending
        }
        return lhs.appURL.path.localizedCaseInsensitiveCompare(rhs.appURL.path) == .orderedAscending
    }

    nonisolated static func discoveryRoots(fileManager: FileManager) -> [URL] {
        [
            URL(fileURLWithPath: "/Applications"),
            fileManager.homeDirectoryForCurrentUser.appendingPathComponent("Applications"),
        ].filter { fileManager.fileExists(atPath: $0.path) }
    }

    nonisolated static func loadMetadata(at appURL: URL) -> AppMetadata? {
        let infoURL = appURL.appendingPathComponent("Contents/Info.plist")
        guard let info = NSDictionary(contentsOf: infoURL) as? [String: Any] else { return nil }

        let bundleIdentifier = info["CFBundleIdentifier"] as? String ?? ""
        let bundleName = (info["CFBundleDisplayName"] as? String)
            ?? (info["CFBundleName"] as? String)
            ?? appURL.deletingPathExtension().lastPathComponent
        let executableName = info["CFBundleExecutable"] as? String ?? ""
        let category = info["LSApplicationCategoryType"] as? String

        return AppMetadata(
            bundleIdentifier: bundleIdentifier,
            displayName: bundleName,
            executableName: executableName,
            category: category,
            appURL: appURL
        )
    }

    nonisolated private static func discoverAppMetadata(in root: URL, fileManager: FileManager) -> [AppMetadata] {
        guard let enumerator = fileManager.enumerator(
            at: root,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        )
        else {
            return []
        }

        var results: [AppMetadata] = []
        for case let url as URL in enumerator {
            guard url.pathExtension.lowercased() == "app" else { continue }
            if let metadata = loadMetadata(at: url) {
                results.append(metadata)
            }
            enumerator.skipDescendants()
        }
        return results
    }

    private func editorLocation(filePath: String?, line: Int?, column: Int?) -> EditorLocation? {
        guard let filePath, !filePath.isEmpty else { return nil }
        return EditorLocation(
            filePath: filePath,
            line: max(1, line ?? 1),
            column: max(1, column ?? 1)
        )
    }

    private func availableCLICommands() -> [String: String] {
        var result: [String: String] = [:]
        for commandName in Self.knownCLICommandNames {
            if let path = Self.executablePath(named: commandName) {
                result[commandName] = path
            }
        }
        return result
    }

    nonisolated private static func dedupeKey(for app: IDEApplication) -> String {
        if !app.bundleIdentifier.isEmpty {
            return app.bundleIdentifier
        }
        return app.appURL.standardizedFileURL.path
    }

    nonisolated private static func executablePath(named commandName: String) -> String? {
        let pathDirectories = (ProcessInfo.processInfo.environment["PATH"] ?? "")
            .split(separator: ":")
            .map(String.init)
            .filter { !$0.isEmpty }
        let searchDirectories = Array(NSOrderedSet(array: pathDirectories + [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
            "/bin",
        ])) as? [String] ?? pathDirectories

        for directory in searchDirectories {
            let candidate = URL(fileURLWithPath: directory).appendingPathComponent(commandName).path
            if FileManager.default.isExecutableFile(atPath: candidate) {
                return candidate
            }
        }

        return nil
    }

    nonisolated private static func resolveCLIPath(
        commandNames: [String],
        availableCLICommands: [String: String]
    ) -> String? {
        for commandName in commandNames {
            if let path = availableCLICommands[commandName] {
                return path
            }
        }
        return nil
    }

    nonisolated private static func launchStrategy(forBundleIdentifier bundleIdentifier: String) -> LaunchStrategy {
        if let commandNames = vscodeLikeBundleIdentifiers[bundleIdentifier] {
            return .vscodeLike(commandNames: commandNames)
        }
        if let commandNames = zedLikeBundleIdentifiers[bundleIdentifier] {
            return .zed(commandNames: commandNames)
        }
        if let commandNames = jetbrainsCLICommandNames[bundleIdentifier], !commandNames.isEmpty {
            return .jetbrains
        }
        return .generic
    }

    nonisolated private static func genericOpenCommand(for ide: IDEApplication, projectPath: String, filePath: String?) -> LaunchCommand {
        LaunchCommand(
            executablePath: "/usr/bin/open",
            arguments: ["-a", ide.appURL.path] + openTargetArguments(projectPath: projectPath, filePath: filePath)
        )
    }

    nonisolated private static func jetbrainsLikeSymbolName(for bundleIdentifier: String) -> String {
        aiCompanionBundleIdentifiers.contains(bundleIdentifier) ? "sparkles" : "chevron.left.forwardslash.chevron.right"
    }

    nonisolated private static func containsKeyword(_ keyword: String, in haystack: String) -> Bool {
        let haystackTokens = haystack
            .split(whereSeparator: { !$0.isLetter && !$0.isNumber })
            .map(String.init)
        let keywordTokens = keyword.lowercased()
            .split(whereSeparator: { !$0.isLetter && !$0.isNumber })
            .map(String.init)

        guard !keywordTokens.isEmpty else { return false }
        guard haystackTokens.count >= keywordTokens.count else { return false }

        if keywordTokens.count == 1 {
            return haystackTokens.contains(keywordTokens[0])
        }

        let lastStartIndex = haystackTokens.count - keywordTokens.count
        for startIndex in 0 ... lastStartIndex
            where Array(haystackTokens[startIndex ..< startIndex + keywordTokens.count]) == keywordTokens
        {
            return true
        }

        return false
    }

    nonisolated private static let developerToolsCategory = "public.app-category.developer-tools"

    nonisolated private static let vscodeLikeBundleIdentifiers: [String: [String]] = [
        "com.microsoft.VSCode": ["code"],
        "com.microsoft.VSCodeInsiders": ["code-insiders"],
        "com.todesktop.230313mzl4w4u92": ["cursor"],
        "com.exafunction.windsurf": ["windsurf"],
        "com.vscodium": ["codium", "vscodium"],
    ]

    nonisolated private static let zedLikeBundleIdentifiers: [String: [String]] = [
        "dev.zed.Zed": ["zed"],
    ]

    nonisolated private static let jetbrainsCLICommandNames: [String: [String]] = [
        "com.jetbrains.PhpStorm": ["phpstorm"],
        "com.jetbrains.WebStorm": ["webstorm"],
        "com.jetbrains.PyCharm": ["pycharm"],
        "com.jetbrains.IntelliJ-IDEA": ["idea", "intellij"],
        "com.jetbrains.CLion": ["clion"],
        "com.jetbrains.GoLand": ["goland"],
        "com.jetbrains.RubyMine": ["rubymine"],
        "com.jetbrains.DataGrip": ["datagrip"],
        "com.jetbrains.Rider": ["rider"],
        "com.jetbrainsFleet": ["fleet"],
        "com.jetbrains.air": ["air"],
    ]

    nonisolated private static let aiCompanionBundleIdentifiers: Set<String> = [
        "com.openai.codex",
        "ai.opencode.desktop",
        "com.google.antigravity",
        "com.jetbrains.air",
    ]

    nonisolated private static let knownCLICommandNames: Set<String> = [
        "air",
        "clion",
        "code",
        "code-insiders",
        "codium",
        "cursor",
        "datagrip",
        "fleet",
        "goland",
        "idea",
        "intellij",
        "phpstorm",
        "pycharm",
        "rider",
        "rubymine",
        "webstorm",
        "windsurf",
        "zed",
        "vscodium",
    ]

    nonisolated private static let curatedBundleMetadata: [String: MatchMetadata] = [
        "com.microsoft.VSCode": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 10, group: .editor),
        "com.microsoft.VSCodeInsiders": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 11, group: .editor),
        "com.vscodium": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 12, group: .editor),
        "com.todesktop.230313mzl4w4u92": .init(symbolName: "cursorarrow.click.2", rank: 13, group: .editor),
        "dev.zed.Zed": .init(symbolName: "bolt.horizontal", rank: 14, group: .editor),
        "com.exafunction.windsurf": .init(symbolName: "wind", rank: 15, group: .editor),
        "com.apple.dt.Xcode": .init(symbolName: "hammer", rank: 16, group: .editor),
        "com.jetbrains.PhpStorm": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 17, group: .editor),
        "com.jetbrains.WebStorm": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 18, group: .editor),
        "com.jetbrains.PyCharm": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 19, group: .editor),
        "com.jetbrains.IntelliJ-IDEA": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 20, group: .editor),
        "com.jetbrains.CLion": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 21, group: .editor),
        "com.jetbrains.GoLand": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 22, group: .editor),
        "com.jetbrains.RubyMine": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 23, group: .editor),
        "com.jetbrains.DataGrip": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 24, group: .editor),
        "com.jetbrains.Rider": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 25, group: .editor),
        "com.jetbrainsFleet": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 26, group: .editor),
        "com.panic.Nova": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 27, group: .editor),
        "com.sublimetext.4": .init(symbolName: "text.cursor", rank: 28, group: .editor),
        "com.barebones.bbedit": .init(symbolName: "text.cursor", rank: 29, group: .editor),
        "com.macromates.TextMate": .init(symbolName: "text.cursor", rank: 30, group: .editor),
        "org.gnu.Emacs": .init(symbolName: "text.cursor", rank: 31, group: .editor),
        "org.aquamacs.Aquamacs": .init(symbolName: "text.cursor", rank: 32, group: .editor),
        "com.openai.codex": .init(symbolName: "sparkles.rectangle.stack", rank: 80, group: .otherTool),
        "ai.opencode.desktop": .init(symbolName: "chevron.left.forwardslash.chevron.right", rank: 81, group: .otherTool),
        "com.google.antigravity": .init(symbolName: "sparkles", rank: 82, group: .otherTool),
        "com.jetbrains.air": .init(symbolName: "sparkles", rank: 84, group: .otherTool),
    ]

    nonisolated private static let editorLikeNameFragments: [String] = [
        "cursor",
        "zed",
        "studio",
        "storm",
        "editor",
        "nova",
        "fleet",
        "windsurf",
        "xcode",
        "sublime",
        "bbedit",
        "textmate",
        "emacs",
        "aquamacs",
        "phpstorm",
        "webstorm",
        "pycharm",
        "rubymine",
        "clion",
        "goland",
        "datagrip",
        "rider",
        "intellij",
        "idea",
        "android studio",
    ]

    nonisolated private static let keywordMatches: [KeywordMatch] = [
        .init(keyword: "visual studio code", symbolName: "chevron.left.forwardslash.chevron.right", rank: 10, group: .editor),
        .init(keyword: "vscode", symbolName: "chevron.left.forwardslash.chevron.right", rank: 11, group: .editor),
        .init(keyword: "code - insiders", symbolName: "chevron.left.forwardslash.chevron.right", rank: 12, group: .editor),
        .init(keyword: "vscodium", symbolName: "chevron.left.forwardslash.chevron.right", rank: 13, group: .editor),
        .init(keyword: "cursor", symbolName: "cursorarrow.click.2", rank: 14, group: .editor),
        .init(keyword: "zed", symbolName: "bolt.horizontal", rank: 15, group: .editor),
        .init(keyword: "windsurf", symbolName: "wind", rank: 16, group: .editor),
        .init(keyword: "xcode", symbolName: "hammer", rank: 17, group: .editor),
        .init(keyword: "phpstorm", symbolName: "chevron.left.forwardslash.chevron.right", rank: 18, group: .editor),
        .init(keyword: "webstorm", symbolName: "chevron.left.forwardslash.chevron.right", rank: 19, group: .editor),
        .init(keyword: "pycharm", symbolName: "chevron.left.forwardslash.chevron.right", rank: 20, group: .editor),
        .init(keyword: "rubymine", symbolName: "chevron.left.forwardslash.chevron.right", rank: 21, group: .editor),
        .init(keyword: "clion", symbolName: "chevron.left.forwardslash.chevron.right", rank: 22, group: .editor),
        .init(keyword: "goland", symbolName: "chevron.left.forwardslash.chevron.right", rank: 23, group: .editor),
        .init(keyword: "datagrip", symbolName: "chevron.left.forwardslash.chevron.right", rank: 24, group: .editor),
        .init(keyword: "rider", symbolName: "chevron.left.forwardslash.chevron.right", rank: 25, group: .editor),
        .init(keyword: "fleet", symbolName: "chevron.left.forwardslash.chevron.right", rank: 26, group: .editor),
        .init(keyword: "intellij", symbolName: "chevron.left.forwardslash.chevron.right", rank: 27, group: .editor),
        .init(keyword: "android studio", symbolName: "chevron.left.forwardslash.chevron.right", rank: 28, group: .editor),
        .init(keyword: "nova", symbolName: "chevron.left.forwardslash.chevron.right", rank: 29, group: .editor),
        .init(keyword: "sublime text", symbolName: "text.cursor", rank: 30, group: .editor),
        .init(keyword: "bbedit", symbolName: "text.cursor", rank: 31, group: .editor),
        .init(keyword: "textmate", symbolName: "text.cursor", rank: 32, group: .editor),
        .init(keyword: "emacs", symbolName: "text.cursor", rank: 33, group: .editor),
        .init(keyword: "aquamacs", symbolName: "text.cursor", rank: 34, group: .editor),
    ]

    private enum LaunchStrategy {
        case generic
        case vscodeLike(commandNames: [String])
        case zed(commandNames: [String])
        case jetbrains
    }
}
