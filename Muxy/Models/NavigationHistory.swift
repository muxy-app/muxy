import Foundation

struct NavigationEntry: Equatable, Hashable {
    let projectID: UUID
    let worktreeID: UUID
    let areaID: UUID
    let tabID: UUID?
}

@MainActor
@Observable
final class NavigationHistory {
    private(set) var entries: [NavigationEntry] = []
    private(set) var cursor: Int = -1
    private var isRecordingSuppressed: Bool = false

    private let maxSize: Int = 100

    var canGoBack: Bool { cursor > 0 }
    var canGoForward: Bool { cursor >= 0 && cursor < entries.count - 1 }

    var current: NavigationEntry? {
        guard cursor >= 0, cursor < entries.count else { return nil }
        return entries[cursor]
    }

    func record(_ entry: NavigationEntry) {
        guard !isRecordingSuppressed else { return }
        if let current, current == entry { return }
        if cursor < entries.count - 1 {
            entries.removeSubrange((cursor + 1)...)
        }
        entries.append(entry)
        cursor = entries.count - 1
        if entries.count > maxSize {
            let overflow = entries.count - maxSize
            entries.removeFirst(overflow)
            cursor -= overflow
        }
    }

    func performWithRecordingSuppressed(_ body: () -> Void) {
        let previous = isRecordingSuppressed
        isRecordingSuppressed = true
        defer { isRecordingSuppressed = previous }
        body()
    }

    func setCursor(_ index: Int) {
        guard index >= 0, index < entries.count else { return }
        cursor = index
    }

    func removeEntry(at index: Int) {
        guard index >= 0, index < entries.count else { return }
        entries.remove(at: index)
        if cursor > index {
            cursor -= 1
        } else if cursor >= entries.count {
            cursor = entries.count - 1
        }
    }

    func removeEntries(where predicate: (NavigationEntry) -> Bool) {
        let previousCursor = cursor
        var kept: [NavigationEntry] = []
        var cursorAtOrBefore: Int = -1
        for (index, entry) in entries.enumerated() {
            if predicate(entry) { continue }
            kept.append(entry)
            if index <= previousCursor {
                cursorAtOrBefore = kept.count - 1
            }
        }
        entries = kept
        cursor = kept.isEmpty ? -1 : max(0, cursorAtOrBefore)
    }
}
