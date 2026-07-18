import AppKit
import os

private let logger = Logger(subsystem: "app.muxy", category: "EditorSettings")

@MainActor
@Observable
final class EditorSettings {
    static let shared = EditorSettings()

    static let defaultLineHeightMultiplier: CGFloat = 1.2
    static let minLineHeightMultiplier: CGFloat = 1.1
    static let maxLineHeightMultiplier: CGFloat = 2.0
    static let lineHeightMultiplierStep: CGFloat = 0.1

    static let defaultRichInputFontFamily = "SF Mono"
    static let defaultRichInputLineHeightMultiplier: CGFloat = 1.2

    var richInputFontFamily: String = EditorSettings.defaultRichInputFontFamily { didSet { save() } }
    var richInputLineHeightMultiplier: CGFloat = EditorSettings.defaultRichInputLineHeightMultiplier {
        didSet { save() }
    }

    var richInputImageStrategy: RichInputImageStrategy = .clipboard { didSet { save() } }

    @ObservationIgnored private let store: CodableFileStore<Snapshot>
    @ObservationIgnored private var isBatchLoading = false

    static var availableMonospacedFonts: [String] {
        if let cached = cachedMonospacedFonts {
            return cached
        }
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
        richInputFontFamily = Self.defaultRichInputFontFamily
        richInputLineHeightMultiplier = Self.defaultRichInputLineHeightMultiplier
        richInputImageStrategy = .clipboard
        isBatchLoading = false
        save()
    }

    private func load() {
        do {
            guard let snapshot = try store.load() else { return }
            isBatchLoading = true
            richInputFontFamily = snapshot.richInputFontFamily ?? Self.defaultRichInputFontFamily
            let loadedRichInputMultiplier = snapshot.richInputLineHeightMultiplier
                ?? Self.defaultRichInputLineHeightMultiplier
            richInputLineHeightMultiplier = min(
                max(loadedRichInputMultiplier, Self.minLineHeightMultiplier),
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
                richInputFontFamily: richInputFontFamily,
                richInputLineHeightMultiplier: richInputLineHeightMultiplier,
                richInputImageStrategy: richInputImageStrategy
            ))
            SettingsJSONStore.syncUserSettingsFileWithCurrentSettings()
        } catch {
            logger.error("Failed to save editor settings: \(error.localizedDescription)")
        }
    }
}

private struct Snapshot: Codable {
    let richInputFontFamily: String?
    let richInputLineHeightMultiplier: CGFloat?
    let richInputImageStrategy: RichInputImageStrategy?
}
