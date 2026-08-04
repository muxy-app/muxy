import Foundation
import os

private let tipsLogger = Logger(subsystem: "app.muxy", category: "TipsStore")

enum TipCatalog {
    static func decode(_ data: Data) throws -> [MuxyTip] {
        let tips = try JSONDecoder().decode([MuxyTip].self, from: data)
        guard !tips.isEmpty else {
            throw DecodingError.dataCorrupted(.init(
                codingPath: [],
                debugDescription: "Tips catalog cannot be empty"
            ))
        }
        return tips
    }

    static func load(bundle: Bundle = .appResources) -> [MuxyTip] {
        guard let url = bundle.url(forResource: "tips", withExtension: "json") else {
            tipsLogger.error("Bundled tips.json not found")
            return []
        }
        do {
            return try decode(Data(contentsOf: url))
        } catch {
            tipsLogger.error("Failed to load tips.json: \(error.localizedDescription)")
            return []
        }
    }
}

@MainActor
@Observable
final class TipsStore {
    static let shared = TipsStore()

    let tips: [MuxyTip]
    private(set) var currentIndex: Int

    init(
        tips: [MuxyTip] = TipCatalog.load(),
        startingIndex: (Int) -> Int = { Int.random(in: 0 ..< $0) }
    ) {
        self.tips = tips
        currentIndex = tips.isEmpty ? 0 : Self.validIndex(startingIndex(tips.count), count: tips.count)
    }

    var currentTip: MuxyTip? {
        guard tips.indices.contains(currentIndex) else { return nil }
        return tips[currentIndex]
    }

    var position: Int {
        guard currentTip != nil else { return 0 }
        return currentIndex + 1
    }

    func showNext() {
        guard !tips.isEmpty else { return }
        currentIndex = (currentIndex + 1) % tips.count
    }

    func showPrevious() {
        guard !tips.isEmpty else { return }
        currentIndex = (currentIndex - 1 + tips.count) % tips.count
    }

    private static func validIndex(_ index: Int, count: Int) -> Int {
        min(max(index, 0), count - 1)
    }
}
