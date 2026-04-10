import AppKit
import SwiftUI

struct LineLayoutInfo: Equatable {
    let lineNumber: Int
    let yOffset: CGFloat
    let height: CGFloat
}

private final class PlainPasteTextView: NSTextView {
    override func paste(_ sender: Any?) {
        pasteAsPlainText(sender)
    }
}

struct CodeEditorView: NSViewRepresentable {
    @Bindable var state: EditorTabState
    let editorSettings: EditorSettings
    let themeVersion: Int
    let focused: Bool
    let searchNeedle: String
    let searchNavigationVersion: Int
    let searchNavigationDirection: EditorSearchNavigationDirection
    let searchCaseSensitive: Bool
    let searchUseRegex: Bool
    let replaceText: String
    let replaceVersion: Int
    let replaceAllVersion: Int
    let onLineLayoutChange: ([LineLayoutInfo]) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(state: state, editorSettings: editorSettings)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autoresizingMask = [.width, .height]

        let textStorage = NSTextStorage()
        let layoutManager = NSLayoutManager()
        textStorage.addLayoutManager(layoutManager)

        let textContainer = NSTextContainer(containerSize: NSSize(
            width: scrollView.contentSize.width,
            height: CGFloat.greatestFiniteMagnitude
        ))
        textContainer.widthTracksTextView = true
        textContainer.lineFragmentPadding = 8
        layoutManager.addTextContainer(textContainer)

        let textView = PlainPasteTextView(frame: NSRect(origin: .zero, size: scrollView.contentSize), textContainer: textContainer)
        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        scrollView.documentView = textView

        textView.isEditable = true
        textView.isSelectable = true
        textView.allowsUndo = true
        textView.isRichText = false
        textView.usesFindBar = true
        textView.isIncrementalSearchingEnabled = true
        textView.drawsBackground = true
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.isAutomaticTextCompletionEnabled = false
        textView.isAutomaticLinkDetectionEnabled = false
        textView.smartInsertDeleteEnabled = false
        textView.isGrammarCheckingEnabled = false
        textView.isContinuousSpellCheckingEnabled = false
        textView.textContainerInset = NSSize(width: 0, height: 4)

        let font = editorSettings.resolvedFont
        textView.font = font
        textView.backgroundColor = GhosttyService.shared.backgroundColor
        textView.insertionPointColor = GhosttyService.shared.foregroundColor
        textView.textColor = GhosttyService.shared.foregroundColor
        textView.typingAttributes = [
            .font: font,
            .foregroundColor: GhosttyService.shared.foregroundColor,
        ]
        textView.selectedTextAttributes = [
            .backgroundColor: GhosttyService.shared.foregroundColor.withAlphaComponent(0.15),
        ]

        Self.applyWordWrap(editorSettings.wordWrap, to: textView, scrollView: scrollView)

        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        scrollView.contentView.postsBoundsChangedNotifications = true

        textView.delegate = context.coordinator
        context.coordinator.textView = textView
        state.registerContentProvider { [weak textView] in
            textView?.string
        }
        context.coordinator.setScrollObserver(for: scrollView, onLineLayoutChange: onLineLayoutChange)

        textView.undoManager?.removeAllActions()

