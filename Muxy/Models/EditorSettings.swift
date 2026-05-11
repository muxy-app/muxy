import AppKit
import os

private let logger = Logger(subsystem: "app.muxy", category: "EditorSettings")

@MainActor
@Observable
final class EditorSettings {
    static let shared = EditorSettings()

    enum DefaultEditor: String, Codable, CaseIterable, Identifiable {
        case builtIn
        case terminalCommand

        var id: String { rawValue }

        var displayName: String {
            switch self {
            case .builtIn:
                "Built-in Editor"
            case .terminalCommand:
                "Terminal Command"
            }
        }
    }

    static let systemFontFamilyToken = "System Default"
    static let defaultMarkdownPreviewFontFamily = systemFontFamilyToken
    static let defaultMarkdownPreviewFontScale: CGFloat = 1.0
    static let minMarkdownPreviewFontScale: CGFloat = 0.6
    static let maxMarkdownPreviewFontScale: CGFloat = 2.5
    static let markdownPreviewBaseFontSize: CGFloat = 14
    static let markdownPreviewZoomStep: CGFloat = 0.1

    static let defaultLineHeightMultiplier: CGFloat = 1.2
    static let minLineHeightMultiplier: CGFloat = 1.1
    static let maxLineHeightMultiplier: CGFloat = 2.0
    static let lineHeightMultiplierStep: CGFloat = 0.1

    var fontSize: CGFloat = 13 { didSet { save() } }
    var fontFamily: String = "SF Mono" { didSet { save() } }
    var defaultEditor: DefaultEditor = .builtIn { didSet { save() } }
    var externalEditorCommand: String = "vim" { didSet { save() } }
    var markdownPreviewFontFamily: String = EditorSettings.defaultMarkdownPreviewFontFamily { didSet { save() } }
    var markdownPreviewFontScale: CGFloat = EditorSettings.defaultMarkdownPreviewFontScale { didSet { save() } }
    var highlightCurrentLine: Bool = true { didSet { save() } }
    var lineWrapping: Bool = false { didSet { save() } }
    var showLineNumbers: Bool = true { didSet { save() } }

    var lineHeightMultiplier: CGFloat = EditorSettings.defaultLineHeightMultiplier {
        didSet { save() }
    }

    var richInputImageStrategy: RichInputImageStrategy = .clipboard { didSet { save() } }

    @ObservationIgnored private let store: CodableFileStore<Snapshot>
    @ObservationIgnored private var isBatchLoading = false

    var resolvedFont: NSFont {
        if let cached = cachedResolvedFont, cached.fontName == fontFamily, cached.pointSize == fontSize {
            return cached
        }
        let font = NSFont(name: fontFamily, size: fontSize)
            ?? NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        cachedResolvedFont = font
        return font
    }

    @ObservationIgnored private var cachedResolvedFont: NSFont?

    var resolvedMarkdownPreviewFontFamilyCSS: String {
        if markdownPreviewFontFamily == Self.systemFontFamilyToken {
            return Self.systemFontFamilyCSSStack
        }
        let escaped = markdownPreviewFontFamily
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        return "\"\(escaped)\", \(Self.systemFontFamilyCSSStack)"
    }

    func adjustMarkdownPreviewFontScale(by delta: CGFloat) {
        let next = markdownPreviewFontScale + delta
        markdownPreviewFontScale = min(
            Self.maxMarkdownPreviewFontScale,
            max(Self.minMarkdownPreviewFontScale, next)
        )
    }

    static let systemFontFamilyCSSStack =
        "-apple-system, BlinkMacSystemFont, \"Segoe UI\", Helvetica, Arial, sans-serif"

    static var availableMarkdownPreviewFonts: [String] {
        if let cached = cachedMarkdownPreviewFonts { return cached }
        let families = NSFontManager.shared.availableFontFamilies.sorted()
        let result = [systemFontFamilyToken] + families
        cachedMarkdownPreviewFonts = result
        return result
    }

    static var availableMonospacedFonts: [String] {
        if let cached = cachedMonospacedFonts { return cached }
        let result = NSFontManager.shared
            .availableFontFamilies
            .filter { family in
                guard let font = NSFont(name: family, size: 13) else { return false }
                return font.isFixedPitch || family.localizedCaseInsensitiveContains("mono")
                    || family.localizedCaseInsensitiveContains("courier")
                    || family.localizedCaseInsensitiveContains("menlo")
                    || family.localizedCaseInsensitiveContains("consolas")
            }
            .sorted()
        cachedMonospacedFonts = result
        return result
    }

