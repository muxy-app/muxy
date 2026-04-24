import AppKit
import os
import SwiftUI

private final class CodeEditorTextView: NSTextView {
    private static let undoActionSelector = #selector(CodeEditorTextView.undo(_:))
    private static let redoActionSelector = #selector(CodeEditorTextView.redo(_:))

    var onUndoRequest: (() -> Bool)?
    var onRedoRequest: (() -> Bool)?
    var canUndoRequest: (() -> Bool)?
    var canRedoRequest: (() -> Bool)?

    override func paste(_ sender: Any?) {
        pasteAsPlainText(sender)
    }

    @objc
    func undo(_ sender: Any?) {
        if onUndoRequest?() == true {
            return
        }
        undoManager?.undo()
    }

    @objc
    func redo(_ sender: Any?) {
        if onRedoRequest?() == true {
            return
        }
        undoManager?.redo()
    }

    override func validateUserInterfaceItem(_ item: any NSValidatedUserInterfaceItem) -> Bool {
        if item.action == Self.undoActionSelector, let canUndoRequest {
            return canUndoRequest()
        }
        if item.action == Self.redoActionSelector, let canRedoRequest {
            return canRedoRequest()
        }
        return super.validateUserInterfaceItem(item)
    }

    override func scrollRangeToVisible(_ range: NSRange) {
        guard let layoutManager, let textContainer, let scrollView = enclosingScrollView else {
            super.scrollRangeToVisible(range)
            return
        }

        let glyphRange = layoutManager.glyphRange(forCharacterRange: range, actualCharacterRange: nil)
        var rect = layoutManager.boundingRect(forGlyphRange: glyphRange, in: textContainer)
        rect.origin.y += textContainerOrigin.y
        rect.origin.x += textContainerOrigin.x
        if let documentView = scrollView.documentView {
            rect = convert(rect, to: documentView)
        }

        let clipBounds = scrollView.contentView.bounds
        let visibleMinX = clipBounds.origin.x
        let visibleMaxX = visibleMinX + clipBounds.width
        let visibleMinY = clipBounds.origin.y
        let visibleMaxY = visibleMinY + clipBounds.height

        let cursorMinX = rect.origin.x
        let cursorMaxX = rect.origin.x + max(rect.width, 2)
        let cursorMinY = rect.origin.y
        let cursorMaxY = rect.origin.y + rect.height

        let maxScrollX: CGFloat = if let documentView = scrollView.documentView {
            max(0, documentView.bounds.width - clipBounds.width)
        } else {
            0
        }

        let maxScrollY: CGFloat = if let documentView = scrollView.documentView {
            max(0, documentView.bounds.height - clipBounds.height)
        } else {
            0
        }

        var newOrigin = clipBounds.origin

        if cursorMaxX > visibleMaxX {
            newOrigin.x = min(maxScrollX, max(0, cursorMaxX - clipBounds.width))
        } else if cursorMinX < visibleMinX {
            newOrigin.x = min(maxScrollX, max(0, cursorMinX))
        }

        if cursorMaxY > visibleMaxY {
            newOrigin.y = min(maxScrollY, max(0, cursorMaxY - clipBounds.height))
        } else if cursorMinY < visibleMinY {
            newOrigin.y = min(maxScrollY, max(0, cursorMinY))
        }

        if newOrigin != clipBounds.origin {
            scrollView.contentView.setBoundsOrigin(newOrigin)
            scrollView.reflectScrolledClipView(scrollView.contentView)
        }
    }
}

private final class CodeEditorLayoutManager: NSLayoutManager {
    override func setGlyphs(
        _ glyphs: UnsafePointer<CGGlyph>,
        properties props: UnsafePointer<NSLayoutManager.GlyphProperty>,
        characterIndexes charIndexes: UnsafePointer<Int>,
        font aFont: NSFont,
        forGlyphRange glyphRange: NSRange
    ) {
        guard aFont.isFixedPitch else {
            super.setGlyphs(glyphs, properties: props, characterIndexes: charIndexes, font: aFont, forGlyphRange: glyphRange)
            return
        }
        let mutableProps = UnsafeMutablePointer(mutating: props)
        for index in 0 ..< glyphRange.length {
            mutableProps[index].subtract(.elastic)
        }
        super.setGlyphs(glyphs, properties: mutableProps, characterIndexes: charIndexes, font: aFont, forGlyphRange: glyphRange)
    }
}

final class ViewportContainerView: NSView {
    override var isFlipped: Bool { true }

    override func mouseDown(with event: NSEvent) {
        guard let textView = subviews.first as? NSTextView else {
            super.mouseDown(with: event)
            return
        }

        let pointInContainer = convert(event.locationInWindow, from: nil)
        if textView.frame.contains(pointInContainer) {
            super.mouseDown(with: event)
            return
        }

        let clampedX = min(pointInContainer.x, textView.frame.maxX - 1)
        let clampedY = min(max(pointInContainer.y, textView.frame.minY), textView.frame.maxY - 1)
        let pointInTextView = NSPoint(x: clampedX, y: clampedY - textView.frame.origin.y)
        let charIndex = textView.characterIndexForInsertion(at: pointInTextView)

        textView.window?.makeFirstResponder(textView)

        guard event.modifierFlags.contains(.shift) else {
            textView.setSelectedRange(NSRange(location: charIndex, length: 0))
            return
        }

        let current = textView.selectedRange()
        let anchor = current.location
        let newRange = if charIndex >= anchor {
            NSRange(location: anchor, length: charIndex - anchor)
        } else {
            NSRange(location: charIndex, length: anchor - charIndex)
        }
        textView.setSelectedRange(newRange)
    }
}

struct CodeEditorView: NSViewRepresentable {
    @Bindable var state: EditorTabState
    let editorSettings: EditorSettings
    let themeVersion: Int
    let searchNeedle: String
    let searchNavigationVersion: Int
    let searchNavigationDirection: EditorSearchNavigationDirection
    let searchCaseSensitive: Bool
    let searchUseRegex: Bool
    let replaceText: String
    let replaceVersion: Int
    let replaceAllVersion: Int
    let editorFocusVersion: Int

    func makeCoordinator() -> Coordinator {
        Coordinator(state: state, editorSettings: editorSettings)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autoresizingMask = [.width, .height]

        let textStorage = NSTextStorage()
        let layoutManager = CodeEditorLayoutManager()
        textStorage.addLayoutManager(layoutManager)

        let textContainer = NSTextContainer(containerSize: NSSize(
            width: CGFloat.greatestFiniteMagnitude,
            height: CGFloat.greatestFiniteMagnitude
        ))
        textContainer.widthTracksTextView = false
        textContainer.lineFragmentPadding = 8
        layoutManager.addTextContainer(textContainer)

        let textView = CodeEditorTextView(frame: NSRect(origin: .zero, size: scrollView.contentSize), textContainer: textContainer)
        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = true
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

        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        scrollView.contentView.postsBoundsChangedNotifications = true
        scrollView.contentView.postsFrameChangedNotifications = true

        let coordinator = context.coordinator
        textView.delegate = coordinator
        coordinator.textView = textView
        coordinator.scrollView = scrollView
        textView.onUndoRequest = { [weak coordinator] in
            coordinator?.performUndoRequest() ?? false
        }
        textView.onRedoRequest = { [weak coordinator] in
            coordinator?.performRedoRequest() ?? false
        }
        textView.canUndoRequest = { [weak coordinator] in
            coordinator?.canPerformUndoRequest() ?? false
        }
        textView.canRedoRequest = { [weak coordinator] in
            coordinator?.canPerformRedoRequest() ?? false
        }
        coordinator.setScrollObserver(for: scrollView)
        textView.undoManager?.removeAllActions()

        return scrollView
    }