        return scrollView
    }

    static func dismantleNSView(_ scrollView: NSScrollView, coordinator: Coordinator) {
        guard let textView = scrollView.documentView as? NSTextView else { return }
        coordinator.state.flushEditorContent(textView.string)
        coordinator.state.registerContentProvider(nil)
        textView.undoManager?.removeAllActions()
        if let window = textView.window, window.firstResponder === textView {
            window.makeFirstResponder(nil)
        }
        textView.delegate = nil
    }

    private static func claimFirstResponder(textView: NSTextView, attemptsRemaining: Int) {
        guard attemptsRemaining > 0 else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.02) { [weak textView] in
            guard let textView else { return }
            guard let window = textView.window else {
                claimFirstResponder(textView: textView, attemptsRemaining: attemptsRemaining - 1)
                return
            }
            window.makeFirstResponder(textView)
            textView.setSelectedRange(NSRange(location: 0, length: 0))
            textView.scrollRangeToVisible(NSRange(location: 0, length: 0))
        }
    }

    private static func applyWordWrap(_ wrap: Bool, to textView: NSTextView, scrollView: NSScrollView) {
        if wrap {
            textView.isHorizontallyResizable = false
            textView.textContainer?.widthTracksTextView = true
            textView.textContainer?.containerSize = NSSize(
                width: scrollView.contentSize.width,
                height: CGFloat.greatestFiniteMagnitude
            )
            scrollView.hasHorizontalScroller = false
        } else {
            textView.isHorizontallyResizable = true
            textView.textContainer?.widthTracksTextView = false
            textView.textContainer?.containerSize = NSSize(
                width: CGFloat.greatestFiniteMagnitude,
                height: CGFloat.greatestFiniteMagnitude
            )
            scrollView.hasHorizontalScroller = true
        }
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let textView = context.coordinator.textView else { return }
        let coordinator = context.coordinator

        let stateContentChanged = coordinator.lastSyncedContentVersion != state.contentVersion
        var replacedFullContent = false
        if stateContentChanged {
            coordinator.isUpdating = true
            textView.undoManager?.disableUndoRegistration()
            textView.string = state.content
            textView.undoManager?.enableUndoRegistration()
            textView.undoManager?.removeAllActions()
            coordinator.isUpdating = false
            coordinator.rebuildLineStartOffsets(using: state.content as NSString)
            coordinator.lastSyncedContentVersion = state.contentVersion
            replacedFullContent = true
        }

        let streamAppendChanged = coordinator.lastSyncedStreamAppendVersion != state.streamAppendVersion
        var appendedChunk = false
        if streamAppendChanged {
            coordinator.lastSyncedStreamAppendVersion = state.streamAppendVersion
            let chunks = state.dequeuePendingAppendChunks(maxCount: 2)
            if !chunks.isEmpty {
                coordinator.isUpdating = true
                textView.undoManager?.disableUndoRegistration()
                for chunk in chunks {
                    coordinator.appendChunkToTextView(chunk, textView: textView)
                }
                textView.didChangeText()
                textView.undoManager?.enableUndoRegistration()
                textView.undoManager?.removeAllActions()
                coordinator.isUpdating = false
                appendedChunk = true
            }
            if state.hasPendingAppendChunks {
                state.requestPendingAppendDrainIfNeeded()
            }
        }

        let contentChanged = replacedFullContent || appendedChunk
        let incrementalFinished = coordinator.wasIncrementalLoading && !state.isIncrementalLoading
        coordinator.wasIncrementalLoading = state.isIncrementalLoading

        if !coordinator.hasAppliedInitialContent, !state.content.isEmpty || contentChanged {
            coordinator.hasAppliedInitialContent = true
            textView.setSelectedRange(NSRange(location: 0, length: 0))
            textView.scrollRangeToVisible(NSRange(location: 0, length: 0))
            if focused {
                Self.claimFirstResponder(textView: textView, attemptsRemaining: 20)
            }
        }

        textView.backgroundColor = GhosttyService.shared.backgroundColor
        textView.insertionPointColor = GhosttyService.shared.foregroundColor

        let themeChanged = coordinator.lastThemeVersion != themeVersion
        if themeChanged {
            coordinator.lastThemeVersion = themeVersion
        }

        let font = editorSettings.resolvedFont
        let fontChanged = textView.font != font
        if fontChanged {
            textView.font = font
            textView.typingAttributes[.font] = font
        }

        let wrapChanged = coordinator.lastWordWrap != editorSettings.wordWrap
        if wrapChanged {
            coordinator.lastWordWrap = editorSettings.wordWrap
            Self.applyWordWrap(editorSettings.wordWrap, to: textView, scrollView: scrollView)
        }

        coordinator.tabSize = editorSettings.tabSize

        if contentChanged {
            if appendedChunk {
                if !state.isIncrementalLoading {
                    coordinator.resetHighlightedRange()
                    coordinator.highlightVisibleRange(force: true)
                }
            } else {
                coordinator.resetHighlightedRange()
                coordinator.highlightVisibleRange(force: true)
            }
        } else if incrementalFinished {
            coordinator.resetHighlightedRange()
            coordinator.highlightVisibleRange(force: true)
        } else if themeChanged || fontChanged {
            coordinator.applyHighlighting()
        }

        let searchOptionsChanged = coordinator.lastSearchCaseSensitive != searchCaseSensitive
            || coordinator.lastSearchUseRegex != searchUseRegex
        if coordinator.lastSearchNeedle != searchNeedle || searchOptionsChanged {
            coordinator.lastSearchNeedle = searchNeedle
            coordinator.lastSearchCaseSensitive = searchCaseSensitive
            coordinator.lastSearchUseRegex = searchUseRegex
            coordinator.performSearch(searchNeedle, caseSensitive: searchCaseSensitive, useRegex: searchUseRegex)
        }

        if coordinator.lastSearchNavigationVersion != searchNavigationVersion {
            coordinator.lastSearchNavigationVersion = searchNavigationVersion
            coordinator.navigateSearch(forward: searchNavigationDirection == .next)
        }

        if coordinator.lastReplaceVersion != replaceVersion {
            coordinator.lastReplaceVersion = replaceVersion
            coordinator.replaceCurrent(
                with: replaceText,
                needle: searchNeedle,
                caseSensitive: searchCaseSensitive,
                useRegex: searchUseRegex
            )
        }

        if coordinator.lastReplaceAllVersion != replaceAllVersion {
            coordinator.lastReplaceAllVersion = replaceAllVersion
            coordinator.replaceAll(
                with: replaceText,
                needle: searchNeedle,
                caseSensitive: searchCaseSensitive,
                useRegex: searchUseRegex
            )
        }

        coordinator.onLineLayoutChange = onLineLayoutChange

        let shouldInvalidateLayouts = themeChanged || fontChanged || wrapChanged || replacedFullContent
        if shouldInvalidateLayouts || incrementalFinished {
            coordinator.invalidateAndReportLayouts()
        } else if !(state.isIncrementalLoading && appendedChunk) {
            coordinator.reportLineLayouts()
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        let state: EditorTabState
        let editorSettings: EditorSettings
        weak var textView: NSTextView? {
            didSet {
                observeTextViewFrame()
                setupLineHighlight()
            }
        }

        var isUpdating = false
        var hasAppliedInitialContent = false
        var lastThemeVersion = -1
        var lastSearchNeedle = ""
        var lastSearchNavigationVersion = -1
        var lastSearchCaseSensitive = false
        var lastSearchUseRegex = false
        var lastReplaceVersion = 0
        var lastReplaceAllVersion = 0
        var lastWordWrap = true
        var lastSyncedContentVersion = -1
        var lastSyncedStreamAppendVersion = -1
        var wasIncrementalLoading = false
        var lastHighlightedRange: NSRange = .init(location: 0, length: 0)
        private static let highlightBuffer = 2000
        var tabSize = 4
        var onLineLayoutChange: ([LineLayoutInfo]) -> Void = { _ in }
        private weak var observedContentView: NSClipView?
        private weak var observedTextView: NSTextView?
        private var lineStartOffsets: [Int] = [0]
        private var pendingEditRange: NSRange?
        private var pendingEditReplacement = ""
        private var hasPendingEdit = false
        private var hasMultiplePendingEdits = false
        private var lastReportedLayouts: [LineLayoutInfo] = []
        private let lineHighlightView: NSView = {
            let view = NSView()
            view.wantsLayer = true
            view.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.06).cgColor
            return view
        }()

        private let bracketHighlightViews: [NSView] = [
            Coordinator.makeBracketHighlightView(),
            Coordinator.makeBracketHighlightView(),
        ]

        private static func makeBracketHighlightView() -> NSView {
            let view = NSView()
            view.wantsLayer = true
            view.layer?.cornerRadius = 2
            view.isHidden = true
            return view
        }

        init(state: EditorTabState, editorSettings: EditorSettings) {
            self.state = state
            self.editorSettings = editorSettings
            self.lastWordWrap = editorSettings.wordWrap
            self.tabSize = editorSettings.tabSize
            super.init()
        }

        func setScrollObserver(for scrollView: NSScrollView, onLineLayoutChange: @escaping ([LineLayoutInfo]) -> Void) {
            self.onLineLayoutChange = onLineLayoutChange

            guard observedContentView !== scrollView.contentView else {
                reportLineLayouts()
                return
            }

            removeScrollObserver()
            observedContentView = scrollView.contentView
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(handleScrollBoundsChange),
                name: NSView.boundsDidChangeNotification,
                object: scrollView.contentView
            )

            reportLineLayouts()
        }

        private func removeScrollObserver() {
            NotificationCenter.default.removeObserver(
                self,
                name: NSView.boundsDidChangeNotification,
                object: observedContentView
            )
            observedContentView = nil
        }

        private func setupLineHighlight() {
            guard let textView else { return }
            lineHighlightView.removeFromSuperview()
            textView.addSubview(lineHighlightView, positioned: .below, relativeTo: nil)
            for view in bracketHighlightViews {
                view.removeFromSuperview()
                textView.addSubview(view, positioned: .below, relativeTo: nil)
            }
            updateLineHighlight()
            updateBracketMatching()
        }

        func updateLineHighlight() {
            guard let textView, let layoutManager = textView.layoutManager,
                  let textContainer = textView.textContainer
            else { return }

            let highlightColor = GhosttyService.shared.foregroundColor.withAlphaComponent(0.06)
            lineHighlightView.layer?.backgroundColor = highlightColor.cgColor

            let content = textView.string as NSString
            let selectedRange = textView.selectedRange()
            guard content.length > 0, selectedRange.location <= content.length else {
                lineHighlightView.frame = .zero
                return
            }

            let lineRange = content.lineRange(for: NSRange(location: selectedRange.location, length: 0))
            let glyphRange = layoutManager.glyphRange(forCharacterRange: lineRange, actualCharacterRange: nil)
            var lineRect = Self.lineFragmentRect(
                for: glyphRange,
                layoutManager: layoutManager,
                textContainer: textContainer
            )
            lineRect.origin.x = 0
            lineRect.origin.y += textView.textContainerOrigin.y
            lineRect.size.width = max(textView.bounds.width, textView.enclosingScrollView?.contentSize.width ?? 0)

            lineHighlightView.frame = lineRect
        }

        static func lineFragmentRect(
            for glyphRange: NSRange,
            layoutManager: NSLayoutManager,
            textContainer: NSTextContainer
        ) -> CGRect {
            guard glyphRange.length > 0 else {
                return layoutManager.boundingRect(forGlyphRange: glyphRange, in: textContainer)
            }
            var effectiveRange = NSRange(location: 0, length: 0)
            var rect = layoutManager.lineFragmentRect(
                forGlyphAt: glyphRange.location,
                effectiveRange: &effectiveRange
            )
            var nextGlyph = NSMaxRange(effectiveRange)
            while nextGlyph < NSMaxRange(glyphRange) {
                let fragment = layoutManager.lineFragmentRect(
                    forGlyphAt: nextGlyph,
                    effectiveRange: &effectiveRange
                )
                rect = rect.union(fragment)
                nextGlyph = NSMaxRange(effectiveRange)
            }
            return rect
        }

        private func observeTextViewFrame() {
            guard let textView, observedTextView !== textView else { return }
            observedTextView = textView
            textView.postsFrameChangedNotifications = true
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(handleFrameChange),
                name: NSView.frameDidChangeNotification,
                object: textView
            )
        }

        @objc
        private func handleScrollBoundsChange() {
            reportLineLayouts()
            highlightVisibleRange()
        }

        @objc
        private func handleFrameChange() {
            DispatchQueue.main.async { [weak self] in
                self?.reportLineLayouts()
            }
        }

        func invalidateAndReportLayouts() {
            lastReportedLayouts = []
            guard let textView, let layoutManager = textView.layoutManager,
                  let container = textView.textContainer
            else { return }
            layoutManager.ensureLayout(for: container)
            DispatchQueue.main.async { [weak self] in
                self?.reportLineLayouts()
            }
        }

        func reportLineLayouts() {
            guard let textView, let layoutManager = textView.layoutManager,
                  let textContainer = textView.textContainer,
                  let scrollView = textView.enclosingScrollView
            else { return }

            let visibleRect = scrollView.contentView.bounds
            let containerOriginY = textView.textContainerOrigin.y
            let content = textView.string as NSString
            guard content.length > 0 else {
                let info = [LineLayoutInfo(lineNumber: 1, yOffset: containerOriginY - visibleRect.origin.y, height: 16)]
                guard info != lastReportedLayouts else { return }
                lastReportedLayouts = info
                onLineLayoutChange(info)
                return
            }

            let visibleGlyphRange = layoutManager.glyphRange(forBoundingRect: visibleRect, in: textContainer)
            let visibleCharRange = layoutManager.characterRange(forGlyphRange: visibleGlyphRange, actualGlyphRange: nil)

            var lineNumber = lineNumber(atCharacterLocation: visibleCharRange.location)

            var layouts: [LineLayoutInfo] = []
            var index = visibleCharRange.location
            while index <= NSMaxRange(visibleCharRange), index < content.length {
                let lineRange = content.lineRange(for: NSRange(location: index, length: 0))
                let glyphRange = layoutManager.glyphRange(forCharacterRange: lineRange, actualCharacterRange: nil)
                let lineRect = Self.lineFragmentRect(
                    for: glyphRange,
                    layoutManager: layoutManager,
                    textContainer: textContainer
                )

                layouts.append(LineLayoutInfo(
                    lineNumber: lineNumber,
                    yOffset: lineRect.origin.y + containerOriginY - visibleRect.origin.y,
                    height: lineRect.height
                ))

                lineNumber += 1
                let nextIndex = NSMaxRange(lineRange)
                if nextIndex <= index { break }
                index = nextIndex
            }

            guard layouts != lastReportedLayouts else { return }
            lastReportedLayouts = layouts
            onLineLayoutChange(layouts)
        }

        func textDidChange(_: Notification) {
            guard let textView, !isUpdating else { return }
            isUpdating = true
            let canApplyIncrementalEdit = hasPendingEdit
                && !hasMultiplePendingEdits
                && applyPendingLineStartOffsetEdit()
            if !canApplyIncrementalEdit {
                rebuildLineStartOffsets(using: textView.string as NSString)
            }
            resetPendingEditState()
            state.markModified()
            isUpdating = false
        }

        func textView(
            _ textView: NSTextView,
            shouldChangeTextIn affectedCharRange: NSRange,
            replacementString: String?
        ) -> Bool {
            guard !isUpdating else { return true }
            if hasPendingEdit {
                hasMultiplePendingEdits = true
                pendingEditRange = nil
                pendingEditReplacement = ""
                return true
            }
            let textLength = (textView.string as NSString).length
            if isValidEditRange(affectedCharRange, textLength: textLength) {
                pendingEditRange = affectedCharRange
                pendingEditReplacement = replacementString ?? ""
                hasPendingEdit = true
            } else {
                pendingEditRange = nil
                pendingEditReplacement = ""
                hasPendingEdit = true
                hasMultiplePendingEdits = true
            }
            return true
        }

        private func resetPendingEditState() {
            pendingEditRange = nil
            pendingEditReplacement = ""
            hasPendingEdit = false
            hasMultiplePendingEdits = false
        }

        func textViewDidChangeSelection(_: Notification) {
            guard let textView else { return }
            let range = textView.selectedRange()
            let content = textView.string as NSString
            let loc = min(range.location, content.length)
            let line = lineNumber(atCharacterLocation: loc)
            state.cursorLine = line
            let lineStart = lineStartOffsets[max(0, min(line - 1, lineStartOffsets.count - 1))]
            state.cursorColumn = max(1, loc - lineStart + 1)
            updateCurrentSelection(in: textView, range: range)
            updateLineHighlight()
            updateBracketMatching()
        }

        private func updateCurrentSelection(in textView: NSTextView, range: NSRange) {
            guard range.length > 0, range.length <= 200 else {
                state.currentSelection = ""
                return
            }
            let nsContent = textView.string as NSString
            guard NSMaxRange(range) <= nsContent.length else {
                state.currentSelection = ""
                return
            }
            let selected = nsContent.substring(with: range)
            if selected.contains("\n") {
                state.currentSelection = ""
                return
            }
            state.currentSelection = selected
        }

        func updateBracketMatching() {
            hideBracketHighlights()
            guard let textView else { return }
            let selectedRange = textView.selectedRange()
            guard selectedRange.length == 0 else { return }

            let content = textView.string as NSString
            let length = content.length
            guard length > 0 else { return }
            guard selectedRange.location != NSNotFound, selectedRange.location <= length else { return }

            let cursor = selectedRange.location
            guard let match = findBracketMatch(in: content, cursor: cursor) else { return }

            highlightBracket(at: match.first, view: bracketHighlightViews[0])
            highlightBracket(at: match.second, view: bracketHighlightViews[1])
        }

        private func hideBracketHighlights() {
            for view in bracketHighlightViews {
                view.isHidden = true
            }
        }

        private func highlightBracket(at location: Int, view: NSView) {
            guard let textView, let layoutManager = textView.layoutManager,
                  let textContainer = textView.textContainer
            else { return }

            let charRange = NSRange(location: location, length: 1)
            let glyphRange = layoutManager.glyphRange(forCharacterRange: charRange, actualCharacterRange: nil)
            var rect = layoutManager.boundingRect(forGlyphRange: glyphRange, in: textContainer)
            rect.origin.y += textView.textContainerOrigin.y
            rect.origin.x += textView.textContainerOrigin.x

            let color = GhosttyService.shared.foregroundColor.withAlphaComponent(0.25)
            view.layer?.backgroundColor = color.cgColor
            view.frame = rect
            view.isHidden = false
        }

        private struct BracketMatch {
            let first: Int
            let second: Int
        }

        private func findBracketMatch(in content: NSString, cursor: Int) -> BracketMatch? {
            let length = content.length

            if cursor < length {
                let char = character(at: cursor, in: content)
                if let match = findMatchingBracket(for: char, at: cursor, in: content) {
                    return BracketMatch(first: cursor, second: match)
                }
            }

            if cursor > 0 {
                let prev = cursor - 1
                let char = character(at: prev, in: content)
                if let match = findMatchingBracket(for: char, at: prev, in: content) {
                    return BracketMatch(first: prev, second: match)
                }
            }

            return nil
        }

        private func findMatchingBracket(for char: Character, at location: Int, in content: NSString) -> Int? {
            let openers: [Character: Character] = ["(": ")", "[": "]", "{": "}"]
            let closers: [Character: Character] = [")": "(", "]": "[", "}": "{"]

            if let match = openers[char] {
                return scanForward(from: location + 1, open: char, close: match, in: content)
            }
            if let match = closers[char] {
                return scanBackward(from: location - 1, open: match, close: char, in: content)
            }
            return nil
        }

        private static let bracketScanLimit = 5000

        private func scanForward(from start: Int, open: Character, close: Character, in content: NSString) -> Int? {
            let length = content.length
            let end = min(length, start + Coordinator.bracketScanLimit)
            var depth = 1
            var state = BracketScanState()
            var index = start
            while index < end {
                let ch = character(at: index, in: content)
                let next = index + 1 < length ? character(at: index + 1, in: content) : nil
                state.advance(current: ch, next: next)
                if state.isInSkipRegion {
                    index += 1
                    continue
                }
                if ch == open {
                    depth += 1
                } else if ch == close {
                    depth -= 1
                    if depth == 0 { return index }
                }
                index += 1
            }
            return nil
        }

        private func scanBackward(from start: Int, open: Character, close: Character, in content: NSString) -> Int? {
            guard start >= 0 else { return nil }
            let scanStart = max(0, start - Coordinator.bracketScanLimit)

            var skipMask: [Bool] = []
            skipMask.reserveCapacity(start - scanStart + 1)
            var state = BracketScanState()
            var i = scanStart
            while i <= start {
                let ch = character(at: i, in: content)
                let next = i + 1 < content.length ? character(at: i + 1, in: content) : nil
                state.advance(current: ch, next: next)
                skipMask.append(state.isInSkipRegion)
                i += 1
            }

            var depth = 1
            var index = start
            while index >= scanStart {
                let maskIndex = index - scanStart
                if skipMask[maskIndex] {
                    index -= 1
                    continue
                }
                let ch = character(at: index, in: content)
                if ch == close {
                    depth += 1
                } else if ch == open {
                    depth -= 1
                    if depth == 0 { return index }
                }
                index -= 1
            }
            return nil
        }

        private func character(at index: Int, in content: NSString) -> Character {
            guard let scalar = UnicodeScalar(content.character(at: index)) else {
                return "\u{FFFD}"
            }
            return Character(scalar)
        }

        func applyHighlighting() {
            guard let textView, let storage = textView.textStorage else { return }
            guard storage.length > 0 else { return }
            let scrollPos = textView.enclosingScrollView?.contentView.bounds.origin
            let fullRange = NSRange(location: 0, length: storage.length)
            let font = editorSettings.resolvedFont
            textView.undoManager?.disableUndoRegistration()
            storage.beginEditing()
            storage.addAttribute(.font, value: font, range: fullRange)
            storage.addAttribute(.foregroundColor, value: GhosttyService.shared.foregroundColor, range: fullRange)
            SyntaxHighlightExtension(fileExtension: state.fileExtension)
                .applyTextAttributes(to: storage, fullRange: fullRange)
            storage.endEditing()
            textView.undoManager?.enableUndoRegistration()
            if let scrollPos {
                textView.enclosingScrollView?.contentView.setBoundsOrigin(scrollPos)
            }
            textView.needsDisplay = true
            lastHighlightedRange = fullRange
        }

        func highlightVisibleRange(force: Bool = false) {
            guard let textView, let storage = textView.textStorage,
                  let layoutManager = textView.layoutManager,
                  let textContainer = textView.textContainer,
                  let scrollView = textView.enclosingScrollView
            else { return }
            guard storage.length > 0 else {
                lastHighlightedRange = NSRange(location: 0, length: 0)
                return
            }

            let visibleRect = scrollView.contentView.bounds
            let visibleGlyphRange = layoutManager.glyphRange(forBoundingRect: visibleRect, in: textContainer)
            let visibleCharRange = layoutManager.characterRange(forGlyphRange: visibleGlyphRange, actualGlyphRange: nil)
            guard visibleCharRange.location != NSNotFound else {
                lastHighlightedRange = NSRange(location: 0, length: 0)
                return
            }

            let total = storage.length
            let bufferStart = max(0, visibleCharRange.location - Coordinator.highlightBuffer)
            let bufferEnd = min(total, NSMaxRange(visibleCharRange) + Coordinator.highlightBuffer)

            let nsContent = storage.string as NSString
            let expandedStart = nsContent.lineRange(for: NSRange(location: bufferStart, length: 0)).location
            let expandedEndRange = bufferEnd >= total
                ? NSRange(location: total, length: 0)
                : nsContent.lineRange(for: NSRange(location: bufferEnd, length: 0))
            let expandedEnd = min(total, NSMaxRange(expandedEndRange))

            let range = NSRange(location: expandedStart, length: expandedEnd - expandedStart)
            guard range.length > 0 else { return }

            if !force, rangeContains(lastHighlightedRange, other: range) {
                return
            }

            let font = editorSettings.resolvedFont
            textView.undoManager?.disableUndoRegistration()
            storage.beginEditing()
            storage.addAttribute(.font, value: font, range: range)
            storage.addAttribute(.foregroundColor, value: GhosttyService.shared.foregroundColor, range: range)
            SyntaxHighlightExtension(fileExtension: state.fileExtension)
                .applyTextAttributes(to: storage, fullRange: range)
            storage.endEditing()
            textView.undoManager?.enableUndoRegistration()
            textView.needsDisplay = true

            lastHighlightedRange = unionRange(lastHighlightedRange, range)
        }

        func resetHighlightedRange() {
            lastHighlightedRange = NSRange(location: 0, length: 0)
        }

        private func rangeContains(_ outer: NSRange, other: NSRange) -> Bool {
            guard outer.length > 0 else { return false }
            return outer.location <= other.location && NSMaxRange(outer) >= NSMaxRange(other)
        }

        private func unionRange(_ a: NSRange, _ b: NSRange) -> NSRange {
            guard a.length > 0 else { return b }
            let start = min(a.location, b.location)
            let end = max(NSMaxRange(a), NSMaxRange(b))
            return NSRange(location: start, length: end - start)
        }

        private var searchMatches: [NSRange] = []

        func performSearch(_ needle: String, caseSensitive: Bool, useRegex: Bool) {
            guard let textView else { return }
            searchMatches = []
            state.searchInvalidRegex = false
            guard !needle.isEmpty else {
                state.searchMatchCount = 0
                state.searchCurrentIndex = 0
                return
            }
            let content = textView.string as NSString
            let fullRange = NSRange(location: 0, length: content.length)

            if useRegex {
                var options: NSRegularExpression.Options = []
                if !caseSensitive { options.insert(.caseInsensitive) }
                do {
                    let regex = try NSRegularExpression(pattern: needle, options: options)
                    regex.enumerateMatches(in: textView.string, options: [], range: fullRange) { match, _, _ in
                        guard let match, match.range.length > 0 else { return }
                        searchMatches.append(match.range)
                    }
                } catch {
                    state.searchInvalidRegex = true
                    state.searchMatchCount = 0
                    state.searchCurrentIndex = 0
                    return
                }
            } else {
                var options: NSString.CompareOptions = []
                if !caseSensitive { options.insert(.caseInsensitive) }
                var searchRange = fullRange
                while searchRange.location < content.length {
                    let found = content.range(of: needle, options: options, range: searchRange)
                    guard found.location != NSNotFound else { break }
                    searchMatches.append(found)
                    searchRange.location = found.location + found.length
                    searchRange.length = content.length - searchRange.location
                }
            }

            state.searchMatchCount = searchMatches.count
            if !searchMatches.isEmpty {
                state.searchCurrentIndex = 1
                selectMatch(at: 0)
            } else {
                state.searchCurrentIndex = 0
            }
        }

        func navigateSearch(forward: Bool) {
            guard !searchMatches.isEmpty else { return }
            var idx = state.searchCurrentIndex - 1
            if forward {
                idx = (idx + 1) % searchMatches.count
            } else {
                idx = (idx - 1 + searchMatches.count) % searchMatches.count
            }
            state.searchCurrentIndex = idx + 1
            selectMatch(at: idx)
        }

        private func selectMatch(at index: Int) {
            guard let textView, index >= 0, index < searchMatches.count else { return }
            let range = searchMatches[index]
            textView.setSelectedRange(range)
            textView.scrollRangeToVisible(range)
        }

        func replaceCurrent(with replacement: String, needle: String, caseSensitive: Bool, useRegex: Bool) {
            guard let textView, !needle.isEmpty, !searchMatches.isEmpty else { return }
            let currentIndex = max(0, state.searchCurrentIndex - 1)
            guard currentIndex < searchMatches.count else { return }
            let range = searchMatches[currentIndex]
            guard textView.shouldChangeText(in: range, replacementString: replacement) else { return }

            let expanded = expandReplacement(
                template: replacement,
                matchedRange: range,
                needle: needle,
                caseSensitive: caseSensitive,
                useRegex: useRegex
            )
            textView.insertText(expanded, replacementRange: range)
            rebuildLineStartOffsets(using: textView.string as NSString)
            state.markModified()

            performSearch(needle, caseSensitive: caseSensitive, useRegex: useRegex)

            guard !searchMatches.isEmpty else { return }
            let nextLocation = range.location + (expanded as NSString).length
            let nextIndex = searchMatches.firstIndex(where: { $0.location >= nextLocation }) ?? 0
            state.searchCurrentIndex = nextIndex + 1
            selectMatch(at: nextIndex)
        }

        func replaceAll(with replacement: String, needle: String, caseSensitive: Bool, useRegex: Bool) {
            guard let textView, !needle.isEmpty, !searchMatches.isEmpty else { return }
            let matches = searchMatches
            textView.breakUndoCoalescing()
            for range in matches.reversed() {
                guard textView.shouldChangeText(in: range, replacementString: replacement) else { continue }
                let expanded = expandReplacement(
                    template: replacement,
                    matchedRange: range,
                    needle: needle,
                    caseSensitive: caseSensitive,
                    useRegex: useRegex
                )
                textView.insertText(expanded, replacementRange: range)
            }
            rebuildLineStartOffsets(using: textView.string as NSString)
            state.markModified()
            performSearch(needle, caseSensitive: caseSensitive, useRegex: useRegex)
        }

        func rebuildLineStartOffsets(using content: NSString) {
            var offsets = [0]
            offsets.reserveCapacity(max(1, content.length / 40))
            let newline = "\n"
            var searchRange = NSRange(location: 0, length: content.length)

            while searchRange.location < content.length {
                let found = content.range(of: newline, options: [], range: searchRange)
                guard found.location != NSNotFound else { break }
                let nextLineStart = found.location + found.length
                if nextLineStart <= content.length {
                    offsets.append(nextLineStart)
                }
                searchRange.location = nextLineStart
                searchRange.length = content.length - searchRange.location
            }

            lineStartOffsets = offsets
        }

        private func applyPendingLineStartOffsetEdit() -> Bool {
            guard let editRange = pendingEditRange, !lineStartOffsets.isEmpty,
                  let textView
            else { return false }
            let textLength = (textView.string as NSString).length
            guard isValidEditRange(editRange, textLength: textLength) else { return false }

            let replacement = pendingEditReplacement as NSString
            let replacementLength = replacement.length
            let oldEnd = NSMaxRange(editRange)
            let delta = replacementLength - editRange.length

            let removeStart = firstLineStartIndex(after: editRange.location)
            let removeEnd = firstLineStartIndex(after: oldEnd)

            var insertedStarts: [Int] = []
            var searchRange = NSRange(location: 0, length: replacementLength)
            while searchRange.location < replacementLength {
                let found = replacement.range(of: "\n", options: [], range: searchRange)
                guard found.location != NSNotFound else { break }
                insertedStarts.append(editRange.location + found.location + found.length)
                searchRange.location = found.location + found.length
                searchRange.length = replacementLength - searchRange.location
            }

            var updated: [Int] = []
            updated.reserveCapacity(lineStartOffsets.count + insertedStarts.count)
            updated.append(contentsOf: lineStartOffsets[0 ..< removeStart])
            updated.append(contentsOf: insertedStarts)
            if removeEnd < lineStartOffsets.count {
                for index in removeEnd ..< lineStartOffsets.count {
                    let shifted = lineStartOffsets[index] + delta
                    guard shifted >= 0 else { return false }
                    updated.append(shifted)
                }
            }
            if updated.isEmpty || updated[0] != 0 {
                updated.insert(0, at: 0)
            }

            lineStartOffsets = updated
            return true
        }

        private func isValidEditRange(_ range: NSRange, textLength: Int) -> Bool {
            guard range.location != NSNotFound else { return false }
            guard range.location >= 0, range.length >= 0 else { return false }
            guard range.location <= textLength else { return false }
            guard range.length <= textLength - range.location else { return false }
            return true
        }

        private func firstLineStartIndex(after location: Int) -> Int {
            var low = 0
            var high = lineStartOffsets.count
            while low < high {
                let mid = (low + high) / 2
                if lineStartOffsets[mid] <= location {
                    low = mid + 1
                } else {
                    high = mid
                }
            }
            return low
        }

        private func lineNumber(atCharacterLocation location: Int) -> Int {
            guard !lineStartOffsets.isEmpty else { return 1 }
            var low = 0
            var high = lineStartOffsets.count - 1
            var result = 0

            while low <= high {
                let mid = (low + high) / 2
                if lineStartOffsets[mid] <= location {
                    result = mid
                    low = mid + 1
                    continue
                }
                if mid == 0 { break }
                high = mid - 1
            }

            return result + 1
        }

        func appendChunkToTextView(_ chunk: String, textView: NSTextView) {
            guard !chunk.isEmpty, let textStorage = textView.textStorage else { return }
            let baseOffset = textStorage.length

            textStorage.beginEditing()
            textStorage.append(NSAttributedString(string: chunk, attributes: [
                .font: editorSettings.resolvedFont,
                .foregroundColor: GhosttyService.shared.foregroundColor,
            ]))
            textStorage.endEditing()

            appendLineStartOffsets(for: chunk, baseOffset: baseOffset)
        }

        private func appendLineStartOffsets(for chunk: String, baseOffset: Int) {
            let nsChunk = chunk as NSString
            var searchRange = NSRange(location: 0, length: nsChunk.length)

            while searchRange.location < nsChunk.length {
                let found = nsChunk.range(of: "\n", options: [], range: searchRange)
                guard found.location != NSNotFound else { break }
                let nextLineStart = baseOffset + found.location + found.length
                lineStartOffsets.append(nextLineStart)
                searchRange.location = found.location + found.length
                searchRange.length = nsChunk.length - searchRange.location
            }
        }

        private func expandReplacement(
            template: String,
            matchedRange: NSRange,
            needle: String,
            caseSensitive: Bool,
            useRegex: Bool
        ) -> String {
            guard useRegex, let textView else { return template }
            var options: NSRegularExpression.Options = []
            if !caseSensitive { options.insert(.caseInsensitive) }
            guard let regex = try? NSRegularExpression(pattern: needle, options: options) else { return template }
            let source = textView.string as NSString
            let matchedText = source.substring(with: matchedRange)
            let scoped = NSRange(location: 0, length: (matchedText as NSString).length)
            guard let match = regex.firstMatch(in: matchedText, options: [], range: scoped) else { return template }
            return regex.replacementString(for: match, in: matchedText, offset: 0, template: template)
        }

        @objc
        func handleReturn(_ textView: NSTextView) -> Bool {
            textView.breakUndoCoalescing()
            let content = textView.string as NSString
            let range = textView.selectedRange()
            let loc = min(range.location, content.length)
            let lineRange = content.lineRange(for: NSRange(location: loc, length: 0))
            let linePrefixRange = NSRange(location: lineRange.location, length: loc - lineRange.location)
            let lineText = content.substring(with: linePrefixRange)
            let leading = String(lineText.prefix(while: { $0 == " " || $0 == "\t" }))
            let trimmed = lineText.trimmingCharacters(in: .whitespaces)
            let extra = trimmed.hasSuffix("{") || trimmed.hasSuffix("(")
                || trimmed.hasSuffix("[")
            let indentUnit = String(repeating: " ", count: tabSize)
            let indent = extra ? leading + indentUnit : leading
            textView.insertText("\n" + indent, replacementRange: range)
            return true
        }

        func textView(_: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            guard let textView else { return false }
            if commandSelector == #selector(NSResponder.cancelOperation(_:)), state.searchVisible {
                state.searchVisible = false
                return true
            }
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                return handleReturn(textView)
            }
            if commandSelector == #selector(NSResponder.deleteWordBackward(_:)) {
                return handleDeleteWordBackward(textView)
            }
            return false
        }

        private func handleDeleteWordBackward(_ textView: NSTextView) -> Bool {
            let content = textView.string
            let range = textView.selectedRange()
            guard range.location != NSNotFound, range.location > 0 else { return false }
            textView.breakUndoCoalescing()

            let nsContent = content as NSString
            let cursorPos = range.location
            let charBefore = nsContent.character(at: cursorPos - 1)

            if charBefore == 0x0A {
                let deleteRange = NSRange(location: cursorPos - 1, length: 1)
                textView.insertText("", replacementRange: deleteRange)
                return true
            }

            let scalar = Unicode.Scalar(charBefore)
            if let scalar, CharacterSet.punctuationCharacters.union(.symbols).contains(scalar) {
                let deleteRange = NSRange(location: cursorPos - 1, length: 1)
                textView.insertText("", replacementRange: deleteRange)
                return true
            }

            let lineRange = nsContent.lineRange(for: NSRange(location: cursorPos, length: 0))
            let lineStart = lineRange.location
            let textBeforeCursor = nsContent.substring(with: NSRange(location: lineStart, length: cursorPos - lineStart))

            if textBeforeCursor.allSatisfy({ $0 == " " || $0 == "\t" }) {
                let deleteRange = NSRange(location: lineStart, length: cursorPos - lineStart)
                textView.insertText("", replacementRange: deleteRange)
                return true
            }

            return false
        }
    }
}