    private static var cachedMarkdownPreviewFonts: [String]?
    private static var cachedMonospacedFonts: [String]?

    private init() {
        store = CodableFileStore(
            fileURL: MuxyFileStorage.fileURL(filename: "editor-settings.json"),
            options: CodableFileStoreOptions(
                prettyPrinted: true,
                sortedKeys: true,
                filePermissions: FilePermissions.privateFile
            )
        )
        load()
    }

    func resetToDefaults() {
        isBatchLoading = true
        fontSize = 13
        fontFamily = "SF Mono"
        defaultEditor = .builtIn
        externalEditorCommand = "vim"
        markdownPreviewFontFamily = Self.defaultMarkdownPreviewFontFamily
        markdownPreviewFontScale = Self.defaultMarkdownPreviewFontScale
        highlightCurrentLine = true
        lineWrapping = false
        showLineNumbers = true
        lineHeightMultiplier = Self.defaultLineHeightMultiplier
        richInputImageStrategy = .clipboard
        isBatchLoading = false
        save()
    }

    private func load() {
        do {
            guard let snapshot = try store.load() else { return }
            isBatchLoading = true
            fontSize = snapshot.fontSize ?? 13
            fontFamily = snapshot.fontFamily ?? "SF Mono"
            defaultEditor = snapshot.defaultEditor ?? snapshot.quickOpenEditor ?? .builtIn
            externalEditorCommand = snapshot.externalEditorCommand ?? "vim"
            markdownPreviewFontFamily = snapshot.markdownPreviewFontFamily ?? Self.defaultMarkdownPreviewFontFamily
            let loadedScale = snapshot.markdownPreviewFontScale ?? Self.defaultMarkdownPreviewFontScale
            markdownPreviewFontScale = min(
                max(loadedScale, Self.minMarkdownPreviewFontScale),
                Self.maxMarkdownPreviewFontScale
            )
            highlightCurrentLine = snapshot.highlightCurrentLine ?? true
            lineWrapping = snapshot.lineWrapping ?? false
            showLineNumbers = snapshot.showLineNumbers ?? true
            let loadedMultiplier = snapshot.lineHeightMultiplier ?? Self.defaultLineHeightMultiplier
            lineHeightMultiplier = min(
                max(loadedMultiplier, Self.minLineHeightMultiplier),
                Self.maxLineHeightMultiplier
            )
            richInputImageStrategy = snapshot.richInputImageStrategy ?? .clipboard
            isBatchLoading = false
        } catch {
            logger.error("Failed to load editor settings: \(error.localizedDescription)")
        }
    }

    private func save() {
        guard !isBatchLoading else { return }
        do {
            try store.save(Snapshot(
                fontSize: fontSize,
                fontFamily: fontFamily,
                defaultEditor: defaultEditor,
                quickOpenEditor: nil,
                externalEditorCommand: externalEditorCommand,
                markdownPreviewFontFamily: markdownPreviewFontFamily,
                markdownPreviewFontScale: markdownPreviewFontScale,
                highlightCurrentLine: highlightCurrentLine,
                lineWrapping: lineWrapping,
                showLineNumbers: showLineNumbers,
                lineHeightMultiplier: lineHeightMultiplier,
                richInputImageStrategy: richInputImageStrategy
            ))
        } catch {
            logger.error("Failed to save editor settings: \(error.localizedDescription)")
        }
    }
}

private struct Snapshot: Codable {
    let fontSize: CGFloat?
    let fontFamily: String?
    let defaultEditor: EditorSettings.DefaultEditor?
    let quickOpenEditor: EditorSettings.DefaultEditor?
    let externalEditorCommand: String?
    let markdownPreviewFontFamily: String?
    let markdownPreviewFontScale: CGFloat?
    let highlightCurrentLine: Bool?
    let lineWrapping: Bool?
    let showLineNumbers: Bool?
    let lineHeightMultiplier: CGFloat?
    let richInputImageStrategy: RichInputImageStrategy?
}
