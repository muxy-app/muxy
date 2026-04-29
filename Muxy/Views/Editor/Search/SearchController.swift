import AppKit

@MainActor
protocol SearchControllerHost: AnyObject {
    var textView: NSTextView? { get }
    var scrollView: NSScrollView? { get }
    var viewportState: ViewportState? { get }
    var lineStartOffsets: [Int] { get }
    var state: EditorTabState { get }
    var lastSyncedBackingStoreVersion: Int { get set }

    func charOffsetForLocalLine(_ localLine: Int) -> Int
    func refreshViewport(force: Bool)
    func reapplySyntaxHighlights()
    func invalidateSyntaxHighlightsFromLine(_ line: Int)
    func invalidateRenderedViewportText()
    func clearViewportHistory()
    func scheduleMarkdownPreviewRefresh(immediate: Bool)
    func recordHighlightTiming(start: CFTimeInterval?, highlightedRangeCount: Int, force: Bool)
    func beginPerfTiming() -> CFTimeInterval?
}

@MainActor
final class SearchController {
    private weak var host: SearchControllerHost?

    private(set) var matches: [TextBackingStore.SearchMatch] = []
    private var appliedRanges: [NSRange] = []
    private var appliedCurrentRange: NSRange?

    init(host: SearchControllerHost) {
        self.host = host
    }

    func clearHighlights() {
        guard let host else { return }
        matches = []
        host.state.searchMatchCount = 0
        host.state.searchCurrentIndex = 0
        applyHighlights()
    }

    func applyHighlights(force: Bool = false) {
        guard let host else { return }
        let perfStart = host.beginPerfTiming()
        var highlightedRangeCount = 0
        defer {
            host.recordHighlightTiming(start: perfStart, highlightedRangeCount: highlightedRangeCount, force: force)
        }

        guard let textView = host.textView, let layoutManager = textView.layoutManager else { return }
        let storageLength = textView.textStorage?.length ?? 0
        guard storageLength > 0 else {
            appliedRanges.removeAll(keepingCapacity: true)
            appliedCurrentRange = nil
            return
        }

        guard let viewport = host.viewportState, !matches.isEmpty else {
            guard !appliedRanges.isEmpty || appliedCurrentRange != nil else { return }
            clearAppliedHighlights(layoutManager: layoutManager, storageLength: storageLength)
            textView.needsDisplay = true
            return
        }

        var nextRanges: [NSRange] = []
        nextRanges.reserveCapacity(min(matches.count, 256))

        let currentIndex = max(0, host.state.searchCurrentIndex - 1)
        var nextCurrentRange: NSRange?
        let visibleStartLine = viewport.viewportStartLine
        let visibleEndLine = viewport.viewportEndLine

        for (i, match) in matches.enumerated() {
            if match.lineIndex < visibleStartLine { continue }
            if match.lineIndex >= visibleEndLine { break }
            guard let localLine = viewport.viewportLine(forBackingStoreLine: match.lineIndex) else { continue }
            let localCharOffset = host.charOffsetForLocalLine(localLine)
            let highlightRange = NSRange(
                location: localCharOffset + match.range.location,
                length: match.range.length
            )
            guard NSMaxRange(highlightRange) <= storageLength else { continue }
            nextRanges.append(highlightRange)
            if i == currentIndex {
                nextCurrentRange = highlightRange
            }
        }

        if !force,
           appliedRanges == nextRanges,
           appliedCurrentRange == nextCurrentRange
        {
            return
        }

        clearAppliedHighlights(layoutManager: layoutManager, storageLength: storageLength)
        guard !nextRanges.isEmpty else {
            textView.needsDisplay = true
            return
        }

        let matchBg = GhosttyService.shared.foregroundColor.withAlphaComponent(0.2)
        let themeYellow = GhosttyService.shared.paletteColor(at: 3) ?? NSColor.systemYellow
        let currentMatchBg = themeYellow.withAlphaComponent(0.85)
        let currentMatchFg = GhosttyService.shared.backgroundColor

        for highlightRange in nextRanges {
            if highlightRange == nextCurrentRange {
                layoutManager.addTemporaryAttribute(.backgroundColor, value: currentMatchBg, forCharacterRange: highlightRange)
                layoutManager.addTemporaryAttribute(.foregroundColor, value: currentMatchFg, forCharacterRange: highlightRange)
            } else {
                layoutManager.addTemporaryAttribute(.backgroundColor, value: matchBg, forCharacterRange: highlightRange)
            }
        }

        highlightedRangeCount = nextRanges.count
        appliedRanges = nextRanges
        appliedCurrentRange = nextCurrentRange
        textView.needsDisplay = true
    }

    func performSearch(_ needle: String, caseSensitive: Bool, useRegex: Bool) {
        guard let host, let store = host.state.backingStore else { return }
        host.state.searchInvalidRegex = false
        matches = []
        guard !needle.isEmpty else {
            host.state.searchMatchCount = 0
            host.state.searchCurrentIndex = 0
            applyHighlights()
            return
        }
        if useRegex, (try? NSRegularExpression(pattern: needle)) == nil {
            host.state.searchInvalidRegex = true
            host.state.searchMatchCount = 0
            host.state.searchCurrentIndex = 0
            applyHighlights()
            return
        }
        matches = store.search(needle: needle, caseSensitive: caseSensitive, useRegex: useRegex)
        host.state.searchMatchCount = matches.count
        if !matches.isEmpty {
            host.state.searchCurrentIndex = 1
            scrollToMatch(at: 0)
        } else {
            host.state.searchCurrentIndex = 0
            applyHighlights()
        }
    }

