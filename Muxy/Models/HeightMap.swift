import Foundation

@MainActor
final class HeightMap {
    enum BlockKind: Equatable {
        case measured(lineHeights: [CGFloat])
        case estimated(perLineCharCounts: [Int])
    }

    struct Block: Equatable {
        let kind: BlockKind
        let lineCount: Int
        let charCount: Int
        let height: CGFloat
    }

    struct LineLocation: Equatable {
        let line: Int
        let topY: CGFloat
        let height: CGFloat
    }

    private(set) var blocks: [Block] = []
    private(set) var totalLineCount: Int = 0
    private(set) var totalHeight: CGFloat = 0

    private let oracle: HeightOracle

    init(oracle: HeightOracle) {
        self.oracle = oracle
    }

    func reset(lineCharCounts: [Int]) {
        guard !lineCharCounts.isEmpty else {
            blocks = []
            totalLineCount = 0
            totalHeight = 0
            return
        }
        let chars = lineCharCounts.reduce(0, +)
        let estimatedHeight = oracle.heightForGap(charCount: chars, logicalLineCount: lineCharCounts.count)
        let initialBlock = Block(
            kind: .estimated(perLineCharCounts: lineCharCounts),
            lineCount: lineCharCounts.count,
            charCount: chars,
            height: estimatedHeight
        )
        blocks = [initialBlock]
        totalLineCount = lineCharCounts.count
        totalHeight = estimatedHeight
    }

    func heightAbove(line: Int) -> CGFloat {
        let target = max(0, min(line, totalLineCount))
        var remaining = target
        var height: CGFloat = 0
        for block in blocks {
            if remaining >= block.lineCount {
                height += block.height
                remaining -= block.lineCount
                if remaining == 0 { return height }
                continue
            }
            height += partialHeight(of: block, throughLines: remaining)
            return height
        }
        return height
    }

    func lineAtY(_ y: CGFloat) -> LineLocation {
        guard totalLineCount > 0 else { return LineLocation(line: 0, topY: 0, height: oracle.lineHeight) }
        let clampedY = max(0, min(y, totalHeight))
        var lineCursor = 0
        var heightCursor: CGFloat = 0
        for block in blocks {
            if heightCursor + block.height <= clampedY, lineCursor + block.lineCount < totalLineCount {
                heightCursor += block.height
                lineCursor += block.lineCount
                continue
            }
            return locate(in: block, baseLine: lineCursor, baseY: heightCursor, targetY: clampedY)
        }
        let lastLine = max(0, totalLineCount - 1)
        return LineLocation(line: lastLine, topY: heightCursor, height: oracle.lineHeight)
    }

    func heightOfLine(_ line: Int) -> CGFloat {
        guard line >= 0, line < totalLineCount else { return oracle.lineHeight }
        var remaining = line
        for block in blocks {
            if remaining >= block.lineCount {
                remaining -= block.lineCount
                continue
            }
            switch block.kind {
            case let .measured(lineHeights):
                return lineHeights[remaining]
            case let .estimated(perLineCharCounts):
                return oracle.heightForLine(charCount: perLineCharCounts[remaining])
            }
        }
        return oracle.lineHeight
    }

    func applyMeasurements(startLine: Int, lineHeights: [CGFloat], lineCharCounts: [Int]) {
        guard !lineHeights.isEmpty,
              lineHeights.count == lineCharCounts.count,
              startLine >= 0,
              startLine + lineHeights.count <= totalLineCount
        else { return }

        let measuredBlock = makeMeasuredBlock(lineHeights: lineHeights, lineCharCounts: lineCharCounts)
        replaceRange(startLine: startLine, lineCount: lineHeights.count, with: [measuredBlock])
    }

    func invalidateAllToEstimates(lineCharCounts: [Int]) {
        reset(lineCharCounts: lineCharCounts)
    }

    func replaceLines(startLine: Int, removingCount: Int, insertingLineCharCounts: [Int]) {
        let safeStart = max(0, min(startLine, totalLineCount))
        let safeRemove = max(0, min(removingCount, totalLineCount - safeStart))
        guard safeRemove > 0 || !insertingLineCharCounts.isEmpty else { return }

        var replacement: [Block] = []
        if !insertingLineCharCounts.isEmpty {
            let chars = insertingLineCharCounts.reduce(0, +)
            let estimatedHeight = oracle.heightForGap(
                charCount: chars,
                logicalLineCount: insertingLineCharCounts.count
            )
            replacement.append(Block(
                kind: .estimated(perLineCharCounts: insertingLineCharCounts),
                lineCount: insertingLineCharCounts.count,
                charCount: chars,
                height: estimatedHeight
            ))
        }
        replaceRange(startLine: safeStart, lineCount: safeRemove, with: replacement)
    }

    private func makeMeasuredBlock(lineHeights: [CGFloat], lineCharCounts: [Int]) -> Block {
        let totalH = lineHeights.reduce(0, +)
        let chars = lineCharCounts.reduce(0, +)
        return Block(
            kind: .measured(lineHeights: lineHeights),
            lineCount: lineHeights.count,
            charCount: chars,
            height: totalH
        )
    }

