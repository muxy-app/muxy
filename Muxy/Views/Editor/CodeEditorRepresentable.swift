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

final class EditorScrollContainer: NSView {
    let scrollView: NSScrollView
    private(set) var gutterView: LineNumberGutterView?

    init(scrollView: NSScrollView) {
        self.scrollView = scrollView
        super.init(frame: .zero)
        wantsLayer = true
        layer?.masksToBounds = true
        autoresizesSubviews = false
        addSubview(scrollView)
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var isFlipped: Bool { true }

    override func resizeSubviews(withOldSize _: NSSize) {
        layoutChildren()
    }

    override func layout() {
        super.layout()
        layoutChildren()
    }

    func setGutter(_ gutter: LineNumberGutterView?) {
        guard gutter !== gutterView else { return }
        gutterView?.removeFromSuperview()
        gutterView = gutter
        if let gutter {
            addSubview(gutter)
        }
        layoutChildren()
    }

    func gutterWidthDidChange() {
        layoutChildren()
    }

    private func layoutChildren() {
        let bounds = self.bounds
        let gutterWidth = gutterView?.preferredWidth ?? 0
        if let gutterView {
            let gutterFrame = NSRect(x: 0, y: 0, width: gutterWidth, height: bounds.height)
            if gutterView.frame != gutterFrame {
                gutterView.frame = gutterFrame
            }
        }
        let scrollFrame = NSRect(
            x: gutterWidth,
            y: 0,
            width: max(0, bounds.width - gutterWidth),
            height: bounds.height
        )
        if scrollView.frame != scrollFrame {
            scrollView.frame = scrollFrame
        }
    }
}

final class ViewportContainerView: NSView {
    override var isFlipped: Bool { true }

