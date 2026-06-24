import Foundation

@MainActor
@Observable
final class BrowserSuggestionModel {
    var suggestions: [BrowserHistoryEntry] = []
    var selectedIndex: Int?
    var hoveredEntryID: UUID?

    var isEmpty: Bool { suggestions.isEmpty }

    var selectedEntry: BrowserHistoryEntry? {
        guard let selectedIndex, suggestions.indices.contains(selectedIndex) else { return nil }
        return suggestions[selectedIndex]
    }

    var activeEntry: BrowserHistoryEntry? {
        selectedEntry ?? hoveredEntry
    }

    func update(_ entries: [BrowserHistoryEntry]) {
        suggestions = entries
        selectedIndex = nil
        hoveredEntryID = nil
    }

    func clear() {
        suggestions = []
        selectedIndex = nil
        hoveredEntryID = nil
    }

    func moveSelection(_ delta: Int) {
        guard !suggestions.isEmpty else { return }
        hoveredEntryID = nil
        let current = selectedIndex ?? (delta > 0 ? -1 : 0)
        let next = current + delta
        guard suggestions.indices.contains(next) else {
            selectedIndex = nil
            return
        }
        selectedIndex = next
    }

    func hover(_ entry: BrowserHistoryEntry?) {
        hoveredEntryID = entry?.id
    }

    private var hoveredEntry: BrowserHistoryEntry? {
        guard let hoveredEntryID else { return nil }
        return suggestions.first { $0.id == hoveredEntryID }
    }
}