    static func dismantleNSView(_ scrollView: NSScrollView, coordinator: Coordinator) {
        if let textView = coordinator.textView {
            textView.undoManager?.removeAllActions()
            if let window = textView.window, window.firstResponder === textView {
                window.makeFirstResponder(nil)
            }
            if let codeTextView = textView as? CodeEditorTextView {
                codeTextView.onUndoRequest = nil
                codeTextView.onRedoRequest = nil
                codeTextView.canUndoRequest = nil
                codeTextView.canRedoRequest = nil
            }
        }
        coordinator.textView?.delegate = nil
    }

    // MARK: - updateNSView

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let textView = context.coordinator.textView else { return }
        let coordinator = context.coordinator

        if state.backingStore != nil, coordinator.viewportState == nil {
            coordinator.enterViewportMode(scrollView: scrollView)
        }

        updateNSViewViewportMode(scrollView: scrollView, textView: textView, coordinator: coordinator)
    }

    // MARK: - Viewport Mode

    private func updateNSViewViewportMode(scrollView: NSScrollView, textView: NSTextView, coordinator: Coordinator) {
        guard let viewport = coordinator.viewportState else { return }

        let backingStoreChanged = coordinator.lastSyncedBackingStoreVersion != state.backingStoreVersion
        if backingStoreChanged {
            coordinator.lastSyncedBackingStoreVersion = state.backingStoreVersion
            coordinator.invalidateRenderedViewportText()
            coordinator.clearViewportHistory()
        }

        let incrementalFinished = coordinator.wasIncrementalLoading && !state.isIncrementalLoading
        coordinator.wasIncrementalLoading = state.isIncrementalLoading

        if backingStoreChanged || incrementalFinished {
            coordinator.updateContainerHeight()
        }

        if !coordinator.hasAppliedInitialContent, viewport.backingStore.lineCount > 1 || backingStoreChanged {
            coordinator.hasAppliedInitialContent = true
            coordinator.refreshViewport(force: true)
        }

        let themeChanged = coordinator.lastThemeVersion != themeVersion
        let font = editorSettings.resolvedFont
        let fontChanged = textView.font != font

        applyThemeAndFont(textView: textView, font: font)

        if fontChanged {
            viewport.updateEstimatedLineHeight(font: font)
            coordinator.updateContainerHeight()
            coordinator.refreshViewport(force: true)
        }

        if themeChanged, !fontChanged {
            coordinator.refreshViewport(force: true)
        }

        if themeChanged {
            coordinator.applySearchHighlights(force: true)
            coordinator.lastThemeVersion = themeVersion
        }

        updateSearchViewport(coordinator: coordinator)

        if coordinator.lastEditorFocusVersion != editorFocusVersion {
            coordinator.lastEditorFocusVersion = editorFocusVersion
            coordinator.focusEditorPreservingSelection()
        }
    }

    // MARK: - Shared helpers

    private func applyThemeAndFont(textView: NSTextView, font: NSFont) {
        let fgColor = GhosttyService.shared.foregroundColor
        let bgColor = GhosttyService.shared.backgroundColor

        if textView.backgroundColor != bgColor {
            textView.backgroundColor = bgColor
        }
        if textView.insertionPointColor != fgColor {
            textView.insertionPointColor = fgColor
        }
        if textView.textColor != fgColor {
            textView.textColor = fgColor
        }

        if (textView.typingAttributes[.foregroundColor] as? NSColor) != fgColor {
            textView.typingAttributes[.foregroundColor] = fgColor
        }

        if textView.font != font {
            textView.font = font
            textView.typingAttributes[.font] = font
        }

        let selectionBackground = fgColor.withAlphaComponent(0.15)
        if let selectedBg = textView.selectedTextAttributes[.backgroundColor] as? NSColor, selectedBg != selectionBackground {
            textView.selectedTextAttributes = [
                .backgroundColor: selectionBackground,
            ]
        }
    }

    private func updateSearchViewport(coordinator: Coordinator) {
        if !state.searchVisible, coordinator.lastSearchVisible {
            coordinator.lastSearchVisible = false
            coordinator.clearSearchHighlights()
            return
        }

        let becameVisible = state.searchVisible && !coordinator.lastSearchVisible
        coordinator.lastSearchVisible = state.searchVisible

        let searchOptionsChanged = coordinator.lastSearchCaseSensitive != searchCaseSensitive
            || coordinator.lastSearchUseRegex != searchUseRegex
        if coordinator.lastSearchNeedle != searchNeedle || searchOptionsChanged || becameVisible {
            coordinator.lastSearchNeedle = searchNeedle
            coordinator.lastSearchCaseSensitive = searchCaseSensitive
            coordinator.lastSearchUseRegex = searchUseRegex
            coordinator.performSearchViewport(searchNeedle, caseSensitive: searchCaseSensitive, useRegex: searchUseRegex)
        }

        if coordinator.lastSearchNavigationVersion != searchNavigationVersion {
            coordinator.lastSearchNavigationVersion = searchNavigationVersion
            coordinator.navigateSearchViewport(forward: searchNavigationDirection == .next)
        }

        if coordinator.lastReplaceVersion != replaceVersion {
            coordinator.lastReplaceVersion = replaceVersion
            coordinator.replaceCurrentViewport(
                with: replaceText,
                needle: searchNeedle,
                caseSensitive: searchCaseSensitive,
                useRegex: searchUseRegex
            )
        }

        if coordinator.lastReplaceAllVersion != replaceAllVersion {
            coordinator.lastReplaceAllVersion = replaceAllVersion
            coordinator.replaceAllViewport(
                with: replaceText,
                needle: searchNeedle,
                caseSensitive: searchCaseSensitive,
                useRegex: searchUseRegex
            )
        }
    }

    // MARK: - Coordinator

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        private struct ViewportCursor {
            let line: Int
            let column: Int
        }

        private struct PendingViewportEdit {
            let startLine: Int
            let oldLines: [String]
            let newLines: [String]
            let selectionBefore: ViewportCursor
        }

        private struct ViewportEdit {
            let startLine: Int
            let oldLines: [String]
            let newLines: [String]
            let selectionBefore: ViewportCursor
            let selectionAfter: ViewportCursor
        }

        private struct ViewportEditGroup {
            var edits: [ViewportEdit]
        }

        let state: EditorTabState
        let editorSettings: EditorSettings
        weak var textView: NSTextView?

        weak var scrollView: NSScrollView?
        var viewportState: ViewportState?
        var containerView: ViewportContainerView?

        var isUpdating = false
        private var isEditingViewport = false
        var hasAppliedInitialContent = false
        var lastThemeVersion = -1
        var lastSearchVisible = false
        var lastSearchNeedle = ""
        var lastSearchNavigationVersion = -1
        var lastSearchCaseSensitive = false
        var lastSearchUseRegex = false
        var lastReplaceVersion = 0
        var lastReplaceAllVersion = 0
        var lastEditorFocusVersion = 0
        var lastSyncedBackingStoreVersion = -1
        var wasIncrementalLoading = false
        private static let initialViewportLineLimit = 1100
        private(set) var lineStartOffsets: [Int] = [0]
        private weak var observedContentView: NSClipView?
        private static let viewportUndoLimit = 200
        private static let viewportUndoCoalesceInterval: CFTimeInterval = 1.0
        private static let undoCommandSelector = #selector(CodeEditorTextView.undo(_:))
        private static let redoCommandSelector = #selector(CodeEditorTextView.redo(_:))
        private static let perfLogger = Logger(subsystem: "app.muxy", category: "EditorPerf")
        private static let perfEnabled: Bool = {
            if let env = ProcessInfo.processInfo.environment["MUXY_EDITOR_PERF"] {
                let value = env.lowercased()
                return value == "1" || value == "true" || value == "yes"
            }
            return UserDefaults.standard.bool(forKey: "MuxyEditorPerf")
        }()

        private var pendingViewportEdit: PendingViewportEdit?
        private var viewportUndoStack: [ViewportEditGroup] = []
        private var viewportRedoStack: [ViewportEditGroup] = []
        private var lastViewportEditTimestamp: CFTimeInterval?
        private var isApplyingViewportHistory = false
        private var needsViewportTextReload = true
        private var lastRenderedViewportRange: Range<Int>?
        private var lastRenderedBackingStoreVersion = -1
        private var lastObservedClipSize: CGSize = .zero
        private var refreshTimingCount = 0
        private var highlightTimingCount = 0
        private var lastRefreshDurationMs: Double = 0
        private var lastHighlightDurationMs: Double = 0
        private var pendingCascadeReapplyGeneration: UInt64 = 0

        init(state: EditorTabState, editorSettings: EditorSettings) {
            self.state = state
            self.editorSettings = editorSettings
            super.init()
        }

        deinit {
            NotificationCenter.default.removeObserver(self)
        }

        private func beginPerfTiming() -> CFTimeInterval? {
            guard Self.perfEnabled else { return nil }
            return CACurrentMediaTime()
        }

        func invalidateRenderedViewportText() {
            needsViewportTextReload = true
        }

        private func recordRefreshTiming(start: CFTimeInterval?, durationLineCount: Int, force: Bool) {
            guard let start else { return }
            let durationMs = (CACurrentMediaTime() - start) * 1000
            let deltaMs = durationMs - lastRefreshDurationMs
            lastRefreshDurationMs = durationMs
            refreshTimingCount += 1
            if refreshTimingCount.isMultiple(of: 24) || durationMs >= 3 {
                Self.perfLogger.debug(
                    "refresh ms \(durationMs) delta \(deltaMs) force \(force) lines \(durationLineCount)"
                )
            }
        }

        private func recordHighlightTiming(start: CFTimeInterval?, highlightedRangeCount: Int, force: Bool) {
            guard let start else { return }
            let durationMs = (CACurrentMediaTime() - start) * 1000
            let deltaMs = durationMs - lastHighlightDurationMs
            lastHighlightDurationMs = durationMs
            highlightTimingCount += 1
            if highlightTimingCount.isMultiple(of: 30) || durationMs >= 2 {
                Self.perfLogger.debug(
                    "highlight ms \(durationMs) delta \(deltaMs) force \(force) ranges \(highlightedRangeCount)"
                )
            }
        }

        // MARK: - Viewport Mode Setup

        func enterViewportMode(scrollView: NSScrollView) {
            guard let store = state.backingStore, let textView else { return }
            textView.allowsUndo = false
            textView.undoManager?.removeAllActions()
            textView.usesFindBar = false
            clearViewportHistory()

            let viewport = ViewportState(backingStore: store)
            viewport.updateEstimatedLineHeight(font: editorSettings.resolvedFont)
            viewportState = viewport
            invalidateRenderedViewportText()
            lastRenderedViewportRange = nil
            lastRenderedBackingStoreVersion = -1
            lastObservedClipSize = scrollView.contentView.bounds.size

            textView.isVerticallyResizable = false
            textView.autoresizingMask = []

            let container = ViewportContainerView()
            container.wantsLayer = true
            let height = max(viewport.totalDocumentHeight, scrollView.contentView.bounds.height)
            let width = max(scrollView.contentSize.width, textView.frame.width)
            container.frame = NSRect(x: 0, y: 0, width: width, height: height)
            container.autoresizingMask = []

            textView.removeFromSuperview()
            container.addSubview(textView)
            scrollView.documentView = container
            containerView = container

            textView.frame = NSRect(
                x: 0, y: 0,
                width: width,
                height: viewport.estimatedLineHeight * CGFloat(min(Self.initialViewportLineLimit, store.lineCount))
            )
        }

        func updateContainerHeight() {
            guard let viewport = viewportState, let container = containerView, let scrollView else { return }
            let height = max(viewport.totalDocumentHeight, scrollView.contentView.bounds.height)
            let width = max(scrollView.contentSize.width, textView?.frame.width ?? scrollView.contentSize.width)
            container.frame = NSRect(x: 0, y: 0, width: width, height: height)
        }

        func refreshViewport(force: Bool) {
            guard let viewport = viewportState, let textView, let scrollView else { return }
            let scrollY = scrollView.contentView.bounds.origin.y
            let visibleHeight = scrollView.contentView.bounds.height

            guard force || viewport.shouldUpdateViewport(scrollY: scrollY, visibleHeight: visibleHeight) else { return }

            let previousRange = viewport.viewportStartLine ..< viewport.viewportEndLine

            let savedCursor = globalCursorFromLocalLocation(textView.selectedRange().location)
            let savedSelectionLength = textView.selectedRange().length

            let newRange = viewport.computeViewport(scrollY: scrollY, visibleHeight: visibleHeight)
            if !force, newRange == previousRange {
                return
            }

            let perfStart = beginPerfTiming()
            let renderedLineCount = newRange.count
            defer {
                recordRefreshTiming(start: perfStart, durationLineCount: renderedLineCount, force: force)
            }

            viewport.applyViewport(newRange)

            let yOffset = viewport.viewportYOffset()
            let shouldReloadText = needsViewportTextReload
                || lastRenderedViewportRange != newRange
                || lastRenderedBackingStoreVersion != state.backingStoreVersion

            let text: String? = if shouldReloadText {
                viewport.viewportText()
            } else {
                nil
            }

            isUpdating = true
            CATransaction.begin()
            CATransaction.setDisableActions(true)

            if let text {
                textView.string = text
                lastRenderedViewportRange = newRange
                lastRenderedBackingStoreVersion = state.backingStoreVersion
                needsViewportTextReload = false
                rebuildLineStartOffsetsForViewport()
            }
            let font = editorSettings.resolvedFont
            if let storage = textView.textStorage, storage.length > 0 {
                let fullRange = NSRange(location: 0, length: storage.length)
                storage.beginEditing()
                storage.addAttribute(.font, value: font, range: fullRange)
                storage.endEditing()
                applySyntaxHighlights(storage: storage, viewport: viewport)
            }

            updateViewportFrames(
                viewport: viewport,
                textView: textView,
                scrollView: scrollView,
                yOffset: yOffset,
                visibleLineCount: newRange.count
            )

            CATransaction.commit()

            if let savedCursor,
               let newLocalLine = viewport.viewportLine(forBackingStoreLine: savedCursor.line)
            {
                let newCharOffset = charOffsetForLocalLine(newLocalLine)
                let newContent = textView.string as NSString
                let lineRange = newContent.lineRange(for: NSRange(location: min(newCharOffset, newContent.length), length: 0))
                let lineLength = lineRange.length - (NSMaxRange(lineRange) < newContent.length ? 1 : 0)
                let newCursor = newCharOffset + min(savedCursor.column, max(0, lineLength))
                let safeCursor = min(newCursor, newContent.length)
                textView.setSelectedRange(NSRange(location: safeCursor, length: min(savedSelectionLength, newContent.length - safeCursor)))
            }

            isUpdating = false
            applySearchHighlights()
        }

        func applySyntaxHighlights(storage: NSTextStorage, viewport: ViewportState) {
            guard let highlighter = state.syntaxHighlighter,
                  let backingStore = state.backingStore,
                  let textView,
                  let layoutManager = textView.layoutManager
            else { return }
            let storageLength = storage.length
            guard storageLength > 0 else { return }
            let range = viewport.viewportStartLine ..< viewport.viewportEndLine
            guard !range.isEmpty else { return }

            let spans = highlighter.spans(
                in: range,
                lineStartOffsets: lineStartOffsets,
                backingStore: backingStore
            )

            let fullRange = NSRange(location: 0, length: storageLength)
            layoutManager.removeTemporaryAttribute(.foregroundColor, forCharacterRange: fullRange)
            for span in spans {
                let availableLength = storageLength - span.range.location
                guard span.range.location >= 0, availableLength > 0 else { continue }
                let clampedLength = min(span.range.length, availableLength)
                guard clampedLength > 0 else { continue }
                layoutManager.addTemporaryAttribute(
                    .foregroundColor,
                    value: SyntaxTheme.color(for: span.scope),
                    forCharacterRange: NSRange(location: span.range.location, length: clampedLength)
                )
            }
        }

        func invalidateSyntaxHighlightsFromLine(_ line: Int) {
            state.syntaxHighlighter?.invalidate(fromLine: max(0, line))
        }

        func reapplySyntaxHighlights() {
            guard let textView, let storage = textView.textStorage, let viewport = viewportState else { return }
            applySyntaxHighlights(storage: storage, viewport: viewport)
        }

        func applyIncrementalSyntaxHighlights(
            startLine: Int,
            oldLineCount: Int,
            newLineCount: Int
        ) {
            guard let highlighter = state.syntaxHighlighter,
                  let backingStore = state.backingStore,
                  let viewport = viewportState,
                  let textView,
                  let storage = textView.textStorage
            else { return }

            let outcome = highlighter.applyEdit(
                startLine: startLine,
                oldLineCount: oldLineCount,
                newLineCount: newLineCount,
                backingStore: backingStore
            )

            applySyntaxAttributes(
                storage: storage,
                viewport: viewport,
                highlighter: highlighter,
                lineRange: startLine ..< startLine + newLineCount
            )

            if case .cascade = outcome {
                scheduleCascadeReapply()
            }
        }

        private func scheduleCascadeReapply() {
            pendingCascadeReapplyGeneration &+= 1
            let generation = pendingCascadeReapplyGeneration
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(16)) { [weak self] in
                guard let self, self.pendingCascadeReapplyGeneration == generation else { return }
                self.reapplySyntaxHighlights()
            }
        }

        private func applySyntaxAttributes(
            storage: NSTextStorage,
            viewport: ViewportState,
            highlighter: SyntaxHighlighter,
            lineRange: Range<Int>
        ) {
            guard let textView, let layoutManager = textView.layoutManager else { return }
            let storageLength = storage.length
            guard storageLength > 0 else { return }

            let viewportStart = viewport.viewportStartLine
            let localStart = max(0, lineRange.lowerBound - viewportStart)
            let localEndRaw = lineRange.upperBound - viewportStart
            let localEnd = min(localEndRaw, viewport.viewportLineCount)
            guard localStart < localEnd, localStart < lineStartOffsets.count else { return }

            let charStart = lineStartOffsets[localStart]
            let charEnd: Int = localEnd < lineStartOffsets.count ? lineStartOffsets[localEnd] : storageLength
            guard charEnd > charStart, charStart >= 0, charEnd <= storageLength else { return }

            layoutManager.removeTemporaryAttribute(
                .foregroundColor,
                forCharacterRange: NSRange(location: charStart, length: charEnd - charStart)
            )
            for localIndex in localStart ..< localEnd {
                let globalLine = viewportStart + localIndex
                guard let tokens = highlighter.tokens(forLine: globalLine) else { continue }
                let lineOffset = lineStartOffsets[localIndex]
                for token in tokens {
                    let location = lineOffset + token.location
                    guard location >= 0, location + token.length <= storageLength else { continue }
                    layoutManager.addTemporaryAttribute(
                        .foregroundColor,
                        value: SyntaxTheme.color(for: token.scope),
                        forCharacterRange: NSRange(location: location, length: token.length)
                    )
                }
            }
        }

        func rebuildLineStartOffsetsForViewport() {
            guard let textView else { return }
            let content = textView.string as NSString
            var offsets = [0]
            offsets.reserveCapacity(content.length / 40)
            var searchRange = NSRange(location: 0, length: content.length)
            while searchRange.location < content.length {
                let found = content.range(of: "\n", options: [], range: searchRange)
                guard found.location != NSNotFound else { break }
                let next = found.location + found.length
                if next <= content.length {
                    offsets.append(next)
                }
                searchRange.location = next
                searchRange.length = content.length - next
            }
            lineStartOffsets = offsets
        }

        private func updateLineStartOffsetsAfterEdit(
            viewportStartLine: Int,
            globalStartLine: Int,
            oldLineCount: Int,
            newLines: [String]
        ) {
            let localStart = globalStartLine - viewportStartLine
            let oldCount = lineStartOffsets.count
            guard localStart >= 0,
                  localStart < oldCount,
                  localStart + oldLineCount <= oldCount,
                  localStart + oldLineCount < oldCount
            else {
                rebuildLineStartOffsetsForViewport()
                return
            }

            let baseOffset = lineStartOffsets[localStart]
            let oldBlockSpan = lineStartOffsets[localStart + oldLineCount] - baseOffset

            var newBlockSpan = 0
            var newOffsets: [Int] = []
            newOffsets.reserveCapacity(newLines.count)
            for line in newLines {
                newOffsets.append(baseOffset + newBlockSpan)
                newBlockSpan += (line as NSString).length + 1
            }

            let delta = newBlockSpan - oldBlockSpan
            lineStartOffsets.replaceSubrange(localStart ..< localStart + oldLineCount, with: newOffsets)

            let shiftStart = localStart + newLines.count
            if delta != 0, shiftStart < lineStartOffsets.count {
                for index in shiftStart ..< lineStartOffsets.count {
                    lineStartOffsets[index] += delta
                }
            }
        }

        // MARK: - Editor Focus

        func focusEditorPreservingSelection() {
            guard let textView else { return }
            if let viewport = viewportState, !viewportSearchMatches.isEmpty {
                let currentIndex = max(0, state.searchCurrentIndex - 1)
                if currentIndex < viewportSearchMatches.count {
                    let match = viewportSearchMatches[currentIndex]
                    if let localLine = viewport.viewportLine(forBackingStoreLine: match.lineIndex) {
                        let localCharOffset = charOffsetForLocalLine(localLine)
                        let selectRange = NSRange(
                            location: localCharOffset + match.range.location,
                            length: match.range.length
                        )
                        let content = textView.string as NSString
                        if NSMaxRange(selectRange) <= content.length {
                            textView.setSelectedRange(selectRange)
                        }
                    }
                }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.02) { [weak textView] in
                guard let textView, let window = textView.window else { return }
                window.makeFirstResponder(textView)
            }
        }

        // MARK: - Search Highlighting

        func clearSearchHighlights() {
            viewportSearchMatches = []
            state.searchMatchCount = 0
            state.searchCurrentIndex = 0
            applySearchHighlights()
        }

        func applySearchHighlights(force: Bool = false) {
            let perfStart = beginPerfTiming()
            var highlightedRangeCount = 0
            defer {
                recordHighlightTiming(start: perfStart, highlightedRangeCount: highlightedRangeCount, force: force)
            }

            guard let textView, let layoutManager = textView.layoutManager else { return }
            let storageLength = textView.textStorage?.length ?? 0
            guard storageLength > 0 else {
                appliedSearchHighlightRanges.removeAll(keepingCapacity: true)
                appliedCurrentSearchMatchRange = nil
                return
            }

            guard let viewport = viewportState, !viewportSearchMatches.isEmpty else {
                guard !appliedSearchHighlightRanges.isEmpty || appliedCurrentSearchMatchRange != nil else { return }
                clearAppliedSearchHighlights(layoutManager: layoutManager, storageLength: storageLength)
                textView.needsDisplay = true
                return
            }

            var nextRanges: [NSRange] = []
            nextRanges.reserveCapacity(min(viewportSearchMatches.count, 256))

            let currentIndex = max(0, state.searchCurrentIndex - 1)
            var nextCurrentRange: NSRange?
            let visibleStartLine = viewport.viewportStartLine
            let visibleEndLine = viewport.viewportEndLine

            for (i, match) in viewportSearchMatches.enumerated() {
                if match.lineIndex < visibleStartLine {
                    continue
                }
                if match.lineIndex >= visibleEndLine {
                    break
                }
                guard let localLine = viewport.viewportLine(forBackingStoreLine: match.lineIndex) else { continue }
                let localCharOffset = charOffsetForLocalLine(localLine)
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
               appliedSearchHighlightRanges == nextRanges,
               appliedCurrentSearchMatchRange == nextCurrentRange
            {
                return
            }

            clearAppliedSearchHighlights(layoutManager: layoutManager, storageLength: storageLength)
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
            appliedSearchHighlightRanges = nextRanges
            appliedCurrentSearchMatchRange = nextCurrentRange
            textView.needsDisplay = true
        }

        // MARK: - Viewport Search

        private var viewportSearchMatches: [TextBackingStore.SearchMatch] = []
        private var appliedSearchHighlightRanges: [NSRange] = []
        private var appliedCurrentSearchMatchRange: NSRange?

        private func clearAppliedSearchHighlights(layoutManager: NSLayoutManager, storageLength: Int) {
            for range in appliedSearchHighlightRanges {
                guard NSMaxRange(range) <= storageLength else { continue }
                layoutManager.removeTemporaryAttribute(.backgroundColor, forCharacterRange: range)
                layoutManager.removeTemporaryAttribute(.foregroundColor, forCharacterRange: range)
            }
            if let range = appliedCurrentSearchMatchRange, NSMaxRange(range) <= storageLength {
                layoutManager.removeTemporaryAttribute(.foregroundColor, forCharacterRange: range)
            }
            appliedSearchHighlightRanges.removeAll(keepingCapacity: true)
            appliedCurrentSearchMatchRange = nil
        }

        func performSearchViewport(_ needle: String, caseSensitive: Bool, useRegex: Bool) {
            guard let store = state.backingStore else { return }
            state.searchInvalidRegex = false
            viewportSearchMatches = []
            guard !needle.isEmpty else {
                state.searchMatchCount = 0
                state.searchCurrentIndex = 0
                applySearchHighlights()
                return
            }
            if useRegex {
                if (try? NSRegularExpression(pattern: needle)) == nil {
                    state.searchInvalidRegex = true
                    state.searchMatchCount = 0
                    state.searchCurrentIndex = 0
                    applySearchHighlights()
                    return
                }
            }
            viewportSearchMatches = store.search(needle: needle, caseSensitive: caseSensitive, useRegex: useRegex)
            state.searchMatchCount = viewportSearchMatches.count
            if !viewportSearchMatches.isEmpty {
                state.searchCurrentIndex = 1
                scrollToSearchMatch(at: 0)
            } else {
                state.searchCurrentIndex = 0
                applySearchHighlights()
            }
        }

        func navigateSearchViewport(forward: Bool) {
            guard !viewportSearchMatches.isEmpty else { return }
            var idx = state.searchCurrentIndex - 1
            if forward {
                idx = (idx + 1) % viewportSearchMatches.count
            } else {
                idx = (idx - 1 + viewportSearchMatches.count) % viewportSearchMatches.count
            }
            state.searchCurrentIndex = idx + 1
            scrollToSearchMatch(at: idx)
        }

        func replaceCurrentViewport(with replacement: String, needle: String, caseSensitive: Bool, useRegex: Bool) {
            guard let store = state.backingStore, !needle.isEmpty, !viewportSearchMatches.isEmpty else { return }
            clearViewportHistory()
            let currentIndex = max(0, state.searchCurrentIndex - 1)
            guard currentIndex < viewportSearchMatches.count else { return }
            let match = viewportSearchMatches[currentIndex]
            let line = store.line(at: match.lineIndex)
            let nsLine = line as NSString
            let newLine = nsLine.replacingCharacters(in: match.range, with: replacement)
            _ = store.replaceLines(in: match.lineIndex ..< match.lineIndex + 1, with: [newLine])
            state.backingStoreVersion += 1
            state.markModified()
            invalidateSyntaxHighlightsFromLine(match.lineIndex)
            invalidateRenderedViewportText()
            performSearchViewport(needle, caseSensitive: caseSensitive, useRegex: useRegex)
            refreshViewport(force: true)
        }

        func replaceAllViewport(with replacement: String, needle: String, caseSensitive: Bool, useRegex: Bool) {
            guard let store = state.backingStore, !needle.isEmpty, !viewportSearchMatches.isEmpty else { return }
            clearViewportHistory()
            var grouped: [Int: [NSRange]] = [:]
            for match in viewportSearchMatches {
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
                invalidateSyntaxHighlightsFromLine(earliestInvalidation)
            }
            state.backingStoreVersion += 1
            state.markModified()
            invalidateRenderedViewportText()
            performSearchViewport(needle, caseSensitive: caseSensitive, useRegex: useRegex)
            refreshViewport(force: true)
        }

        private func scrollToSearchMatch(at index: Int) {
            guard index >= 0, index < viewportSearchMatches.count,
                  let viewport = viewportState, let scrollView, let textView
            else { return }
            let match = viewportSearchMatches[index]
            let targetScrollY = viewport.scrollY(forLine: match.lineIndex)
            let visibleHeight = scrollView.contentView.bounds.height
            let centeredY = max(0, targetScrollY - visibleHeight / 2)
            scrollView.contentView.setBoundsOrigin(NSPoint(x: 0, y: centeredY))
            scrollView.reflectScrolledClipView(scrollView.contentView)
            refreshViewport(force: true)

            guard let localLine = viewport.viewportLine(forBackingStoreLine: match.lineIndex) else { return }
            let localCharOffset = charOffsetForLocalLine(localLine)
            let matchStart = localCharOffset + match.range.location
            let content = textView.string as NSString
            guard matchStart <= content.length else { return }
            textView.setSelectedRange(NSRange(location: matchStart, length: 0))
            applySearchHighlights()
        }

        private func charOffsetForLocalLine(_ localLine: Int) -> Int {
            guard localLine >= 0, localLine < lineStartOffsets.count else { return 0 }
            return lineStartOffsets[localLine]
        }

        private func viewportContentWidth(for textView: NSTextView, scrollView: NSScrollView) -> CGFloat {
            guard let layoutManager = textView.layoutManager, let textContainer = textView.textContainer else {
                return scrollView.contentSize.width
            }
            layoutManager.ensureLayout(for: textContainer)
            let padding = textView.textContainerInset.width * 2 + textContainer.lineFragmentPadding * 2
            let usedWidth = layoutManager.usedRect(for: textContainer).width + padding
            return max(scrollView.contentSize.width, ceil(usedWidth))
        }

        private func updateViewportFrames(
            viewport: ViewportState,
            textView: NSTextView,
            scrollView: NSScrollView,
            yOffset: CGFloat,
            visibleLineCount: Int
        ) {
            let estimatedHeight = viewport.estimatedLineHeight * CGFloat(max(1, visibleLineCount))
                + textView.textContainerInset.height * 2
            let viewportWidth = viewportContentWidth(for: textView, scrollView: scrollView)
            let newTextFrame = NSRect(
                x: 0,
                y: yOffset,
                width: viewportWidth,
                height: max(estimatedHeight, 100)
            )
            if textView.frame != newTextFrame {
                textView.frame = newTextFrame
            }

            if let container = containerView {
                let containerHeight = max(viewport.totalDocumentHeight, scrollView.contentView.bounds.height)
                let newContainerFrame = NSRect(
                    x: 0,
                    y: 0,
                    width: viewportWidth,
                    height: containerHeight
                )
                if container.frame != newContainerFrame {
                    container.frame = newContainerFrame
                }
            }
        }

        private func ensureViewportMinimumWidth() {
            guard let viewport = viewportState, let scrollView, let textView, let container = containerView else { return }
            let minimumWidth = scrollView.contentSize.width
            guard textView.frame.width < minimumWidth || container.frame.width < minimumWidth else { return }
            let width = max(minimumWidth, textView.frame.width)
            textView.frame = NSRect(
                x: textView.frame.origin.x,
                y: textView.frame.origin.y,
                width: width,
                height: textView.frame.height
            )
            container.frame = NSRect(
                x: 0,
                y: 0,
                width: width,
                height: max(viewport.totalDocumentHeight, scrollView.contentView.bounds.height)
            )
        }

        // MARK: - Scroll Observer

        func setScrollObserver(for scrollView: NSScrollView) {
            guard observedContentView !== scrollView.contentView else { return }
            removeScrollObserver()
            observedContentView = scrollView.contentView
            lastObservedClipSize = scrollView.contentView.bounds.size
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(handleScrollBoundsChange),
                name: NSView.boundsDidChangeNotification,
                object: scrollView.contentView
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(handleClipFrameChange),
                name: NSView.frameDidChangeNotification,
                object: scrollView.contentView
            )
        }

        private func removeScrollObserver() {
            NotificationCenter.default.removeObserver(
                self,
                name: NSView.boundsDidChangeNotification,
                object: observedContentView
            )
            NotificationCenter.default.removeObserver(
                self,
                name: NSView.frameDidChangeNotification,
                object: observedContentView
            )
            observedContentView = nil
            lastObservedClipSize = .zero
        }

        @objc
        private func handleScrollBoundsChange() {
            reconcileClipSize(observedContentView?.bounds.size)
        }

        @objc
        private func handleClipFrameChange() {
            reconcileClipSize(observedContentView?.frame.size)
        }

        private func reconcileClipSize(_ size: CGSize?) {
            if let size {
                if size.width != lastObservedClipSize.width {
                    ensureViewportMinimumWidth()
                }
                if size.height != lastObservedClipSize.height {
                    updateContainerHeight()
                }
                lastObservedClipSize = size
            }
            if !isEditingViewport {
                refreshViewport(force: false)
            }
        }

        // MARK: - NSTextViewDelegate

        func textDidChange(_: Notification) {
            guard let textView, !isUpdating else { return }
            handleTextDidChangeViewport(textView)
        }

        private func handleTextDidChangeViewport(_ textView: NSTextView) {
            guard let viewport = viewportState, let scrollView else { return }
            let pendingEdit = pendingViewportEdit
            pendingViewportEdit = nil
            let cursorLocation = textView.selectedRange().location
            let viewportStartLine = viewport.viewportStartLine
            var lineDelta = 0
            var recordedViewportEdit = false

            if let pendingEdit {
                let oldRange = pendingEdit.startLine ..< pendingEdit.startLine + pendingEdit.oldLines.count
                _ = viewport.backingStore.replaceLines(in: oldRange, with: pendingEdit.newLines)
                lineDelta = pendingEdit.newLines.count - pendingEdit.oldLines.count
                let newViewportEnd = max(viewportStartLine, viewport.viewportEndLine + lineDelta)
                viewport.applyViewport(viewportStartLine ..< newViewportEnd)
            } else {
                if !isApplyingViewportHistory {
                    clearViewportHistory()
                }
                let newLocalText = textView.string
                let newLocalLines = newLocalText.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
                let oldRange = viewport.viewportStartLine ..< viewport.viewportEndLine
                _ = viewport.backingStore.replaceLines(in: oldRange, with: newLocalLines)
                lineDelta = newLocalLines.count - oldRange.count
                viewport.applyViewport(viewport.viewportStartLine ..< viewport.viewportStartLine + newLocalLines.count)
                invalidateSyntaxHighlightsFromLine(viewportStartLine)
            }

            state.markModified()

            isEditingViewport = true
            defer { isEditingViewport = false }

            lastRenderedViewportRange = viewport.viewportStartLine ..< viewport.viewportEndLine
            lastRenderedBackingStoreVersion = state.backingStoreVersion
            needsViewportTextReload = false
            if let pendingEdit {
                updateLineStartOffsetsAfterEdit(
                    viewportStartLine: viewportStartLine,
                    globalStartLine: pendingEdit.startLine,
                    oldLineCount: pendingEdit.oldLines.count,
                    newLines: pendingEdit.newLines
                )
            } else {
                rebuildLineStartOffsetsForViewport()
            }

            if let pendingEdit,
               !isApplyingViewportHistory,
               let selectionAfter = globalCursorFromLocalLocation(cursorLocation)
            {
                pushViewportEdit(ViewportEdit(
                    startLine: pendingEdit.startLine,
                    oldLines: pendingEdit.oldLines,
                    newLines: pendingEdit.newLines,
                    selectionBefore: pendingEdit.selectionBefore,
                    selectionAfter: selectionAfter
                ))
                recordedViewportEdit = true
            }

            if pendingEdit != nil, !recordedViewportEdit, !isApplyingViewportHistory {
                clearViewportHistory()
            }

            if lineDelta != 0 {
                updateContainerHeight()
                updateViewportFrames(
                    viewport: viewport,
                    textView: textView,
                    scrollView: scrollView,
                    yOffset: viewport.viewportYOffset(),
                    visibleLineCount: max(1, viewport.viewportLineCount)
                )
            }

            scrollCursorVisibleInViewport(textView: textView, cursorLocation: cursorLocation)

            let scrollY = scrollView.contentView.bounds.origin.y
            let visibleHeight = scrollView.contentView.bounds.height
            if viewport.shouldUpdateViewport(scrollY: scrollY, visibleHeight: visibleHeight) {
                if let pendingEdit {
                    invalidateSyntaxHighlightsFromLine(pendingEdit.startLine)
                }
                let localLine = lineNumber(atCharacterLocation: cursorLocation)
                let globalLine = viewport.backingStoreLine(forViewportLine: localLine - 1)
                let columnOffset = cursorLocation - lineStartOffsets[max(0, min(localLine - 1, lineStartOffsets.count - 1))]

                refreshViewport(force: true)

                if let newLocalLine = viewport.viewportLine(forBackingStoreLine: globalLine) {
                    let newCharOffset = charOffsetForLocalLine(newLocalLine)
                    let content = textView.string as NSString
                    let lineRange = content.lineRange(for: NSRange(location: newCharOffset, length: 0))
                    let lineLength = lineRange.length - (NSMaxRange(lineRange) < content.length ? 1 : 0)
                    let newCursor = newCharOffset + min(columnOffset, max(0, lineLength))
                    let safeCursor = min(newCursor, content.length)
                    textView.setSelectedRange(NSRange(location: safeCursor, length: 0))
                    scrollCursorVisibleInViewport(textView: textView, cursorLocation: safeCursor)
                }
            } else {
                if let pendingEdit {
                    applyIncrementalSyntaxHighlights(
                        startLine: pendingEdit.startLine,
                        oldLineCount: pendingEdit.oldLines.count,
                        newLineCount: pendingEdit.newLines.count
                    )
                } else {
                    reapplySyntaxHighlights()
                }
            }
        }

        func clearViewportHistory() {
            pendingViewportEdit = nil
            viewportUndoStack.removeAll(keepingCapacity: false)
            viewportRedoStack.removeAll(keepingCapacity: false)
            lastViewportEditTimestamp = nil
        }

        func performUndoRequest() -> Bool {
            performViewportUndo()
        }

        func performRedoRequest() -> Bool {
            performViewportRedo()
        }

        func canPerformUndoRequest() -> Bool {
            !viewportUndoStack.isEmpty
        }

        func canPerformRedoRequest() -> Bool {
            !viewportRedoStack.isEmpty
        }

        private func performViewportUndo() -> Bool {
            guard let viewport = viewportState else { return false }
            guard let group = viewportUndoStack.popLast(), !group.edits.isEmpty else { return false }

            isApplyingViewportHistory = true
            defer { isApplyingViewportHistory = false }

            var earliestInvalidation = Int.max
            for edit in group.edits.reversed() {
                let replaceRange = edit.startLine ..< edit.startLine + edit.newLines.count
                _ = viewport.backingStore.replaceLines(in: replaceRange, with: edit.oldLines)
                adjustViewportRangeForReplacement(
                    startLine: edit.startLine,
                    replacedLineCount: edit.newLines.count,
                    insertedLineCount: edit.oldLines.count
                )
                earliestInvalidation = min(earliestInvalidation, edit.startLine)
            }
            if earliestInvalidation != Int.max {
                invalidateSyntaxHighlightsFromLine(earliestInvalidation)
            }
            state.markModified()
            invalidateRenderedViewportText()
            appendViewportRedo(group)
            if let selection = group.edits.first?.selectionBefore {
                applyViewportHistorySelection(selection)
            }
            lastViewportEditTimestamp = nil
            return true
        }

        private func performViewportRedo() -> Bool {
            guard let viewport = viewportState else { return false }
            guard let group = viewportRedoStack.popLast(), !group.edits.isEmpty else { return false }

            isApplyingViewportHistory = true
            defer { isApplyingViewportHistory = false }

            var earliestInvalidation = Int.max
            for edit in group.edits {
                let replaceRange = edit.startLine ..< edit.startLine + edit.oldLines.count
                _ = viewport.backingStore.replaceLines(in: replaceRange, with: edit.newLines)
                adjustViewportRangeForReplacement(
                    startLine: edit.startLine,
                    replacedLineCount: edit.oldLines.count,
                    insertedLineCount: edit.newLines.count
                )
                earliestInvalidation = min(earliestInvalidation, edit.startLine)
            }
            if earliestInvalidation != Int.max {
                invalidateSyntaxHighlightsFromLine(earliestInvalidation)
            }
            state.markModified()
            invalidateRenderedViewportText()
            appendViewportUndo(group)
            if let selection = group.edits.last?.selectionAfter {
                applyViewportHistorySelection(selection)
            }
            lastViewportEditTimestamp = nil
            return true
        }

        private func pushViewportEdit(_ edit: ViewportEdit) {
            let now = CFAbsoluteTimeGetCurrent()
            if shouldCoalesceViewportEdit(edit, now: now), var group = viewportUndoStack.popLast() {
                group.edits.append(edit)
                viewportUndoStack.append(group)
            } else {
                appendViewportUndo(ViewportEditGroup(edits: [edit]))
            }
            viewportRedoStack.removeAll(keepingCapacity: false)
            lastViewportEditTimestamp = now
        }

        private func appendViewportUndo(_ group: ViewportEditGroup) {
            viewportUndoStack.append(group)
            if viewportUndoStack.count > Self.viewportUndoLimit {
                viewportUndoStack.removeFirst(viewportUndoStack.count - Self.viewportUndoLimit)
            }
        }

        private func appendViewportRedo(_ group: ViewportEditGroup) {
            viewportRedoStack.append(group)
            if viewportRedoStack.count > Self.viewportUndoLimit {
                viewportRedoStack.removeFirst(viewportRedoStack.count - Self.viewportUndoLimit)
            }
        }

        private func shouldCoalesceViewportEdit(_ edit: ViewportEdit, now: CFAbsoluteTime) -> Bool {
            guard let lastTimestamp = lastViewportEditTimestamp else { return false }
            guard now - lastTimestamp <= Self.viewportUndoCoalesceInterval else { return false }
            guard let lastEdit = viewportUndoStack.last?.edits.last else { return false }
            return lastEdit.selectionAfter.line == edit.selectionBefore.line
                && lastEdit.selectionAfter.column == edit.selectionBefore.column
        }

        private func adjustViewportRangeForReplacement(
            startLine: Int,
            replacedLineCount: Int,
            insertedLineCount: Int
        ) {
            guard let viewport = viewportState else { return }
            let lineDelta = insertedLineCount - replacedLineCount
            guard lineDelta != 0 else { return }

            let changeEnd = startLine + replacedLineCount
            var newStart = viewport.viewportStartLine
            var newEnd = viewport.viewportEndLine

            if changeEnd <= newStart {
                newStart += lineDelta
                newEnd += lineDelta
            } else if startLine < newEnd {
                newEnd += lineDelta
            }

            let maxLine = max(1, viewport.backingStore.lineCount)
            newStart = max(0, min(newStart, maxLine - 1))
            newEnd = max(newStart + 1, min(newEnd, maxLine))
            viewport.applyViewport(newStart ..< newEnd)
        }

        private func applyViewportHistorySelection(_ selection: ViewportCursor) {
            updateContainerHeight()
            scrollToGlobalLine(selection.line, column: selection.column)
        }

        private func captureViewportPendingEdit(
            textView: NSTextView,
            affectedCharRange: NSRange,
            replacementString: String?
        ) {
            pendingViewportEdit = nil
            guard let viewport = viewportState else { return }

            let content = textView.string as NSString
            guard isValidEditRange(affectedCharRange, textLength: content.length) else { return }
            guard let selectionBefore = globalCursorFromLocalLocation(textView.selectedRange().location) else { return }
            guard !lineStartOffsets.isEmpty else { return }

            let safeStart = min(max(0, affectedCharRange.location), content.length)
            let safeEnd = min(content.length, NSMaxRange(affectedCharRange))
            let startLocalLine = max(0, lineNumber(atCharacterLocation: safeStart) - 1)
            let endLocalLine = max(startLocalLine, lineNumber(atCharacterLocation: safeEnd) - 1)
            let maxLocalLine = lineStartOffsets.count - 1
            let clampedStartLocalLine = min(startLocalLine, maxLocalLine)
            let clampedEndLocalLine = min(endLocalLine, maxLocalLine)

            let globalStartLine = viewport.backingStoreLine(forViewportLine: clampedStartLocalLine)
            let globalEndLine = viewport.backingStoreLine(forViewportLine: clampedEndLocalLine)
            let oldRange = globalStartLine ..< globalEndLine + 1
            let oldLines = oldRange.map { viewport.backingStore.line(at: $0) }
            guard !oldLines.isEmpty else { return }

            let oldBlock = oldLines.joined(separator: "\n") as NSString
            let blockStartOffset = lineStartOffsets[clampedStartLocalLine]
            let relativeRange = NSRange(
                location: affectedCharRange.location - blockStartOffset,
                length: affectedCharRange.length
            )
            guard isValidEditRange(relativeRange, textLength: oldBlock.length) else { return }

            let replacement = replacementString ?? ""
            let newBlock = oldBlock.replacingCharacters(in: relativeRange, with: replacement)
            let newLines = newBlock.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)

            pendingViewportEdit = PendingViewportEdit(
                startLine: globalStartLine,
                oldLines: oldLines,
                newLines: newLines,
                selectionBefore: selectionBefore
            )
        }

        private func globalCursorFromLocalLocation(_ location: Int) -> ViewportCursor? {
            guard let viewport = viewportState, let textView, !lineStartOffsets.isEmpty else { return nil }
            let content = textView.string as NSString
            let safeLocation = min(max(0, location), content.length)
            let localLine = lineNumber(atCharacterLocation: safeLocation)
            let localLineIndex = max(0, min(localLine - 1, lineStartOffsets.count - 1))
            let lineStart = lineStartOffsets[localLineIndex]
            let column = max(0, safeLocation - lineStart)
            let globalLine = viewport.backingStoreLine(forViewportLine: localLineIndex)
            return ViewportCursor(line: globalLine, column: column)
        }

        private func scrollCursorVisibleInViewport(textView: NSTextView, cursorLocation: Int) {
            guard let scrollView,
                  let layoutManager = textView.layoutManager,
                  let textContainer = textView.textContainer
            else { return }

            let content = textView.string as NSString
            let safeLoc = min(cursorLocation, content.length)
            layoutManager.ensureLayout(forCharacterRange: NSRange(location: safeLoc, length: 0))
            let glyphRange = layoutManager.glyphRange(
                forCharacterRange: NSRange(location: safeLoc, length: 0),
                actualCharacterRange: nil
            )
            var cursorRect = layoutManager.boundingRect(forGlyphRange: glyphRange, in: textContainer)
            cursorRect.origin.x += textView.textContainerOrigin.x + textView.frame.origin.x
            cursorRect.origin.y += textView.textContainerOrigin.y + textView.frame.origin.y

            let clipBounds = scrollView.contentView.bounds
            let visibleMinX = clipBounds.origin.x
            let visibleMaxX = visibleMinX + clipBounds.width
            let visibleMinY = clipBounds.origin.y
            let visibleMaxY = visibleMinY + clipBounds.height

            let cursorMinX = cursorRect.origin.x
            let cursorMaxX = cursorRect.origin.x + max(cursorRect.width, 2)
            var newOrigin = clipBounds.origin

            if cursorMaxX > visibleMaxX {
                let maxScrollX = max(0, (containerView?.frame.width ?? textView.frame.width) - clipBounds.width)
                newOrigin.x = min(maxScrollX, max(0, cursorMaxX - clipBounds.width))
            } else if cursorMinX < visibleMinX {
                let maxScrollX = max(0, (containerView?.frame.width ?? textView.frame.width) - clipBounds.width)
                newOrigin.x = min(maxScrollX, max(0, cursorMinX))
            }

            if cursorRect.maxY > visibleMaxY {
                newOrigin.y = cursorRect.maxY - clipBounds.height
            } else if cursorRect.origin.y < visibleMinY {
                newOrigin.y = cursorRect.origin.y
            }

            if newOrigin != clipBounds.origin {
                scrollView.contentView.setBoundsOrigin(newOrigin)
                scrollView.reflectScrolledClipView(scrollView.contentView)
            }
        }

        func textView(
            _ textView: NSTextView,
            shouldChangeTextIn affectedCharRange: NSRange,
            replacementString: String?
        ) -> Bool {
            guard !isUpdating else { return true }
            captureViewportPendingEdit(
                textView: textView,
                affectedCharRange: affectedCharRange,
                replacementString: replacementString
            )
            return true
        }

        func textViewDidChangeSelection(_: Notification) {
            guard let textView, !isUpdating else { return }
            let range = textView.selectedRange()
            let content = textView.string as NSString
            let loc = min(range.location, content.length)

            let localLine = lineNumber(atCharacterLocation: loc)
            let localLineIndex = localLine - 1

            let globalLine = viewportState?.backingStoreLine(forViewportLine: localLineIndex) ?? localLine
            state.cursorLine = globalLine + 1
            let localLineStart = lineStartOffsets[max(0, min(localLineIndex, lineStartOffsets.count - 1))]
            state.cursorColumn = max(1, loc - localLineStart + 1)

            updateCurrentSelection(in: textView, range: range)
        }

        private func handleMoveAtViewportBoundary(direction: Int) -> Bool {
            guard let viewport = viewportState, let textView else { return false }
            let range = textView.selectedRange()
            let content = textView.string as NSString
            let loc = min(range.location, content.length)
            let localLine = lineNumber(atCharacterLocation: loc)
            let localLineIndex = localLine - 1
            let totalLocalLines = lineStartOffsets.count

            let atFirstLine = localLineIndex <= 0
            let atLastLine = localLineIndex >= totalLocalLines - 1

            if direction < 0, atFirstLine, viewport.viewportStartLine > 0 {
                let lineStart = lineStartOffsets[max(0, min(localLineIndex, lineStartOffsets.count - 1))]
                let column = max(0, loc - lineStart)
                let globalLine = viewport.backingStoreLine(forViewportLine: localLineIndex)
                let targetGlobalLine = max(0, globalLine - 1)
                scrollToGlobalLine(targetGlobalLine, column: column)
                return true
            }

            if direction > 0, atLastLine, viewport.viewportEndLine < viewport.backingStore.lineCount {
                let lineStart = lineStartOffsets[max(0, min(localLineIndex, lineStartOffsets.count - 1))]
                let column = max(0, loc - lineStart)
                let globalLine = viewport.backingStoreLine(forViewportLine: localLineIndex)
                let targetGlobalLine = min(viewport.backingStore.lineCount - 1, globalLine + 1)
                scrollToGlobalLine(targetGlobalLine, column: column)
                return true
            }

            return false
        }

        private func scrollToGlobalLine(_ globalLine: Int, column: Int) {
            guard let viewport = viewportState, let scrollView, let textView else { return }

            let targetScrollY = viewport.scrollY(forLine: globalLine)
            let visibleHeight = scrollView.contentView.bounds.height
            let currentScrollY = scrollView.contentView.bounds.origin.y

            let lineTop = targetScrollY
            let lineBottom = targetScrollY + viewport.estimatedLineHeight

            var newScrollY = currentScrollY
            if lineBottom > currentScrollY + visibleHeight {
                newScrollY = lineBottom - visibleHeight
            } else if lineTop < currentScrollY {
                newScrollY = lineTop
            }

            let maxScrollY = max(0, viewport.totalDocumentHeight - visibleHeight)
            newScrollY = min(maxScrollY, max(0, newScrollY))

            scrollView.contentView.setBoundsOrigin(NSPoint(x: scrollView.contentView.bounds.origin.x, y: newScrollY))
            scrollView.reflectScrolledClipView(scrollView.contentView)

            refreshViewport(force: true)
            rebuildLineStartOffsetsForViewport()

            guard let newLocalLine = viewport.viewportLine(forBackingStoreLine: globalLine) else { return }
            let newCharOffset = charOffsetForLocalLine(newLocalLine)
            let newContent = textView.string as NSString
            let lineRange = newContent.lineRange(for: NSRange(location: min(newCharOffset, newContent.length), length: 0))
            let lineLength = lineRange.length - (NSMaxRange(lineRange) < newContent.length ? 1 : 0)
            let newCursor = newCharOffset + min(column, max(0, lineLength))
            let safeCursor = min(newCursor, newContent.length)

            isUpdating = true
            textView.setSelectedRange(NSRange(location: safeCursor, length: 0))
            isUpdating = false

            state.cursorLine = globalLine + 1
            let cursorLineStart = lineStartOffsets[max(0, min(newLocalLine, lineStartOffsets.count - 1))]
            state.cursorColumn = max(1, safeCursor - cursorLineStart + 1)
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

        // MARK: - Line Start Offsets

        private func isValidEditRange(_ range: NSRange, textLength: Int) -> Bool {
            guard range.location != NSNotFound else { return false }
            guard range.location >= 0, range.length >= 0 else { return false }
            guard range.location <= textLength else { return false }
            guard range.length <= textLength - range.location else { return false }
            return true
        }

        func lineNumber(atCharacterLocation location: Int) -> Int {
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

        func textView(_: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            guard let textView else { return false }
            if commandSelector == Self.undoCommandSelector {
                return performUndoRequest()
            }
            if commandSelector == Self.redoCommandSelector {
                return performRedoRequest()
            }
            if commandSelector == #selector(NSResponder.cancelOperation(_:)), state.searchVisible {
                state.searchVisible = false
                return true
            }
            if commandSelector == #selector(NSResponder.deleteWordBackward(_:)) {
                return handleDeleteWordBackward(textView)
            }
            if commandSelector == #selector(NSResponder.moveUp(_:)) {
                return handleMoveAtViewportBoundary(direction: -1)
            }
            if commandSelector == #selector(NSResponder.moveDown(_:)) {
                return handleMoveAtViewportBoundary(direction: 1)
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
