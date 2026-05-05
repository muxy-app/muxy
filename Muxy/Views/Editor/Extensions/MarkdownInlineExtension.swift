import AppKit

@MainActor
final class MarkdownInlineExtension: EditorExtension {
    let identifier = "markdown-inline"

    func renderViewport(context: EditorRenderContext, lineRange: Range<Int>) {
        applyDecorations(context: context, lineRange: lineRange)
    }

    func applyIncremental(context: EditorRenderContext, lineRange: Range<Int>, edit _: EditorTextEdit) {
        applyDecorations(context: context, lineRange: lineRange)
    }

    private func applyDecorations(context: EditorRenderContext, lineRange: Range<Int>) {
        guard let highlighter = context.state.syntaxHighlighter else { return }
        let storage = context.storage
        let storageLength = storage.length
        guard storageLength > 0 else { return }

        let viewportStart = context.viewport.viewportStartLine
        let localStart = max(0, lineRange.lowerBound - viewportStart)
        let localEnd = min(lineRange.upperBound - viewportStart, context.viewport.viewportLineCount)
        guard localStart < localEnd, localStart < context.lineStartOffsets.count else { return }

        let charStart = context.lineStartOffsets[localStart]
        let charEnd: Int = localEnd < context.lineStartOffsets.count
            ? context.lineStartOffsets[localEnd]
            : storageLength
        guard charEnd > charStart, charStart >= 0, charEnd <= storageLength else { return }

        let editedRange = NSRange(location: charStart, length: charEnd - charStart)
        context.layoutManager.removeTemporaryAttribute(.strikethroughStyle, forCharacterRange: editedRange)

        for localIndex in localStart ..< localEnd {
            let globalLine = viewportStart + localIndex
            guard globalLine < context.backingStore.lineCount else { break }
            let lineText = context.backingStore.line(at: globalLine)
            let isInsideFence = if case .inBlockComment = highlighter.lineStartState(at: globalLine) {
                true
            } else {
                false
            }
            let decorations = MarkdownInlineHighlighter.decorations(
                line: lineText,
                isInsideFencedCode: isInsideFence
            )
            guard !decorations.isEmpty else { continue }
            let lineOffset = context.lineStartOffsets[localIndex]
            for decoration in decorations {
                let location = lineOffset + decoration.range.location
                let length = decoration.range.length
                guard location >= 0, length > 0, location + length <= storageLength else { continue }
                let nsRange = NSRange(location: location, length: length)
                if let color = MarkdownInlineStyle.foregroundColor(for: decoration.kind) {
                    context.layoutManager.addTemporaryAttribute(
                        .foregroundColor,
                        value: color,
                        forCharacterRange: nsRange
                    )
                }
                if let strike = MarkdownInlineStyle.strikethroughStyle(for: decoration.kind) {
                    context.layoutManager.addTemporaryAttribute(
                        .strikethroughStyle,
                        value: strike.rawValue,
                        forCharacterRange: nsRange
                    )
                }
            }
        }
    }
}