    func navigate(forward: Bool) {
        guard let host, !matches.isEmpty else { return }
        var idx = host.state.searchCurrentIndex - 1
        if forward {
            idx = (idx + 1) % matches.count
        } else {
            idx = (idx - 1 + matches.count) % matches.count
        }
        host.state.searchCurrentIndex = idx + 1
        scrollToMatch(at: idx)
    }

    func replaceCurrent(with replacement: String, needle: String, caseSensitive: Bool, useRegex: Bool) {
        guard let host, let store = host.state.backingStore, !needle.isEmpty, !matches.isEmpty else { return }
        host.clearViewportHistory()
        let currentIndex = max(0, host.state.searchCurrentIndex - 1)
        guard currentIndex < matches.count else { return }
        let match = matches[currentIndex]
        let line = store.line(at: match.lineIndex)
        let nsLine = line as NSString
        let newLine = nsLine.replacingCharacters(in: match.range, with: replacement)
        _ = store.replaceLines(in: match.lineIndex ..< match.lineIndex + 1, with: [newLine])
        host.state.backingStoreVersion += 1
        host.lastSyncedBackingStoreVersion = host.state.backingStoreVersion
        host.state.markModified()
        host.invalidateSyntaxHighlightsFromLine(match.lineIndex)
        host.invalidateRenderedViewportText()
        host.scheduleMarkdownPreviewRefresh(immediate: true)
        performSearch(needle, caseSensitive: caseSensitive, useRegex: useRegex)
        host.refreshViewport(force: true)
    }

    func replaceAll(with replacement: String, needle: String, caseSensitive: Bool, useRegex: Bool) {
        guard let host, let store = host.state.backingStore, !needle.isEmpty, !matches.isEmpty else { return }
        host.clearViewportHistory()
        var grouped: [Int: [NSRange]] = [:]
        for match in matches {
            grouped[match.lineIndex, default: []].append(match.range)
        }
        var earliestInvalidation = Int.max
        for lineIndex in grouped.keys.sorted().reversed() {
            guard let lineRanges = grouped[lineIndex] else { continue }
            let ranges = lineRanges.sorted { $0.location > $1.location }
            var nsLine = store.line(at: lineIndex) as NSString
            for range in ranges {
                nsLine = nsLine.replacingCharacters(in: range, with: replacement) as NSString
            }
            _ = store.replaceLines(in: lineIndex ..< lineIndex + 1, with: [nsLine as String])
            earliestInvalidation = min(earliestInvalidation, lineIndex)
        }
        if earliestInvalidation != Int.max {
            host.invalidateSyntaxHighlightsFromLine(earliestInvalidation)
        }
        host.state.backingStoreVersion += 1
        host.lastSyncedBackingStoreVersion = host.state.backingStoreVersion
        host.state.markModified()
        host.invalidateRenderedViewportText()
        host.scheduleMarkdownPreviewRefresh(immediate: true)
        performSearch(needle, caseSensitive: caseSensitive, useRegex: useRegex)
        host.refreshViewport(force: true)
    }

    private func clearAppliedHighlights(layoutManager: NSLayoutManager, storageLength: Int) {
        guard let host else { return }
        let hadCurrentMatchOverride = appliedCurrentRange != nil
        for range in appliedRanges {
            guard NSMaxRange(range) <= storageLength else { continue }
            layoutManager.removeTemporaryAttribute(.backgroundColor, forCharacterRange: range)
        }
        if let range = appliedCurrentRange, NSMaxRange(range) <= storageLength {
            layoutManager.removeTemporaryAttribute(.foregroundColor, forCharacterRange: range)
        }
        appliedRanges.removeAll(keepingCapacity: true)
        appliedCurrentRange = nil
        if hadCurrentMatchOverride {
            host.reapplySyntaxHighlights()
        }
    }

    private func scrollToMatch(at index: Int) {
        guard let host, index >= 0, index < matches.count,
              let viewport = host.viewportState,
              let scrollView = host.scrollView,
              let textView = host.textView
        else { return }
        let match = matches[index]
        let targetScrollY = viewport.scrollY(forLine: match.lineIndex)
        let visibleHeight = scrollView.contentView.bounds.height
        let centeredY = max(0, targetScrollY - visibleHeight / 2)
        scrollView.contentView.setBoundsOrigin(NSPoint(x: 0, y: centeredY))
        scrollView.reflectScrolledClipView(scrollView.contentView)
        host.refreshViewport(force: true)

        guard let localLine = viewport.viewportLine(forBackingStoreLine: match.lineIndex) else { return }
        let localCharOffset = host.charOffsetForLocalLine(localLine)
        let matchStart = localCharOffset + match.range.location
        let content = textView.string as NSString
        guard matchStart <= content.length else { return }
        textView.setSelectedRange(NSRange(location: matchStart, length: 0))
        applyHighlights()
    }
}
