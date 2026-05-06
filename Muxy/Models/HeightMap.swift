import Foundation

@MainActor
final class HeightMap {
    enum BlockKind: Equatable {
        case measured(lineHeights: [CGFloat], heightPrefix: [CGFloat])
        case estimated(perLineCharCounts: [Int], heightPrefix: [CGFloat])
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
        blocks = [makeEstimatedBlock(perLineCharCounts: lineCharCounts)]
        totalLineCount = lineCharCounts.count
        totalHeight = blocks[0].height
    }

    private func makeEstimatedBlock(perLineCharCounts: [Int]) -> Block {
        let lineHeights = perLineCharCounts.map { oracle.heightForLine(charCount: $0) }
        let prefix = prefixSums(lineHeights)
        let height = prefix.last ?? 0
        return Block(
            kind: .estimated(perLineCharCounts: perLineCharCounts, heightPrefix: prefix),
            lineCount: perLineCharCounts.count,
            charCount: perLineCharCounts.reduce(0, +),
            height: height
        )
    }

    private func makeMeasuredBlock(lineHeights: [CGFloat], lineCharCounts: [Int]) -> Block {
        let prefix = prefixSums(lineHeights)
        let total = prefix.last ?? 0
        return Block(
            kind: .measured(lineHeights: lineHeights, heightPrefix: prefix),
            lineCount: lineHeights.count,
            charCount: lineCharCounts.reduce(0, +),
            height: total
        )
    }

    private func prefixSums(_ values: [Int]) -> [Int] {
        var result: [Int] = []
        result.reserveCapacity(values.count + 1)
        result.append(0)
        var running = 0
        for value in values {
            running += value
            result.append(running)
        }
        return result
    }

    private func prefixSums(_ values: [CGFloat]) -> [CGFloat] {
        var result: [CGFloat] = []
        result.reserveCapacity(values.count + 1)
        result.append(0)
        var running: CGFloat = 0
        for value in values {
            running += value
            result.append(running)
        }
        return result
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
            case let .measured(lineHeights, _):
                return lineHeights[remaining]
            case let .estimated(perLineCharCounts, _):
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

    func replaceLines(startLine: Int, removingCount: Int, insertingLineCharCounts: [Int]) {
        let safeStart = max(0, min(startLine, totalLineCount))
        let safeRemove = max(0, min(removingCount, totalLineCount - safeStart))
        guard safeRemove > 0 || !insertingLineCharCounts.isEmpty else { return }

        var replacement: [Block] = []
        if !insertingLineCharCounts.isEmpty {
            replacement.append(makeEstimatedBlock(perLineCharCounts: insertingLineCharCounts))
        }
        replaceRange(startLine: safeStart, lineCount: safeRemove, with: replacement)
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
        case let .measured(lineHeights, _):
            let slice = Array(lineHeights[safeOffset ..< safeOffset + safeCount])
            let prefix = prefixSums(slice)
            let total = prefix.last ?? 0
            let proportionalChars = block.charCount * safeCount / max(1, block.lineCount)
            return Block(
                kind: .measured(lineHeights: slice, heightPrefix: prefix),
                lineCount: safeCount,
                charCount: proportionalChars,
                height: total
            )
        case let .estimated(perLineCharCounts, _):
            let slice = Array(perLineCharCounts[safeOffset ..< safeOffset + safeCount])
            return makeEstimatedBlock(perLineCharCounts: slice)
        }
    }

    private func mergeAdjacentEstimatedBlocks(_ input: [Block]) -> [Block] {
        guard input.count > 1 else { return input }
        var output: [Block] = []
        output.reserveCapacity(input.count)
        for block in input {
            guard let last = output.last,
                  case let .estimated(lastChars, _) = last.kind,
                  case let .estimated(blockChars, _) = block.kind
            else {
                output.append(block)
                continue
            }
            output.removeLast()
            output.append(makeEstimatedBlock(perLineCharCounts: lastChars + blockChars))
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
        case let .measured(_, heightPrefix):
            return heightPrefix[lineCount]
        case let .estimated(_, heightPrefix):
            return heightPrefix[lineCount]
        }
    }

    private func locate(in block: Block, baseLine: Int, baseY: CGFloat, targetY: CGFloat) -> LineLocation {
        let relativeY = targetY - baseY
        switch block.kind {
        case let .measured(lineHeights, heightPrefix):
            var low = 0
            var high = lineHeights.count - 1
            while low < high {
                let mid = (low + high) / 2
                if heightPrefix[mid + 1] <= relativeY {
                    low = mid + 1
                } else {
                    high = mid
                }
            }
            let offset = max(0, min(low, lineHeights.count - 1))
            return LineLocation(
                line: baseLine + offset,
                topY: baseY + heightPrefix[offset],
                height: lineHeights[offset]
            )
        case let .estimated(perLineCharCounts, heightPrefix):
            guard block.lineCount > 0 else {
                return LineLocation(line: baseLine, topY: baseY, height: oracle.lineHeight)
            }
            var low = 0
            var high = perLineCharCounts.count - 1
            while low < high {
                let mid = (low + high) / 2
                if heightPrefix[mid + 1] <= relativeY {
                    low = mid + 1
                } else {
                    high = mid
                }
            }
            let approxOffset = max(0, min(low, perLineCharCounts.count - 1))
            let topY = baseY + heightPrefix[approxOffset]
            let charCount = perLineCharCounts[approxOffset]
            return LineLocation(
                line: baseLine + approxOffset,
                topY: topY,
                height: oracle.heightForLine(charCount: charCount)
            )
        }
    }
}
