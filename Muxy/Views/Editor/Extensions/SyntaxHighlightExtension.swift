import AppKit

@MainActor
final class SyntaxHighlightExtension: EditorExtension {
    let identifier = "syntax-highlight"

    private weak var coordinator: SyntaxHighlightCoordinator?

    init(coordinator: SyntaxHighlightCoordinator) {
        self.coordinator = coordinator
    }

    func renderViewport(context: EditorRenderContext, lineRange: Range<Int>) {
        guard let highlighter = context.state.syntaxHighlighter else { return }
        let storage = context.storage
        let storageLength = storage.length
        guard storageLength > 0, !lineRange.isEmpty else { return }

        let spans = highlighter.spans(
            in: lineRange,
            lineStartOffsets: context.lineStartOffsets,
            backingStore: context.backingStore
        )

        let fullRange = NSRange(location: 0, length: storageLength)
        context.layoutManager.removeTemporaryAttribute(.foregroundColor, forCharacterRange: fullRange)
        for span in spans {
            let availableLength = storageLength - span.range.location
            guard span.range.location >= 0, availableLength > 0 else { continue }
            let clampedLength = min(span.range.length, availableLength)
            guard clampedLength > 0 else { continue }
            context.layoutManager.addTemporaryAttribute(
                .foregroundColor,
                value: SyntaxTheme.color(for: span.scope),
                forCharacterRange: NSRange(location: span.range.location, length: clampedLength)
            )
        }
    }

    func applyIncremental(context: EditorRenderContext, lineRange: Range<Int>, edit: EditorTextEdit) {
        guard let highlighter = context.state.syntaxHighlighter else { return }
        let storage = context.storage
        let storageLength = storage.length
        guard storageLength > 0 else { return }

        let outcome = highlighter.applyEdit(
            startLine: edit.startLine,
            oldLineCount: edit.oldLineCount,
            newLineCount: edit.newLineCount,
            backingStore: context.backingStore
        )

        let viewportStart = context.viewport.viewportStartLine
        let localStart = max(0, lineRange.lowerBound - viewportStart)
        let localEnd = min(lineRange.upperBound - viewportStart, context.viewport.viewportLineCount)
        guard localStart < localEnd, localStart < context.lineStartOffsets.count else {
            handleCascade(outcome: outcome)
            return
        }

        let charStart = context.lineStartOffsets[localStart]
        let charEnd: Int = localEnd < context.lineStartOffsets.count
            ? context.lineStartOffsets[localEnd]
            : storageLength
        guard charEnd > charStart, charStart >= 0, charEnd <= storageLength else {
            handleCascade(outcome: outcome)
            return
        }

        let editedRange = NSRange(location: charStart, length: charEnd - charStart)
        context.layoutManager.removeTemporaryAttribute(.foregroundColor, forCharacterRange: editedRange)
        for localIndex in localStart ..< localEnd {
            let globalLine = viewportStart + localIndex
            guard let tokens = highlighter.tokens(forLine: globalLine) else { continue }
            let lineOffset = context.lineStartOffsets[localIndex]
            for token in tokens {
                let location = lineOffset + token.location
                guard location >= 0, location + token.length <= storageLength else { continue }
                context.layoutManager.addTemporaryAttribute(
                    .foregroundColor,
                    value: SyntaxTheme.color(for: token.scope),
                    forCharacterRange: NSRange(location: location, length: token.length)
                )
            }
        }

        handleCascade(outcome: outcome)
    }

    private func handleCascade(outcome: SyntaxHighlighter.EditOutcome) {
        guard case .cascade = outcome else { return }
        coordinator?.scheduleSyntaxCascadeReapply()
    }
}

@MainActor
protocol SyntaxHighlightCoordinator: AnyObject {
    func scheduleSyntaxCascadeReapply()
}
