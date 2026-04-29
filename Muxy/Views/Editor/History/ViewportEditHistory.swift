import Foundation

struct ViewportCursor: Equatable {
    let line: Int
    let column: Int
}

struct PendingViewportEdit {
    let startLine: Int
    let oldLines: [String]
    let newLines: [String]
    let selectionBefore: ViewportCursor
}

struct ViewportEdit {
    let startLine: Int
    let oldLines: [String]
    let newLines: [String]
    let selectionBefore: ViewportCursor
    let selectionAfter: ViewportCursor
}

struct ViewportEditGroup {
    var edits: [ViewportEdit]
}

@MainActor
protocol ViewportEditHistoryHost: AnyObject {
    var viewportState: ViewportState? { get }
    var state: EditorTabState { get }
    var lastSyncedBackingStoreVersion: Int { get set }

    func adjustViewportRangeForReplacement(startLine: Int, replacedLineCount: Int, insertedLineCount: Int)
    func invalidateSyntaxHighlightsFromLine(_ line: Int)
    func invalidateRenderedViewportText()
    func scheduleMarkdownPreviewRefresh(immediate: Bool)
    func applyHistorySelection(_ selection: ViewportCursor)
}

@MainActor
final class ViewportEditHistory {
    static let undoLimit = 200
    static let coalesceInterval: CFTimeInterval = 1.0

    private weak var host: ViewportEditHistoryHost?

    var pendingEdit: PendingViewportEdit?
    private var undoStack: [ViewportEditGroup] = []
    private var redoStack: [ViewportEditGroup] = []
    private var lastEditTimestamp: CFTimeInterval?
    private(set) var isApplyingHistory = false

    init(host: ViewportEditHistoryHost) {
        self.host = host
    }

    var canUndo: Bool { !undoStack.isEmpty }
    var canRedo: Bool { !redoStack.isEmpty }

    func clear() {
        pendingEdit = nil
        undoStack.removeAll(keepingCapacity: false)
        redoStack.removeAll(keepingCapacity: false)
        lastEditTimestamp = nil
    }

    func push(_ edit: ViewportEdit) {
        let now = CFAbsoluteTimeGetCurrent()
        if shouldCoalesce(edit, now: now), var group = undoStack.popLast() {
            group.edits.append(edit)
            undoStack.append(group)
        } else {
            appendUndo(ViewportEditGroup(edits: [edit]))
        }
        redoStack.removeAll(keepingCapacity: false)
        lastEditTimestamp = now
    }

    func performUndo() -> Bool {
        guard let host, let viewport = host.viewportState else { return false }
        guard let group = undoStack.popLast(), !group.edits.isEmpty else { return false }

        isApplyingHistory = true
        defer { isApplyingHistory = false }

        var earliestInvalidation = Int.max
        for edit in group.edits.reversed() {
            let replaceRange = edit.startLine ..< edit.startLine + edit.newLines.count
            _ = viewport.backingStore.replaceLines(in: replaceRange, with: edit.oldLines)
            host.adjustViewportRangeForReplacement(
                startLine: edit.startLine,
                replacedLineCount: edit.newLines.count,
                insertedLineCount: edit.oldLines.count
            )
            earliestInvalidation = min(earliestInvalidation, edit.startLine)
        }
        if earliestInvalidation != Int.max {
            host.invalidateSyntaxHighlightsFromLine(earliestInvalidation)
        }
        host.state.backingStoreVersion += 1
        host.lastSyncedBackingStoreVersion = host.state.backingStoreVersion
        host.state.markModified()
        host.invalidateRenderedViewportText()
        host.scheduleMarkdownPreviewRefresh(immediate: true)
        appendRedo(group)
        if let selection = group.edits.first?.selectionBefore {
            host.applyHistorySelection(selection)
        }
        lastEditTimestamp = nil
        return true
    }

    func performRedo() -> Bool {
        guard let host, let viewport = host.viewportState else { return false }
        guard let group = redoStack.popLast(), !group.edits.isEmpty else { return false }

        isApplyingHistory = true
        defer { isApplyingHistory = false }

        var earliestInvalidation = Int.max
        for edit in group.edits {
            let replaceRange = edit.startLine ..< edit.startLine + edit.oldLines.count
            _ = viewport.backingStore.replaceLines(in: replaceRange, with: edit.newLines)
            host.adjustViewportRangeForReplacement(
                startLine: edit.startLine,
                replacedLineCount: edit.oldLines.count,
                insertedLineCount: edit.newLines.count
            )
            earliestInvalidation = min(earliestInvalidation, edit.startLine)
        }
        if earliestInvalidation != Int.max {
            host.invalidateSyntaxHighlightsFromLine(earliestInvalidation)
        }
        host.state.backingStoreVersion += 1
        host.lastSyncedBackingStoreVersion = host.state.backingStoreVersion
        host.state.markModified()
        host.invalidateRenderedViewportText()
        host.scheduleMarkdownPreviewRefresh(immediate: true)
        appendUndo(group)
        if let selection = group.edits.last?.selectionAfter {
            host.applyHistorySelection(selection)
        }
        lastEditTimestamp = nil
        return true
    }

    private func appendUndo(_ group: ViewportEditGroup) {
        undoStack.append(group)
        if undoStack.count > Self.undoLimit {
            undoStack.removeFirst(undoStack.count - Self.undoLimit)
        }
    }

    private func appendRedo(_ group: ViewportEditGroup) {
        redoStack.append(group)
        if redoStack.count > Self.undoLimit {
            redoStack.removeFirst(redoStack.count - Self.undoLimit)
        }
    }

    private func shouldCoalesce(_ edit: ViewportEdit, now: CFAbsoluteTime) -> Bool {
        guard let lastTimestamp = lastEditTimestamp else { return false }
        guard now - lastTimestamp <= Self.coalesceInterval else { return false }
        guard let lastEdit = undoStack.last?.edits.last else { return false }
        return lastEdit.selectionAfter.line == edit.selectionBefore.line
            && lastEdit.selectionAfter.column == edit.selectionBefore.column
    }
}