    private func replaceRange(startLine: Int, lineCount: Int, with replacement: [Block]) {
        guard startLine >= 0, lineCount >= 0 else { return }
        let endLine = startLine + lineCount

        var newBlocks: [Block] = []
        newBlocks.reserveCapacity(blocks.count + replacement.count)
        var cursor = 0
        var replacementInserted = false

        for block in blocks {
            let blockEnd = cursor + block.lineCount
            if blockEnd <= startLine {
                newBlocks.append(block)
                cursor = blockEnd
                continue
            }
            if cursor >= endLine {
                if !replacementInserted {
                    newBlocks.append(contentsOf: replacement)
                    replacementInserted = true
                }
                newBlocks.append(block)
                cursor = blockEnd
                continue
            }

            if cursor < startLine {
                let prefixCount = startLine - cursor
                if let prefix = sliceBlock(block, fromLineOffset: 0, lineCount: prefixCount) {
                    newBlocks.append(prefix)
                }
            }

            if !replacementInserted {
                newBlocks.append(contentsOf: replacement)
                replacementInserted = true
            }

            if blockEnd > endLine {
                let suffixOffset = endLine - cursor
                let suffixCount = blockEnd - endLine
                if let suffix = sliceBlock(block, fromLineOffset: suffixOffset, lineCount: suffixCount) {
                    newBlocks.append(suffix)
                }
            }

            cursor = blockEnd
        }

        if !replacementInserted {
            newBlocks.append(contentsOf: replacement)
        }

        blocks = mergeAdjacentEstimatedBlocks(newBlocks)
        recomputeTotals()
    }

    private func sliceBlock(_ block: Block, fromLineOffset offset: Int, lineCount: Int) -> Block? {
        guard lineCount > 0 else { return nil }
        let safeOffset = max(0, min(offset, block.lineCount))
        let safeCount = max(0, min(lineCount, block.lineCount - safeOffset))
        guard safeCount > 0 else { return nil }
        switch block.kind {
        case let .measured(lineHeights):
            let slice = Array(lineHeights[safeOffset ..< safeOffset + safeCount])
            let height = slice.reduce(0, +)
            let proportionalChars = block.charCount * safeCount / max(1, block.lineCount)
            return Block(
                kind: .measured(lineHeights: slice),
                lineCount: safeCount,
                charCount: proportionalChars,
                height: height
            )
        case let .estimated(perLineCharCounts):
            let slice = Array(perLineCharCounts[safeOffset ..< safeOffset + safeCount])
            let chars = slice.reduce(0, +)
            let height = oracle.heightForGap(charCount: chars, logicalLineCount: safeCount)
            return Block(
                kind: .estimated(perLineCharCounts: slice),
                lineCount: safeCount,
                charCount: chars,
                height: height
            )
        }
    }

    private func mergeAdjacentEstimatedBlocks(_ input: [Block]) -> [Block] {
        guard input.count > 1 else { return input }
        var output: [Block] = []
        output.reserveCapacity(input.count)
        for block in input {
            guard let last = output.last,
                  case let .estimated(lastChars) = last.kind,
                  case let .estimated(blockChars) = block.kind
            else {
                output.append(block)
                continue
            }
            output.removeLast()
            let combinedCharsArray = lastChars + blockChars
            let combinedCharCount = last.charCount + block.charCount
            let combinedLineCount = last.lineCount + block.lineCount
            let combinedHeight = oracle.heightForGap(
                charCount: combinedCharCount,
                logicalLineCount: combinedLineCount
            )
            output.append(Block(
                kind: .estimated(perLineCharCounts: combinedCharsArray),
                lineCount: combinedLineCount,
                charCount: combinedCharCount,
                height: combinedHeight
            ))
        }
        return output
    }

    private func recomputeTotals() {
        var lines = 0
        var height: CGFloat = 0
        for block in blocks {
            lines += block.lineCount
            height += block.height
        }
        totalLineCount = lines
        totalHeight = height
    }

    private func partialHeight(of block: Block, throughLines lines: Int) -> CGFloat {
        let lineCount = max(0, min(lines, block.lineCount))
        guard lineCount > 0 else { return 0 }
        switch block.kind {
        case let .measured(lineHeights):
            return lineHeights[..<lineCount].reduce(0, +)
        case let .estimated(perLineCharCounts):
            let charsBefore = perLineCharCounts[..<lineCount].reduce(0, +)
            return oracle.heightForGap(charCount: charsBefore, logicalLineCount: lineCount)
        }
    }

    private func locate(in block: Block, baseLine: Int, baseY: CGFloat, targetY: CGFloat) -> LineLocation {
        let relativeY = targetY - baseY
        switch block.kind {
        case let .measured(lineHeights):
            var heightCursor: CGFloat = 0
            for (offset, lineHeight) in lineHeights.enumerated() {
                if heightCursor + lineHeight > relativeY {
                    return LineLocation(line: baseLine + offset, topY: baseY + heightCursor, height: lineHeight)
                }
                heightCursor += lineHeight
            }
            let lastIndex = max(0, block.lineCount - 1)
            let lastHeight = lineHeights.last ?? oracle.lineHeight
            return LineLocation(
                line: baseLine + lastIndex,
                topY: baseY + heightCursor - lastHeight,
                height: lastHeight
            )
        case let .estimated(perLineCharCounts):
            guard block.lineCount > 0 else {
                return LineLocation(line: baseLine, topY: baseY, height: oracle.lineHeight)
            }
            let perLine = block.height / CGFloat(block.lineCount)
            let approxOffset = max(0, min(block.lineCount - 1, Int(relativeY / max(1, perLine))))
            let topY = baseY + CGFloat(approxOffset) * perLine
            let charCount = perLineCharCounts[approxOffset]
            return LineLocation(
                line: baseLine + approxOffset,
                topY: topY,
                height: oracle.heightForLine(charCount: charCount)
            )
        }
    }
}
