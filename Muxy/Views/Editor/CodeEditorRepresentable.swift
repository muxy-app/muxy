import AppKit
import SwiftUI

struct CodeEditorView: NSViewRepresentable {
    @Bindable var state: EditorTabState
    let themeVersion: Int

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
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        let state: EditorTabState
        weak var textView: NSTextView?
        var isUpdating = false
        var lastThemeVersion = -1

        init(state: EditorTabState) {
            self.state = state
        }

        func textDidChange(_: Notification) {
            guard let textView, !isUpdating else { return }
            isUpdating = true
            state.content = textView.string
            state.markModified()
            applyHighlighting()
            isUpdating = false
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
            let fullRange = NSRange(location: 0, length: storage.length)
            let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
            storage.beginEditing()
            storage.addAttribute(.font, value: font, range: fullRange)
            storage.addAttribute(.foregroundColor, value: GhosttyService.shared.foregroundColor, range: fullRange)
            SyntaxHighlightExtension(fileExtension: state.fileExtension)
                .applyTextAttributes(to: storage, fullRange: fullRange)
            storage.endEditing()
            textView.needsDisplay = true
        }

        @objc
        func handleReturn(_ textView: NSTextView) -> Bool {
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
            guard commandSelector == #selector(NSResponder.insertNewline(_:)),
                  let textView
            else { return false }
            return handleReturn(textView)
        }
    }
}
