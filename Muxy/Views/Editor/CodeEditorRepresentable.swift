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
    let searchNeedle: String
    let searchNavigationVersion: Int
    let searchNavigationDirection: EditorSearchNavigationDirection
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
        textView.isRichText = true
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
        context.coordinator.setScrollObserver(for: scrollView, onLineLayoutChange: onLineLayoutChange)

        return scrollView
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

        let contentChanged = !coordinator.isUpdating && textView.string != state.content
        if contentChanged {
            coordinator.isUpdating = true
            textView.string = state.content
            coordinator.isUpdating = false
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
        coordinator.showInvisibles = editorSettings.showInvisibles

        if contentChanged || themeChanged || fontChanged {
            coordinator.applyHighlighting()
        }

        if coordinator.lastSearchNeedle != searchNeedle {
            coordinator.lastSearchNeedle = searchNeedle
            coordinator.performSearch(searchNeedle)
        }

        if coordinator.lastSearchNavigationVersion != searchNavigationVersion {
            coordinator.lastSearchNavigationVersion = searchNavigationVersion
            coordinator.navigateSearch(forward: searchNavigationDirection == .next)
        }

        coordinator.onLineLayoutChange = onLineLayoutChange

        if contentChanged || themeChanged || fontChanged || wrapChanged {
            coordinator.invalidateAndReportLayouts()
        } else {
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
        var lastThemeVersion = -1
        var lastSearchNeedle = ""
        var lastSearchNavigationVersion = -1
        var lastWordWrap = true
        var tabSize = 4
        var showInvisibles = false
        var onLineLayoutChange: ([LineLayoutInfo]) -> Void = { _ in }
        private weak var observedContentView: NSClipView?
        private weak var observedTextView: NSTextView?
        private var lastReportedLayouts: [LineLayoutInfo] = []
        private let lineHighlightView: NSView = {
            let view = NSView()
            view.wantsLayer = true
            view.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.06).cgColor
            return view
        }()

        init(state: EditorTabState, editorSettings: EditorSettings) {
            self.state = state
            self.editorSettings = editorSettings
            self.lastWordWrap = editorSettings.wordWrap
            self.tabSize = editorSettings.tabSize
            self.showInvisibles = editorSettings.showInvisibles
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
            updateLineHighlight()
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
            var lineRect = layoutManager.boundingRect(forGlyphRange: glyphRange, in: textContainer)
            lineRect.origin.x = 0
            lineRect.origin.y += textView.textContainerOrigin.y
            lineRect.size.width = max(textView.bounds.width, textView.enclosingScrollView?.contentSize.width ?? 0)

            lineHighlightView.frame = lineRect
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

            var lineNumber = 1
            var index = 0
            while index < visibleCharRange.location {
                let lineRange = content.lineRange(for: NSRange(location: index, length: 0))
                index = NSMaxRange(lineRange)
                lineNumber += 1
            }

            var layouts: [LineLayoutInfo] = []
            index = visibleCharRange.location
            while index <= NSMaxRange(visibleCharRange), index < content.length {
                let lineRange = content.lineRange(for: NSRange(location: index, length: 0))
                let glyphRange = layoutManager.glyphRange(forCharacterRange: lineRange, actualCharacterRange: nil)
                let lineRect = layoutManager.boundingRect(forGlyphRange: glyphRange, in: textContainer)

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
            state.content = textView.string
            state.markModified()
            isUpdating = false
            reportLineLayouts()
        }

        func textViewDidChangeSelection(_: Notification) {
            guard let textView else { return }
            let range = textView.selectedRange()
            let str = textView.string
            let loc = min(range.location, str.count)
            let index = str.index(str.startIndex, offsetBy: loc)
            let lineRange = str.lineRange(for: index ..< index)
            state.cursorLine = str[str.startIndex ..< lineRange.lowerBound].count(where: { $0 == "\n" }) + 1
            state.cursorColumn = str.distance(from: lineRange.lowerBound, to: index) + 1
            updateLineHighlight()
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
        }

        private var searchMatches: [NSRange] = []

        func performSearch(_ needle: String) {
            guard let textView else { return }
            searchMatches = []
            guard !needle.isEmpty else {
                state.searchMatchCount = 0
                state.searchCurrentIndex = 0
                return
            }
            let content = textView.string as NSString
            var searchRange = NSRange(location: 0, length: content.length)
            while searchRange.location < content.length {
                let found = content.range(of: needle, options: .caseInsensitive, range: searchRange)
                guard found.location != NSNotFound else { break }
                searchMatches.append(found)
                searchRange.location = found.location + found.length
                searchRange.length = content.length - searchRange.location
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

        @objc
        func handleReturn(_ textView: NSTextView) -> Bool {
            textView.breakUndoCoalescing()
            let content = textView.string
            let range = textView.selectedRange()
            let loc = min(range.location, content.count)
            let index = content.index(content.startIndex, offsetBy: loc)
            let lineRange = content.lineRange(for: index ..< index)
            let lineText = String(content[lineRange.lowerBound ..< index])
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
            guard range.location > 0 else { return false }
            textView.breakUndoCoalescing()

            let nsContent = content as NSString
            let cursorPos = range.location
            let charBefore = nsContent.character(at: cursorPos - 1)

            if charBefore == 0x0A {
                textView.replaceCharacters(in: NSRange(location: cursorPos - 1, length: 1), with: "")
                return true
            }

            let scalar = Unicode.Scalar(charBefore)
            if let scalar, CharacterSet.punctuationCharacters.union(.symbols).contains(scalar) {
                textView.replaceCharacters(in: NSRange(location: cursorPos - 1, length: 1), with: "")
                return true
            }

            let lineRange = nsContent.lineRange(for: NSRange(location: cursorPos, length: 0))
            let lineStart = lineRange.location
            let textBeforeCursor = nsContent.substring(with: NSRange(location: lineStart, length: cursorPos - lineStart))

            if textBeforeCursor.allSatisfy({ $0 == " " || $0 == "\t" }) {
                textView.replaceCharacters(in: NSRange(location: lineStart, length: cursorPos - lineStart), with: "")
                return true
            }

            return false
        }
    }
}
