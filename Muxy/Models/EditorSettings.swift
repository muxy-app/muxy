import AppKit
import os

private let logger = Logger(subsystem: "app.muxy", category: "EditorSettings")

@MainActor
@Observable
final class EditorSettings {
    static let shared = EditorSettings()

    var fontSize: CGFloat = 13 { didSet { save() } }
    var fontFamily: String = "SF Mono" { didSet { save() } }
    var wordWrap: Bool = true { didSet { save() } }
    var showLineNumbers: Bool = true { didSet { save() } }
    var tabSize: Int = 4 { didSet { save() } }
    var showInvisibles: Bool = false { didSet { save() } }

    @ObservationIgnored private let fileURL: URL
    @ObservationIgnored private var isBatchLoading = false

    var resolvedFont: NSFont {
        if let font = NSFont(name: fontFamily, size: fontSize) {
            return font
        }
        return NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
    }

    static var availableMonospacedFonts: [String] {
        NSFontManager.shared
            .availableFontFamilies
            .filter { family in
                guard let font = NSFont(name: family, size: 13) else { return false }
                return font.isFixedPitch || family.localizedCaseInsensitiveContains("mono")
                    || family.localizedCaseInsensitiveContains("courier")
                    || family.localizedCaseInsensitiveContains("menlo")
                    || family.localizedCaseInsensitiveContains("consolas")
            }
            .sorted()
    }

    private init() {
        fileURL = MuxyFileStorage.fileURL(filename: "editor-settings.json")
        load()
    }

    func resetToDefaults() {
        isBatchLoading = true
        fontSize = 13
        fontFamily = "SF Mono"
        wordWrap = true
        showLineNumbers = true
        tabSize = 4
        showInvisibles = false
        isBatchLoading = false
        save()
    }

    private func load() {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return }
        do {
            let data = try Data(contentsOf: fileURL)
            let snapshot = try JSONDecoder().decode(Snapshot.self, from: data)
            isBatchLoading = true
            fontSize = snapshot.fontSize ?? 13
            fontFamily = snapshot.fontFamily ?? "SF Mono"
            wordWrap = snapshot.wordWrap ?? true
            showLineNumbers = snapshot.showLineNumbers ?? true
            tabSize = snapshot.tabSize ?? 4
            showInvisibles = snapshot.showInvisibles ?? false
            isBatchLoading = false
        } catch {
            logger.error("Failed to load editor settings: \(error.localizedDescription)")
        }
    }

    private func save() {
        guard !isBatchLoading else { return }
        do {
            let snapshot = Snapshot(
                fontSize: fontSize,
                fontFamily: fontFamily,
                wordWrap: wordWrap,
                showLineNumbers: showLineNumbers,
                tabSize: tabSize,
                showInvisibles: showInvisibles
            )
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(snapshot)
            try data.write(to: fileURL, options: .atomic)
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o600],
                ofItemAtPath: fileURL.path
            )
        } catch {
            logger.error("Failed to save editor settings: \(error.localizedDescription)")
        }
    }
}

private struct Snapshot: Codable {
    let fontSize: CGFloat?
    let fontFamily: String?
    let wordWrap: Bool?
    let showLineNumbers: Bool?
    let tabSize: Int?
    let showInvisibles: Bool?
}
