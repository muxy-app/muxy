import AppKit
import SwiftUI

struct CodeEditorView: NSViewRepresentable {
    @Bindable var state: EditorTabState
    let themeVersion: Int
    let searchNeedle: String
    let searchNavigationVersion: Int
    let searchNavigationDirection: EditorSearchNavigationDirection

    func makeCoordinator() -> Coordinator {
        Coordinator(state: state)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        guard let textView = scrollView.documentView as? NSTextView else { return scrollView }

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
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.lineFragmentPadding = 8
        textView.textContainerInset = NSSize(width: 0, height: 4)

        let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
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

        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder

        textView.delegate = context.coordinator
        context.coordinator.textView = textView

        return scrollView
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

        if contentChanged || themeChanged {
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
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        let state: EditorTabState
        weak var textView: NSTextView?
        var isUpdating = false
        var lastThemeVersion = -1
        var lastSearchNeedle = ""
        var lastSearchNavigationVersion = -1
        private var highlightDebounceTask: DispatchWorkItem?

        init(state: EditorTabState) {
            self.state = state
            super.init()
        }

        func textDidChange(_: Notification) {
            guard let textView, !isUpdating else { return }
            isUpdating = true
            state.content = textView.string
            state.markModified()
            scheduleHighlighting()
            isUpdating = false
        }

        private func scheduleHighlighting() {
            highlightDebounceTask?.cancel()
            let task = DispatchWorkItem { [weak self] in
                self?.applyHighlighting()
            }
            highlightDebounceTask = task
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.15, execute: task)
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
        }

        func applyHighlighting() {
            guard let textView, let storage = textView.textStorage else { return }
            guard storage.length > 0 else { return }
            let scrollPos = textView.enclosingScrollView?.contentView.bounds.origin
            let fullRange = NSRange(location: 0, length: storage.length)
            let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
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
                || trimmed.hasSuffix("[") || trimmed.hasSuffix(":") || trimmed.hasSuffix("->")
            let indent = extra ? leading + "    " : leading
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
