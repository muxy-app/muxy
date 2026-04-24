import Foundation

@MainActor
final class SyntaxHighlighter {
    struct AppliedSpan {
        let range: NSRange
        let scope: SyntaxScope
    }

    struct LineTokens {
        let tokens: [TokenSpan]
        let endState: LineEndState
    }

    enum EditOutcome {
        case updated
        case cascade
    }

    let grammar: SyntaxGrammar
    private let tokenizer: SyntaxTokenizer
    private var cache: [LineTokens] = []

    static let longLineThreshold = 10000

    init(grammar: SyntaxGrammar) {
        self.grammar = grammar
        self.tokenizer = SyntaxTokenizer(grammar: grammar)
    }

    func reset() {
        cache.removeAll(keepingCapacity: false)
    }

    func invalidate(fromLine index: Int) {
        let target = max(0, index)
        if target < cache.count {
            cache.removeSubrange(target ..< cache.count)
        }
    }

    func tokens(forLine line: Int) -> [TokenSpan]? {
        guard cache.indices.contains(line) else { return nil }
        return cache[line].tokens
    }

    func applyEdit(
        startLine: Int,
        oldLineCount: Int,
        newLineCount: Int,
        backingStore: TextBackingStore
    ) -> EditOutcome {
        let oldEndLine = startLine + oldLineCount
        let newEndLine = startLine + newLineCount

        let priorBoundaryState: LineEndState? = if oldEndLine >= 1, oldEndLine - 1 < cache.count {
            cache[oldEndLine - 1].endState
        } else {
            nil
        }

        if startLine < cache.count {
            let removeEnd = min(oldEndLine, cache.count)
            cache.removeSubrange(startLine ..< removeEnd)
        }

        var state: LineEndState = startLine == 0
            ? .normal
            : (startLine - 1 < cache.count ? cache[startLine - 1].endState : .normal)

        let availableLines = max(0, backingStore.lineCount - startLine)
        let tokenizeCount = min(newLineCount, availableLines)
        var newEntries: [LineTokens] = []
        newEntries.reserveCapacity(tokenizeCount)
        for offset in 0 ..< tokenizeCount {
            let line = backingStore.line(at: startLine + offset)
            let result = tokenize(line: line, startState: state)
            newEntries.append(result)
            state = result.endState
        }

        guard !newEntries.isEmpty || newLineCount == 0 else {
            if let priorBoundaryState, priorBoundaryState != state, newEndLine < cache.count {
                cache.removeSubrange(newEndLine ..< cache.count)
                return .cascade
            }
            return .updated
        }

        let insertIndex = min(startLine, cache.count)
        cache.insert(contentsOf: newEntries, at: insertIndex)

        let newBoundaryState = tokenizeCount > 0 ? state : priorBoundaryState ?? .normal
        let hasDownstream = newEndLine < cache.count
        let cascade: Bool = if let priorBoundaryState {
            priorBoundaryState != newBoundaryState
        } else {
            hasDownstream
        }

        if cascade, newEndLine < cache.count {
            cache.removeSubrange(newEndLine ..< cache.count)
        }

        return cascade ? .cascade : .updated
    }

    func spans(
        in range: Range<Int>,
        lineStartOffsets: [Int],
        backingStore: TextBackingStore
    ) -> [AppliedSpan] {
        ensureCached(upTo: range.upperBound, backingStore: backingStore)
        let upper = min(range.upperBound, cache.count)
        guard range.lowerBound < upper else { return [] }

        let offsetsCount = lineStartOffsets.count
        var spans: [AppliedSpan] = []
        spans.reserveCapacity((upper - range.lowerBound) * 8)

        for localIndex in 0 ..< (upper - range.lowerBound) {
            let globalLine = range.lowerBound + localIndex
            let lineOffset = localIndex < offsetsCount ? lineStartOffsets[localIndex] : 0
            for token in cache[globalLine].tokens {
                spans.append(AppliedSpan(
                    range: NSRange(location: lineOffset + token.location, length: token.length),
                    scope: token.scope
                ))
            }
        }
        return spans
    }

    private func ensureCached(upTo target: Int, backingStore: TextBackingStore) {
        if cache.count > backingStore.lineCount {
            cache.removeSubrange(backingStore.lineCount ..< cache.count)
        }
        let limit = min(target, backingStore.lineCount)
        guard cache.count < limit else { return }

        var state: LineEndState = cache.isEmpty ? .normal : cache[cache.count - 1].endState
        cache.reserveCapacity(limit)
        while cache.count < limit {
            let line = backingStore.line(at: cache.count)
            let result = tokenize(line: line, startState: state)
            cache.append(result)
            state = result.endState
        }
    }

    private func tokenize(line: String, startState: LineEndState) -> LineTokens {
        if line.utf16.count > Self.longLineThreshold {
            return LineTokens(tokens: [], endState: startState)
        }
        let result = tokenizer.tokenize(line: line, startState: startState)
        return LineTokens(tokens: result.tokens, endState: result.endState)
    }
}