private struct BracketScanState {
    private var inSingleQuote = false
    private var inDoubleQuote = false
    private var inLineComment = false
    private var inBlockComment = false
    private var escaped = false
    private var pendingBlockCommentExit = false

    var isInSkipRegion: Bool {
        inSingleQuote || inDoubleQuote || inLineComment || inBlockComment
    }

    mutating func advance(current: Character, next: Character?) {
        if inBlockComment {
            if pendingBlockCommentExit {
                pendingBlockCommentExit = false
                inBlockComment = false
                return
            }
            if current == "*", next == "/" {
                pendingBlockCommentExit = true
            }
            return
        }
        if inLineComment {
            if current == "\n" { inLineComment = false }
            return
        }
        if escaped {
            escaped = false
            return
        }
        if inSingleQuote {
            if current == "\\" { escaped = true
                return
            }
            if current == "'" { inSingleQuote = false }
            return
        }
        if inDoubleQuote {
            if current == "\\" { escaped = true
                return
            }
            if current == "\"" { inDoubleQuote = false }
            return
        }
        if current == "/", next == "/" {
            inLineComment = true
            return
        }
        if current == "/", next == "*" {
            inBlockComment = true
            return
        }
        if current == "\"" {
            inDoubleQuote = true
            return
        }
        if current == "'" {
            inSingleQuote = true
            return
        }
    }
}