    override func mouseDown(with event: NSEvent) {
        guard let textView = subviews.compactMap({ $0 as? NSTextView }).first else {
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
    let showLineNumbers: Bool
    let lineWrapping: Bool
    let themeVersion: Int
    let showsVerticalScroller: Bool
    let focused: Bool
    let searchNeedle: String
    let searchNavigationVersion: Int
    let searchNavigationDirection: EditorSearchNavigationDirection
    let searchCaseSensitive: Bool
    let searchUseRegex: Bool
    let replaceText: String
    let replaceVersion: Int
    let replaceAllVersion: Int
    let editorFocusVersion: Int
    let onFocus: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(state: state, editorSettings: editorSettings)
    }

    func makeNSView(context: Context) -> EditorScrollContainer {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = showsVerticalScroller
        scrollView.hasHorizontalScroller = !lineWrapping
        scrollView.autoresizingMask = []

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
        let palette = EditorThemePalette.active
        textView.font = font
        textView.backgroundColor = palette.background
        textView.insertionPointColor = palette.foreground
        textView.textColor = palette.foreground
        textView.typingAttributes = [
            .font: font,
            .foregroundColor: palette.foreground,
        ]
        textView.selectedTextAttributes = [
            .backgroundColor: palette.foreground.withAlphaComponent(0.15),
        ]

        scrollView.autohidesScrollers = showsVerticalScroller
        scrollView.drawsBackground = true
        scrollView.backgroundColor = palette.background
        scrollView.contentView.drawsBackground = true
        scrollView.contentView.backgroundColor = palette.background
        scrollView.borderType = .noBorder
        scrollView.contentView.postsBoundsChangedNotifications = true
        scrollView.contentView.postsFrameChangedNotifications = true

        let coordinator = context.coordinator
        let container = EditorScrollContainer(scrollView: scrollView)
        container.autoresizingMask = [.width, .height]
        textView.delegate = coordinator
        coordinator.textView = textView
        coordinator.scrollView = scrollView
        coordinator.scrollContainer = container
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

        coordinator.applyLineWrapping(lineWrapping)

        return container
    }

    static func dismantleNSView(_: EditorScrollContainer, coordinator: Coordinator) {
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

    private static func claimFirstResponder(textView: NSTextView, attemptsRemaining: Int) {
        guard attemptsRemaining > 0 else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.02) { [weak textView] in
            guard let textView else { return }
            guard let window = textView.window else {
                claimFirstResponder(textView: textView, attemptsRemaining: attemptsRemaining - 1)
                return
            }
            window.makeFirstResponder(textView)
        }
    }

    func updateNSView(_ container: EditorScrollContainer, context: Context) {
        let scrollView = container.scrollView
        guard let textView = context.coordinator.textView else { return }
        let coordinator = context.coordinator

        if scrollView.hasVerticalScroller != showsVerticalScroller {
            scrollView.hasVerticalScroller = showsVerticalScroller
            scrollView.autohidesScrollers = showsVerticalScroller
        }

        if state.backingStore != nil, coordinator.viewportState == nil {
            coordinator.enterViewportMode(scrollView: scrollView)
        }

        coordinator.reconcileLineNumberGutter(showLineNumbers)
        coordinator.reconcileCurrentLineHighlight()
        coordinator.reconcileLineWrapping(lineWrapping)
        updateNSViewViewportMode(scrollView: scrollView, textView: textView, coordinator: coordinator)
    }

    private func updateNSViewViewportMode(scrollView: NSScrollView, textView: NSTextView, coordinator: Coordinator) {
        guard let viewport = coordinator.viewportState else { return }

        let backingStoreChanged = coordinator.lastSyncedBackingStoreVersion != state.backingStoreVersion
        if backingStoreChanged {
            coordinator.lastSyncedBackingStoreVersion = state.backingStoreVersion
            coordinator.invalidateRenderedViewportText()
            coordinator.clearViewportHistory()
            if viewport.heightMap.totalLineCount != viewport.backingStore.lineCount {
                viewport.resetMeasurements()
            }
        }

        let incrementalFinished = coordinator.wasIncrementalLoading && !state.isIncrementalLoading
        coordinator.wasIncrementalLoading = state.isIncrementalLoading

        if backingStoreChanged || incrementalFinished {
            coordinator.updateContainerHeight()
            coordinator.updateMarkdownEditorScrollMetrics()
        }

        if !coordinator.hasAppliedInitialContent, viewport.backingStore.lineCount > 1 || backingStoreChanged {
            coordinator.hasAppliedInitialContent = true
            if lineWrapping {
                coordinator.applyLineWrapping(true)
                viewport.resetMeasurements()
            }
            coordinator.refreshViewport(force: true)
            if focused, !state.suppressInitialFocus {
                Self.claimFirstResponder(textView: textView, attemptsRemaining: 20)
            }
            if state.suppressInitialFocus, !state.isMarkdownFile {
                state.suppressInitialFocus = false
            }
        }

        applyPendingJumpIfNeeded(coordinator: coordinator)

        let themeChanged = coordinator.lastThemeVersion != themeVersion
        let font = editorSettings.resolvedFont
        let fontChanged = textView.font != font

        applyThemeAndFont(scrollView: scrollView, textView: textView, font: font)

        if fontChanged {
            viewport.updateEstimatedLineHeight(font: font)
            viewport.updateDocumentPadding(
                topInset: textView.textContainerInset.height,
                bottomInset: textView.textContainerInset.height
            )
            coordinator.updateContainerHeight()
            coordinator.updateMarkdownEditorScrollMetrics()
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
        coordinator.syncMarkdownScrollPositionIfNeeded()
        coordinator.updateMarkdownEditorScrollMetrics()

        if coordinator.lastEditorFocusVersion != editorFocusVersion {
            coordinator.lastEditorFocusVersion = editorFocusVersion
            coordinator.focusEditorPreservingSelection()
        }
    }

    private func applyThemeAndFont(scrollView: NSScrollView, textView: NSTextView, font: NSFont) {
        let palette = EditorThemePalette.active
        let fgColor = palette.foreground
        let bgColor = palette.background

        if !scrollView.drawsBackground {
            scrollView.drawsBackground = true
        }
        if scrollView.backgroundColor != bgColor {
            scrollView.backgroundColor = bgColor
        }
        if !scrollView.contentView.drawsBackground {
            scrollView.contentView.drawsBackground = true
        }
        if scrollView.contentView.backgroundColor != bgColor {
            scrollView.contentView.backgroundColor = bgColor
        }
        if let documentView = scrollView.documentView, documentView !== textView {
            documentView.wantsLayer = true
            documentView.layer?.backgroundColor = bgColor.cgColor
        }
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

    private func applyPendingJumpIfNeeded(coordinator: Coordinator) {
        guard let line = state.pendingJumpLine else { return }
        let version = state.pendingJumpVersion
        if coordinator.lastPendingJumpVersion != version {
            coordinator.lastPendingJumpVersion = version
            coordinator.pendingJumpDeferred = true
        }
        guard coordinator.pendingJumpDeferred, coordinator.hasAppliedInitialContent else { return }
        coordinator.pendingJumpDeferred = false
        let column = state.pendingJumpColumn
        coordinator.applyPendingJump(line: line, column: column)
        state.pendingJumpLine = nil
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

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate, SyntaxHighlightCoordinator, SearchControllerHost, ViewportEditHistoryHost,
        CurrentLineHighlightHost, LineNumberGutterHost
    {
        let state: EditorTabState
        let editorSettings: EditorSettings
        weak var textView: NSTextView?

        weak var scrollView: NSScrollView?
        weak var scrollContainer: EditorScrollContainer?
        var viewportState: ViewportState?
        var containerView: ViewportContainerView?
        private(set) var lineWrappingEnabled: Bool = false
        private var pendingWrapResizeWorkItem: DispatchWorkItem?

        var isUpdating = false
        private var isEditingViewport = false
        private var isReconfiguringLineWrapping = false
        private(set) var scrollAnchor = ScrollAnchor()
        private var isWritingScrollProgrammatically = false
        var hasAppliedInitialContent = false
        var lastThemeVersion = -1
        var lastSearchVisible = false
        var lastSearchNeedle = ""
        var lastSearchNavigationVersion = -1
        var lastPendingJumpVersion = 0
        var pendingJumpDeferred = false
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
        private static let undoCommandSelector = #selector(CodeEditorTextView.undo(_:))
        private static let redoCommandSelector = #selector(CodeEditorTextView.redo(_:))
        private static let previewRefreshDebounceNanos: UInt64 = 500_000_000
        private static let perfLogger = Logger(subsystem: "app.muxy", category: "EditorPerf")
        private static let perfEnabled: Bool = {
            if let env = ProcessInfo.processInfo.environment["MUXY_EDITOR_PERF"] {
                let value = env.lowercased()
                return value == "1" || value == "true" || value == "yes"
            }
            return UserDefaults.standard.bool(forKey: "MuxyEditorPerf")
        }()

        private lazy var history = ViewportEditHistory(host: self)
        private var needsViewportTextReload = true
        private var lastRenderedViewportRange: Range<Int>?
        private var lastRenderedBackingStoreVersion = -1
        private var lastObservedClipSize: CGSize = .zero
        private lazy var markdownScrollSync = MarkdownScrollSyncController(state: state)
        private var refreshTimingCount = 0
        private var highlightTimingCount = 0
        private var lastRefreshDurationMs: Double = 0
        private var lastHighlightDurationMs: Double = 0
        private var previewRefreshTask: Task<Void, Never>?
        private var pendingCascadeReapplyGeneration: UInt64 = 0
        private var extensions: [EditorExtension] = []
        private lazy var searchController = SearchController(host: self)

        init(state: EditorTabState, editorSettings: EditorSettings) {
            self.state = state
            self.editorSettings = editorSettings
            super.init()
            var loaded: [EditorExtension] = [SyntaxHighlightExtension(coordinator: self)]
            if state.isMarkdownFile {
                loaded.append(MarkdownInlineExtension())
            }
            if editorSettings.showLineNumbers {
                loaded.append(LineNumberGutterExtension(host: self))
            }
            if editorSettings.highlightCurrentLine {
                loaded.append(CurrentLineHighlightExtension(host: self))
            }
            extensions = loaded
        }

        func reconcileLineNumberGutter(_ showLineNumbers: Bool) {
            let hasGutter = extensions.contains(where: { $0 is LineNumberGutterExtension })
            if showLineNumbers, !hasGutter {
                let ext = LineNumberGutterExtension(host: self)
                extensions.append(ext)
                if let context = makeRenderContext() {
                    ext.didMount(context: context)
                    refreshViewportPinningAnchor()
                }
                return
            }
            if !showLineNumbers, hasGutter {
                let context = makeRenderContext()
                let removed = extensions.filter { $0 is LineNumberGutterExtension }
                extensions.removeAll { $0 is LineNumberGutterExtension }
                if let context {
                    for ext in removed {
                        ext.willUnmount(context: context)
                    }
                }
                refreshViewportPinningAnchor()
            }
        }

        func reconcileCurrentLineHighlight() {
            let hasHighlight = extensions.contains(where: { $0 is CurrentLineHighlightExtension })
            if editorSettings.highlightCurrentLine, !hasHighlight {
                let ext = CurrentLineHighlightExtension(host: self)
                extensions.append(ext)
                if let context = makeRenderContext() {
                    ext.didMount(context: context)
                }
                return
            }
            if !editorSettings.highlightCurrentLine, hasHighlight {
                let context = makeRenderContext()
                let removed = extensions.filter { $0 is CurrentLineHighlightExtension }
                extensions.removeAll { $0 is CurrentLineHighlightExtension }
                if let context {
                    for ext in removed {
                        ext.willUnmount(context: context)
                    }
                }
            }
        }

        func applyLineWrapping(_ enabled: Bool) {
            lineWrappingEnabled = enabled
            viewportState?.lineWrappingEnabled = enabled
            guard let textView, let textContainer = textView.textContainer else { return }
            scrollView?.hasHorizontalScroller = !enabled
            if enabled {
                let availableWidth = wrappingContentWidth()
                textContainer.widthTracksTextView = true
                textContainer.containerSize = NSSize(
                    width: max(1, availableWidth),
                    height: CGFloat.greatestFiniteMagnitude
                )
                textView.maxSize = NSSize(
                    width: CGFloat.greatestFiniteMagnitude,
                    height: CGFloat.greatestFiniteMagnitude
                )
                textView.isHorizontallyResizable = false
            } else {
                textContainer.widthTracksTextView = false
                textContainer.containerSize = NSSize(
                    width: CGFloat.greatestFiniteMagnitude,
                    height: CGFloat.greatestFiniteMagnitude
                )
                textView.isHorizontallyResizable = true
            }
            textView.layoutManager?.invalidateLayout(
                forCharacterRange: NSRange(location: 0, length: textView.textStorage?.length ?? 0),
                actualCharacterRange: nil
            )
        }

        func reconcileLineWrapping(_ enabled: Bool) {
            guard lineWrappingEnabled != enabled else { return }
            reconfigureLineWrapping(enabled)
        }

        private func reconfigureLineWrapping(_ enabled: Bool) {
            guard !isReconfiguringLineWrapping else { return }
            isReconfiguringLineWrapping = true
            defer { isReconfiguringLineWrapping = false }
            deriveAnchorFromScrollView()
            applyLineWrapping(enabled)
            viewportState?.resetMeasurements()
            updateContainerHeight()
            refreshViewportPinningAnchor()
        }

        private func wrappingContentWidth() -> CGFloat {
            guard let scrollView else { return 0 }
            let total = scrollView.contentSize.width
            let inset = textView?.textContainerInset.width ?? 0
            return max(1, total - inset * 2)
        }

        private func makeRenderContext() -> EditorRenderContext? {
            guard let textView,
                  let storage = textView.textStorage,
                  let layoutManager = textView.layoutManager,
                  let viewport = viewportState,
                  let backingStore = state.backingStore
            else { return nil }
            return EditorRenderContext(
                textView: textView,
                storage: storage,
                layoutManager: layoutManager,
                viewport: viewport,
                backingStore: backingStore,
                lineStartOffsets: lineStartOffsets,
                editorSettings: editorSettings,
                state: state
            )
        }

        deinit {
            previewRefreshTask?.cancel()
            NotificationCenter.default.removeObserver(self)
        }

        func beginPerfTiming() -> CFTimeInterval? {
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

        func recordHighlightTiming(start: CFTimeInterval?, highlightedRangeCount: Int, force: Bool) {
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

        func enterViewportMode(scrollView: NSScrollView) {
            guard let store = state.backingStore, let textView else { return }
            textView.allowsUndo = false
            textView.undoManager?.removeAllActions()
            textView.usesFindBar = false
            clearViewportHistory()

            let viewport = ViewportState(backingStore: store)
            viewport.updateEstimatedLineHeight(font: editorSettings.resolvedFont)
            viewport.lineWrappingEnabled = lineWrappingEnabled
            viewportState = viewport
            invalidateRenderedViewportText()
            lastRenderedViewportRange = nil
            lastRenderedBackingStoreVersion = -1
            lastObservedClipSize = scrollView.contentView.bounds.size

            textView.isVerticallyResizable = false
            textView.autoresizingMask = []

            let container = ViewportContainerView()
            container.wantsLayer = true
            container.layer?.backgroundColor = EditorThemePalette.active.background.cgColor
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

            if let context = makeRenderContext() {
                for ext in extensions {
                    ext.didMount(context: context)
                }
            }
        }

        func updateContainerHeight() {
            guard let viewport = viewportState, let container = containerView, let scrollView else { return }
            let height = max(viewport.totalDocumentHeight, scrollView.contentView.bounds.height)
            let width = max(scrollView.contentSize.width, textView?.frame.width ?? scrollView.contentSize.width)
            container.frame = NSRect(x: 0, y: 0, width: width, height: height)
            updateMarkdownEditorScrollMetrics()
            let maxScrollY = max(0, height - scrollView.contentView.bounds.height)
            if scrollView.contentView.bounds.origin.y > maxScrollY {
                scrollView.contentView.setBoundsOrigin(NSPoint(x: scrollView.contentView.bounds.origin.x, y: maxScrollY))
                scrollView.reflectScrolledClipView(scrollView.contentView)
            }
        }

        func setScrollAnchor(_ anchor: ScrollAnchor) {
            guard let viewport = viewportState else { return }
            scrollAnchor = anchor.clamped(toLineCount: viewport.backingStore.lineCount)
            writeAnchorToScrollView()
        }

        private func writeAnchorToScrollView() {
            guard let viewport = viewportState, let scrollView else { return }
            let visibleHeight = scrollView.contentView.bounds.height
            let containerHeight = max(viewport.totalDocumentHeight, visibleHeight)
            let maxScrollY = max(0, containerHeight - visibleHeight)
            let target = scrollAnchor.pixelY(in: viewport.heightMap)
            let clamped = min(maxScrollY, max(0, target))
            let current = scrollView.contentView.bounds.origin.y
            guard abs(clamped - current) >= 0.5 else { return }
            isWritingScrollProgrammatically = true
            scrollView.contentView.setBoundsOrigin(NSPoint(x: scrollView.contentView.bounds.origin.x, y: clamped))
            scrollView.reflectScrolledClipView(scrollView.contentView)
            isWritingScrollProgrammatically = false
        }

        private func deriveAnchorFromScrollView() {
            guard let viewport = viewportState, let scrollView else { return }
            let pixel = scrollView.contentView.bounds.origin.y
            scrollAnchor = ScrollAnchor.from(pixelY: pixel, in: viewport.heightMap)
        }

        func refreshViewport(force: Bool) {
            refreshViewport(force: force, pinAnchor: false)
        }

        func refreshViewportPinningAnchor() {
            refreshViewport(force: true, pinAnchor: true)
        }

        private func refreshViewport(force: Bool, pinAnchor: Bool) {
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
                storage.addAttribute(.foregroundColor, value: EditorThemePalette.active.foreground, range: fullRange)
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

            notifyGeometryDidChange()

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
            if pinAnchor {
                writeAnchorToScrollView()
            } else {
                deriveAnchorFromScrollView()
            }
        }

        func applySyntaxHighlights(storage _: NSTextStorage, viewport: ViewportState) {
            guard let context = makeRenderContext() else { return }
            let lineRange = viewport.viewportStartLine ..< viewport.viewportEndLine
            for ext in extensions {
                ext.renderViewport(context: context, lineRange: lineRange)
            }
        }

        func invalidateSyntaxHighlightsFromLine(_ line: Int) {
            state.syntaxHighlighter?.invalidate(fromLine: max(0, line))
        }

        func reapplySyntaxHighlights() {
            guard let viewport = viewportState, let storage = textView?.textStorage else { return }
            applySyntaxHighlights(storage: storage, viewport: viewport)
        }

        func applyIncrementalSyntaxHighlights(
            startLine: Int,
            oldLineCount: Int,
            newLineCount: Int
        ) {
            guard let context = makeRenderContext() else { return }
            let lineRange = startLine ..< startLine + newLineCount
            let edit = EditorTextEdit(
                startLine: startLine,
                oldLineCount: oldLineCount,
                newLineCount: newLineCount
            )
            for ext in extensions {
                ext.applyIncremental(context: context, lineRange: lineRange, edit: edit)
            }
        }

        func scheduleSyntaxCascadeReapply() {
            pendingCascadeReapplyGeneration &+= 1
            let generation = pendingCascadeReapplyGeneration
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(16)) { [weak self] in
                guard let self, self.pendingCascadeReapplyGeneration == generation else { return }
                self.reapplySyntaxHighlights()
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

        func focusEditorPreservingSelection() {
            guard let textView else { return }
            let matches = searchController.matches
            if let viewport = viewportState, !matches.isEmpty {
                let currentIndex = max(0, state.searchCurrentIndex - 1)
                if currentIndex < matches.count {
                    let match = matches[currentIndex]
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

        func clearSearchHighlights() {
            searchController.clearHighlights()
        }

        func applySearchHighlights(force: Bool = false) {
            searchController.applyHighlights(force: force)
        }

        func performSearchViewport(_ needle: String, caseSensitive: Bool, useRegex: Bool) {
            searchController.performSearch(needle, caseSensitive: caseSensitive, useRegex: useRegex)
        }

        func navigateSearchViewport(forward: Bool) {
            searchController.navigate(forward: forward)
        }

        func replaceCurrentViewport(with replacement: String, needle: String, caseSensitive: Bool, useRegex: Bool) {
            searchController.replaceCurrent(with: replacement, needle: needle, caseSensitive: caseSensitive, useRegex: useRegex)
        }

        func replaceAllViewport(with replacement: String, needle: String, caseSensitive: Bool, useRegex: Bool) {
            searchController.replaceAll(with: replacement, needle: needle, caseSensitive: caseSensitive, useRegex: useRegex)
        }

        func charOffsetForLocalLine(_ localLine: Int) -> Int {
            guard localLine >= 0, localLine < lineStartOffsets.count else { return 0 }
            return lineStartOffsets[localLine]
        }

        private func measureWrappedFragments(textView: NSTextView, viewport: ViewportState) {
            guard let layoutManager = textView.layoutManager,
                  let textContainer = textView.textContainer,
                  let storage = textView.textStorage,
                  storage.length > 0
            else { return }
            layoutManager.ensureLayout(for: textContainer)
            let nsString = storage.string as NSString
            let storageLength = nsString.length
            let estimatedLineHeight = viewport.estimatedLineHeight
            var lineHeights: [CGFloat] = []
            lineHeights.reserveCapacity(viewport.viewportLineCount)
            var location = 0
            var localLine = 0
            while location <= storageLength, localLine < viewport.viewportLineCount {
                let lineRange = nsString.lineRange(for: NSRange(location: location, length: 0))
                let trimmedLength = lineRange.length - trailingNewlineLength(in: nsString, range: lineRange)
                let contentRange = NSRange(location: lineRange.location, length: max(0, trimmedLength))
                let glyphRange = layoutManager.glyphRange(forCharacterRange: contentRange, actualCharacterRange: nil)
                let measuredHeight: CGFloat
                if glyphRange.length == 0 {
                    measuredHeight = estimatedLineHeight
                } else {
                    let bounding = layoutManager.boundingRect(forGlyphRange: glyphRange, in: textContainer)
                    measuredHeight = max(estimatedLineHeight, bounding.height)
                }
                lineHeights.append(measuredHeight)
                location = NSMaxRange(lineRange)
                localLine += 1
            }
            guard !lineHeights.isEmpty else { return }
            viewport.recordMeasuredLineHeights(startLine: viewport.viewportStartLine, lineHeights: lineHeights)
        }

        private func trailingNewlineLength(in string: NSString, range: NSRange) -> Int {
            guard range.length > 0 else { return 0 }
            let lastIndex = NSMaxRange(range) - 1
            let lastChar = string.character(at: lastIndex)
            if lastChar == 0x0A {
                if range.length >= 2, string.character(at: lastIndex - 1) == 0x0D {
                    return 2
                }
                return 1
            }
            if lastChar == 0x0D { return 1 }
            return 0
        }

        private func viewportContentWidth(for textView: NSTextView, scrollView: NSScrollView) -> CGFloat {
            if lineWrappingEnabled {
                return scrollView.contentSize.width
            }
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
            let targetTextWidth = max(0, viewportWidth)

            if lineWrappingEnabled, textView.frame.width != targetTextWidth, targetTextWidth > 0 {
                textView.frame = NSRect(
                    x: 0,
                    y: textView.frame.origin.y,
                    width: targetTextWidth,
                    height: textView.frame.height
                )
                if let textContainer = textView.textContainer {
                    textContainer.containerSize = NSSize(
                        width: targetTextWidth,
                        height: CGFloat.greatestFiniteMagnitude
                    )
                }
                textView.layoutManager?.invalidateLayout(
                    forCharacterRange: NSRange(location: 0, length: textView.textStorage?.length ?? 0),
                    actualCharacterRange: nil
                )
            }

            if let layoutManager = textView.layoutManager, let textContainer = textView.textContainer {
                layoutManager.ensureLayout(for: textContainer)
            }
            let laidOutHeight: CGFloat = if let layoutManager = textView.layoutManager, let textContainer = textView.textContainer {
                ceil(layoutManager.usedRect(for: textContainer).height + textView.textContainerInset.height * 2)
            } else {
                estimatedHeight
            }
            if lineWrappingEnabled, targetTextWidth > 0 {
                viewport.updateContainerWidth(targetTextWidth)
                measureWrappedFragments(textView: textView, viewport: viewport)
            }
            let resolvedYOffset = viewport.viewportYOffset()
            let newTextFrame = NSRect(
                x: 0,
                y: resolvedYOffset,
                width: targetTextWidth,
                height: max(estimatedHeight, laidOutHeight, 100)
            )
            if textView.frame != newTextFrame {
                textView.frame = newTextFrame
            }

            if let container = containerView {
                let containerHeight = max(
                    viewport.totalDocumentHeight,
                    scrollView.contentView.bounds.height
                )
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
            updateMarkdownEditorScrollMetrics()
            updateMarkdownPreviewSyncPointFromEditorScroll()
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
            reconcileScrollBoundsChange(observedContentView?.bounds.size)
        }

        @objc
        private func handleClipFrameChange() {
            reconcileClipFrameChange(observedContentView?.frame.size)
        }

        private func reconcileScrollBoundsChange(_ size: CGSize?) {
            var widthChanged = false
            if let size {
                if size.width != lastObservedClipSize.width {
                    widthChanged = true
                    handleWrappingWidthChange()
                    ensureViewportMinimumWidth()
                }
                if size.height != lastObservedClipSize.height {
                    updateContainerHeight()
                }
                lastObservedClipSize = size
            }
            if !isWritingScrollProgrammatically {
                deriveAnchorFromScrollView()
            }
            markdownScrollSync.attach(scrollView: scrollView, viewport: viewportState)
            markdownScrollSync.reconcileScrollBoundsChange()
            if !isEditingViewport {
                refreshViewport(force: lineWrappingEnabled && widthChanged && !isLiveResizing)
            }
        }

        private func reconcileClipFrameChange(_ size: CGSize?) {
            var widthChanged = false
            if let size {
                if size.width != lastObservedClipSize.width {
                    widthChanged = true
                    handleWrappingWidthChange()
                    ensureViewportMinimumWidth()
                }
                if size.height != lastObservedClipSize.height {
                    updateContainerHeight()
                }
                lastObservedClipSize = size
            }
            updateMarkdownEditorScrollMetrics()
            if !isEditingViewport {
                refreshViewport(force: lineWrappingEnabled && widthChanged && !isLiveResizing)
            }
        }

        private var isLiveResizing: Bool {
            scrollView?.inLiveResize == true
        }

        private func handleWrappingWidthChange() {
            guard lineWrappingEnabled else { return }
            if isLiveResizing {
                schedulePendingWrapResize()
                return
            }
            reconfigureLineWrapping(true)
        }

        private func schedulePendingWrapResize() {
            pendingWrapResizeWorkItem?.cancel()
            let work = DispatchWorkItem { [weak self] in
                MainActor.assumeIsolated {
                    guard let self else { return }
                    self.pendingWrapResizeWorkItem = nil
                    if self.scrollView?.inLiveResize == true {
                        self.schedulePendingWrapResize()
                        return
                    }
                    guard self.lineWrappingEnabled else { return }
                    self.reconfigureLineWrapping(true)
                }
            }
            pendingWrapResizeWorkItem = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.12, execute: work)
        }

        func updateMarkdownEditorScrollMetrics() {
            markdownScrollSync.attach(scrollView: scrollView, viewport: viewportState)
            markdownScrollSync.updateEditorScrollMetrics()
        }

        func syncMarkdownScrollPositionIfNeeded() {
            markdownScrollSync.attach(scrollView: scrollView, viewport: viewportState)
            markdownScrollSync.syncScrollPositionIfNeeded(
                refreshViewport: { [weak self] in self?.refreshViewport(force: true) },
                rebuildLineStartOffsets: { [weak self] in self?.rebuildLineStartOffsetsForViewport() }
            )
        }

        func updateMarkdownPreviewSyncPointFromEditorScroll() {
            markdownScrollSync.attach(scrollView: scrollView, viewport: viewportState)
            markdownScrollSync.updatePreviewSyncPointFromEditorScroll()
        }

        private func publishMarkdownProgressIfEditorAutoScrolled(_ work: () -> Void) {
            markdownScrollSync.attach(scrollView: scrollView, viewport: viewportState)
            markdownScrollSync.publishProgressIfEditorAutoScrolled(work)
        }

        func textDidChange(_: Notification) {
            guard let textView, !isUpdating else { return }
            handleTextDidChangeViewport(textView)
            scheduleMarkdownPreviewRefresh()
        }

        func scheduleMarkdownPreviewRefresh(immediate: Bool = false) {
            guard state.isMarkdownFile else { return }
            previewRefreshTask?.cancel()
            if immediate {
                state.previewRefreshVersion += 1
                return
            }
            let task = Task { @MainActor [weak self] in
                try? await Task.sleep(nanoseconds: Self.previewRefreshDebounceNanos)
                guard !Task.isCancelled, let self else { return }
                guard self.previewRefreshTask?.isCancelled == false else { return }
                self.state.previewRefreshVersion += 1
            }
            previewRefreshTask = task
        }

        private func handleTextDidChangeViewport(_ textView: NSTextView) {
            guard let viewport = viewportState, let scrollView else { return }
            let pendingEdit = history.pendingEdit
            history.pendingEdit = nil
            let cursorLocation = textView.selectedRange().location
            let viewportStartLine = viewport.viewportStartLine
            var lineDelta = 0
            var recordedViewportEdit = false

            if let pendingEdit {
                let oldRange = pendingEdit.startLine ..< pendingEdit.startLine + pendingEdit.oldLines.count
                _ = viewport.backingStore.replaceLines(in: oldRange, with: pendingEdit.newLines)
                viewport.notifyLinesReplaced(
                    start: pendingEdit.startLine,
                    removingCount: pendingEdit.oldLines.count,
                    insertingLineCharCounts: pendingEdit.newLines.map { ($0 as NSString).length }
                )
                lineDelta = pendingEdit.newLines.count - pendingEdit.oldLines.count
                let newViewportEnd = max(viewportStartLine, viewport.viewportEndLine + lineDelta)
                viewport.applyViewport(viewportStartLine ..< newViewportEnd)
            } else {
                if !history.isApplyingHistory {
                    clearViewportHistory()
                }
                let newLocalText = textView.string
                let newLocalLines = newLocalText.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
                let oldRange = viewport.viewportStartLine ..< viewport.viewportEndLine
                _ = viewport.backingStore.replaceLines(in: oldRange, with: newLocalLines)
                viewport.notifyLinesReplaced(
                    start: viewport.viewportStartLine,
                    removingCount: oldRange.count,
                    insertingLineCharCounts: newLocalLines.map { ($0 as NSString).length }
                )
                lineDelta = newLocalLines.count - oldRange.count
                viewport.applyViewport(viewport.viewportStartLine ..< viewport.viewportStartLine + newLocalLines.count)
                invalidateSyntaxHighlightsFromLine(viewportStartLine)
            }

            state.backingStoreVersion += 1
            lastSyncedBackingStoreVersion = state.backingStoreVersion
            state.markModified()

            isEditingViewport = true
            defer { isEditingViewport = false }

            lastRenderedViewportRange = viewport.viewportStartLine ..< viewport.viewportEndLine
            lastRenderedBackingStoreVersion = state.backingStoreVersion
            needsViewportTextReload = false
            rebuildLineStartOffsetsForViewport()

            if let pendingEdit,
               !history.isApplyingHistory,
               let selectionAfter = globalCursorFromLocalLocation(cursorLocation)
            {
                history.push(ViewportEdit(
                    startLine: pendingEdit.startLine,
                    oldLines: pendingEdit.oldLines,
                    newLines: pendingEdit.newLines,
                    selectionBefore: pendingEdit.selectionBefore,
                    selectionAfter: selectionAfter
                ))
                recordedViewportEdit = true
            }

            if pendingEdit != nil, !recordedViewportEdit, !history.isApplyingHistory {
                clearViewportHistory()
            }

            if lineDelta != 0 || lineWrappingEnabled || state.isMarkdownFile {
                updateContainerHeight()
                updateViewportFrames(
                    viewport: viewport,
                    textView: textView,
                    scrollView: scrollView,
                    yOffset: viewport.viewportYOffset(),
                    visibleLineCount: max(1, viewport.viewportLineCount)
                )
                notifyGeometryDidChange()
            }

            publishMarkdownProgressIfEditorAutoScrolled {
                scrollCursorVisibleInViewport(textView: textView, cursorLocation: cursorLocation)
            }

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
                    publishMarkdownProgressIfEditorAutoScrolled {
                        scrollCursorVisibleInViewport(textView: textView, cursorLocation: safeCursor)
                    }
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
            history.clear()
        }

        func performUndoRequest() -> Bool {
            if viewportState != nil {
                return history.performUndo()
            }
            guard let textView, textView.undoManager?.canUndo == true else { return false }
            textView.undoManager?.undo()
            return true
        }

        func performRedoRequest() -> Bool {
            if viewportState != nil {
                return history.performRedo()
            }
            guard let textView, textView.undoManager?.canRedo == true else { return false }
            textView.undoManager?.redo()
            return true
        }

        func canPerformUndoRequest() -> Bool {
            if viewportState != nil {
                return history.canUndo
            }
            return textView?.undoManager?.canUndo ?? false
        }

        func canPerformRedoRequest() -> Bool {
            if viewportState != nil {
                return history.canRedo
            }
            return textView?.undoManager?.canRedo ?? false
        }

        func applyHistorySelection(_ selection: ViewportCursor) {
            updateContainerHeight()
            scrollToGlobalLine(selection.line, column: selection.column)
        }

        func adjustViewportRangeForReplacement(
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

        private func captureViewportPendingEdit(
            textView: NSTextView,
            affectedCharRange: NSRange,
            replacementString: String?
        ) {
            history.pendingEdit = nil
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

            history.pendingEdit = PendingViewportEdit(
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
            cursorRect.origin.y += textView.textContainerOrigin.y + textView.frame.origin.y

            let clipBounds = scrollView.contentView.bounds
            let visibleMinY = clipBounds.origin.y
            let visibleMaxY = visibleMinY + clipBounds.height

            if cursorRect.maxY > visibleMaxY {
                let newY = cursorRect.maxY - clipBounds.height
                scrollView.contentView.setBoundsOrigin(NSPoint(x: clipBounds.origin.x, y: newY))
                scrollView.reflectScrolledClipView(scrollView.contentView)
            } else if cursorRect.origin.y < visibleMinY {
                scrollView.contentView.setBoundsOrigin(NSPoint(x: clipBounds.origin.x, y: cursorRect.origin.y))
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
            if replacementString?.contains("\n") == true {
                scheduleMarkdownPreviewRefresh(immediate: true)
            }
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

            notifySelectionDidChange()

            updateCurrentSelection(in: textView, range: range)
        }

        private func notifySelectionDidChange() {
            guard let context = makeRenderContext() else { return }
            for ext in extensions {
                ext.selectionDidChange(context: context)
            }
        }

        private func notifyGeometryDidChange() {
            guard let context = makeRenderContext() else { return }
            for ext in extensions {
                ext.geometryDidChange(context: context)
            }
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

        func applyPendingJump(line: Int, column: Int) {
            scrollToGlobalLine(max(0, line - 1), column: max(0, column - 1))
        }

        private func scrollToGlobalLine(_ globalLine: Int, column: Int) {
            guard let viewport = viewportState, let scrollView, let textView else { return }

            let visibleHeight = scrollView.contentView.bounds.height
            let lineCount = viewport.backingStore.lineCount
            guard lineCount > 0 else { return }
            let targetLine = max(0, min(globalLine, lineCount - 1))

            if !isGlobalLineFullyVisible(targetLine, viewport: viewport, scrollView: scrollView) {
                setScrollAnchor(ScrollAnchor(line: targetLine, deltaPixels: -visibleHeight / 3))
            }

            for _ in 0 ..< 5 {
                let pixelBefore = scrollAnchor.pixelY(in: viewport.heightMap)
                refreshViewportPinningAnchor()
                let pixelAfter = scrollAnchor.pixelY(in: viewport.heightMap)
                if abs(pixelAfter - pixelBefore) < 0.5 { break }
            }

            rebuildLineStartOffsetsForViewport()

            guard let newLocalLine = viewport.viewportLine(forBackingStoreLine: targetLine) else { return }
            let newCharOffset = charOffsetForLocalLine(newLocalLine)
            let newContent = textView.string as NSString
            let lineRange = newContent.lineRange(for: NSRange(location: min(newCharOffset, newContent.length), length: 0))
            let lineLength = lineRange.length - (NSMaxRange(lineRange) < newContent.length ? 1 : 0)
            let newCursor = newCharOffset + min(column, max(0, lineLength))
            let safeCursor = min(newCursor, newContent.length)

            isUpdating = true
            textView.setSelectedRange(NSRange(location: safeCursor, length: 0))
            isUpdating = false

            state.cursorLine = targetLine + 1
            let safeLocalLine = max(0, min(newLocalLine, lineStartOffsets.count - 1))
            let cursorLineStart = lineStartOffsets[safeLocalLine]
            state.cursorColumn = max(1, safeCursor - cursorLineStart + 1)
            notifySelectionDidChange()
        }

        private func isGlobalLineFullyVisible(_ line: Int, viewport: ViewportState, scrollView: NSScrollView) -> Bool {
            let lineTop = viewport.heightMap.heightAbove(line: line)
            let lineHeight = viewport.heightMap.heightOfLine(line)
            let visibleTop = scrollView.contentView.bounds.origin.y
            let visibleBottom = visibleTop + scrollView.contentView.bounds.height
            return lineTop >= visibleTop && lineTop + lineHeight <= visibleBottom
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
