import Foundation
import Testing

@testable import Muxy

@Suite("NavigationHistory")
@MainActor
struct NavigationHistoryTests {
    private func makeEntry(tab: Int = 0) -> NavigationEntry {
        NavigationEntry(
            projectID: UUID(),
            worktreeID: UUID(),
            areaID: UUID(),
            tabID: tab == 0 ? nil : UUID()
        )
    }

    @Test("starts empty with cursor at -1")
    func startsEmpty() {
        let history = NavigationHistory()
        #expect(history.entries.isEmpty)
        #expect(history.cursor == -1)
        #expect(history.current == nil)
        #expect(!history.canGoBack)
        #expect(!history.canGoForward)
    }

    @Test("record appends and advances cursor")
    func recordAppends() {
        let history = NavigationHistory()
        let a = makeEntry(tab: 1)
        let b = makeEntry(tab: 2)
        history.record(a)
        history.record(b)
        #expect(history.entries == [a, b])
        #expect(history.cursor == 1)
        #expect(history.current == b)
        #expect(history.canGoBack)
        #expect(!history.canGoForward)
    }

    @Test("record dedupes consecutive duplicates")
    func recordDedupes() {
        let history = NavigationHistory()
        let a = makeEntry(tab: 1)
        history.record(a)
        history.record(a)
        history.record(a)
        #expect(history.entries.count == 1)
        #expect(history.cursor == 0)
    }

    @Test("record truncates forward entries when cursor is not at end")
    func recordTruncatesForward() {
        let history = NavigationHistory()
        let a = makeEntry(tab: 1)
        let b = makeEntry(tab: 2)
        let c = makeEntry(tab: 3)
        let d = makeEntry(tab: 4)
        history.record(a)
        history.record(b)
        history.record(c)
        history.setCursor(0)
        history.record(d)
        #expect(history.entries == [a, d])
        #expect(history.cursor == 1)
    }

    @Test("record enforces maxSize by dropping oldest and adjusting cursor")
    func recordEnforcesMaxSize() {
        let history = NavigationHistory()
        var recorded: [NavigationEntry] = []
        for i in 1 ... 105 {
            let entry = makeEntry(tab: i)
            recorded.append(entry)
            history.record(entry)
        }
        #expect(history.entries.count == 100)
        #expect(history.entries.first == recorded[5])
        #expect(history.entries.last == recorded.last)
        #expect(history.cursor == 99)
    }

    @Test("performWithRecordingSuppressed blocks record inside closure")
    func suppressionBlocksRecord() {
        let history = NavigationHistory()
        let a = makeEntry(tab: 1)
        let b = makeEntry(tab: 2)
        history.record(a)
        history.performWithRecordingSuppressed {
            history.record(b)
        }
        #expect(history.entries == [a])
        history.record(b)
        #expect(history.entries == [a, b])
    }

    @Test("performWithRecordingSuppressed restores previous state when nested")
    func suppressionNests() {
        let history = NavigationHistory()
        let a = makeEntry(tab: 1)
        let b = makeEntry(tab: 2)
        history.performWithRecordingSuppressed {
            history.performWithRecordingSuppressed {
                history.record(a)
            }
            history.record(b)
        }
        #expect(history.entries.isEmpty)
    }

    @Test("removeEntry shifts cursor when removing earlier index")
    func removeEntryBeforeCursor() {
        let history = NavigationHistory()
        let a = makeEntry(tab: 1)
        let b = makeEntry(tab: 2)
        let c = makeEntry(tab: 3)
        history.record(a)
        history.record(b)
        history.record(c)
        history.removeEntry(at: 0)
        #expect(history.entries == [b, c])
        #expect(history.cursor == 1)
    }

    @Test("removeEntry clamps cursor when removing cursor at end")
    func removeEntryAtCursorEnd() {
        let history = NavigationHistory()
        let a = makeEntry(tab: 1)
        let b = makeEntry(tab: 2)
        history.record(a)
        history.record(b)
        history.removeEntry(at: 1)
        #expect(history.entries == [a])
        #expect(history.cursor == 0)
    }

    @Test("removeEntries preserves cursor when current survives")
    func removeEntriesPreservesCursor() {
        let history = NavigationHistory()
        let entries = (1 ... 5).map { makeEntry(tab: $0) }
        entries.forEach { history.record($0) }
        history.setCursor(2)
        history.removeEntries { $0 == entries[4] }
        #expect(history.entries == [entries[0], entries[1], entries[2], entries[3]])
        #expect(history.current == entries[2])
        #expect(history.cursor == 2)
    }

    @Test("removeEntries shifts cursor when earlier entries are removed")
    func removeEntriesShiftsCursor() {
        let history = NavigationHistory()
        let entries = (1 ... 5).map { makeEntry(tab: $0) }
        entries.forEach { history.record($0) }
        history.setCursor(3)
        history.removeEntries { $0 == entries[0] || $0 == entries[1] }
        #expect(history.entries == [entries[2], entries[3], entries[4]])
        #expect(history.current == entries[3])
        #expect(history.cursor == 1)
    }

    @Test("removeEntries falls back to entry before cursor when current is removed")
    func removeEntriesCurrentRemoved() {
        let history = NavigationHistory()
        let entries = (1 ... 4).map { makeEntry(tab: $0) }
        entries.forEach { history.record($0) }
        history.setCursor(1)
        history.removeEntries { $0 == entries[1] }
        #expect(history.entries == [entries[0], entries[2], entries[3]])
        #expect(history.current == entries[0])
        #expect(history.cursor == 0)
    }

    @Test("removeEntries falls back to first kept entry when cursor was at front and removed")
    func removeEntriesFrontRemoved() {
        let history = NavigationHistory()
        let entries = (1 ... 3).map { makeEntry(tab: $0) }
        entries.forEach { history.record($0) }
        history.setCursor(0)
        history.removeEntries { $0 == entries[0] }
        #expect(history.entries == [entries[1], entries[2]])
        #expect(history.cursor == 0)
    }

    @Test("removeEntries handles clearing everything")
    func removeEntriesClears() {
        let history = NavigationHistory()
        let entries = (1 ... 3).map { makeEntry(tab: $0) }
        entries.forEach { history.record($0) }
        history.removeEntries { _ in true }
        #expect(history.entries.isEmpty)
        #expect(history.cursor == -1)
    }

    @Test("setCursor rejects out-of-range indices")
    func setCursorBounds() {
        let history = NavigationHistory()
        let a = makeEntry(tab: 1)
        let b = makeEntry(tab: 2)
        history.record(a)
        history.record(b)
        history.setCursor(-1)
        #expect(history.cursor == 1)
        history.setCursor(5)
        #expect(history.cursor == 1)
        history.setCursor(0)
        #expect(history.cursor == 0)
    }
}
