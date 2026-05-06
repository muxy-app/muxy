import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "UIScale")

@MainActor
@Observable
final class UIScale {
    static let shared = UIScale()

    enum Preset: String, Codable, CaseIterable, Identifiable {
        case regular
        case large
        case extraLarge

        var id: String { rawValue }

        var multiplier: CGFloat {
            switch self {
            case .regular: 1.00
            case .large: 1.12
            case .extraLarge: 1.24
            }
        }

        var title: String {
            switch self {
            case .regular: "Default"
            case .large: "Large"
            case .extraLarge: "Extra Large"
            }
        }
    }

    static let defaultPreset: Preset = .regular

    var preset: Preset = UIScale.defaultPreset {
        didSet { save() }
    }

    var multiplier: CGFloat { preset.multiplier }

    @ObservationIgnored private let store: CodableFileStore<Snapshot>
    @ObservationIgnored private var isLoading = false

    private init() {
        store = CodableFileStore(
            fileURL: MuxyFileStorage.fileURL(filename: "ui-scale.json"),
            options: CodableFileStoreOptions(
                prettyPrinted: true,
                sortedKeys: true,
                filePermissions: FilePermissions.privateFile
            )
        )
        load()
    }

    private func load() {
        do {
            guard let snapshot = try store.load() else { return }
            isLoading = true
            preset = snapshot.preset
            isLoading = false
        } catch {
            logger.error("Failed to load UI scale settings: \(error.localizedDescription)")
        }
    }

    private func save() {
        guard !isLoading else { return }
        do {
            try store.save(Snapshot(preset: preset))
        } catch {
            logger.error("Failed to save UI scale settings: \(error.localizedDescription)")
        }
    }
}

private struct Snapshot: Codable {
    let preset: UIScale.Preset
}
