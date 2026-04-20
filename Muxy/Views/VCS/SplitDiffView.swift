import SwiftUI

struct SplitDiffView: View {
    let rows: [DiffDisplayRow]
    let filePath: String
    var suppressLeadingTopBorder: Bool = false

    private var chunks: [SplitDiffChunk] {
        buildSplitDiffChunks(from: rows)
    }

    private var numberColumnWidth: CGFloat {
        lineNumberWidth(for: maxLineNumber(in: rows))
    }

    var body: some View {
        LazyVStack(spacing: 0) {
            ForEach(Array(chunks.enumerated()), id: \.element.id) { index, chunk in
                switch chunk {
                case let .divider(_, text):
                    DiffSectionDivider(
                        text: text,
                        showsTopBorder: !(index == 0 && suppressLeadingTopBorder)
                    )
                case let .codeBlock(_, leftRows, rightRows):
                    splitCodeBlock(leftRows: leftRows, rightRows: rightRows)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Split diff, \(filePath)")
    }

    private func splitCodeBlock(leftRows: [DiffDisplayRow], rightRows: [DiffDisplayRow]) -> some View {
        let lineCount = max(leftRows.count, rightRows.count)
        let height = CGFloat(lineCount) * diffLineHeight
        let leftMeta = buildDiffMetadata(from: leftRows)
        let rightMeta = buildDiffMetadata(from: rightRows)

        return HStack(alignment: .top, spacing: 0) {
            DiffGutterBridge(metadata: leftMeta, filePath: filePath, mode: .singleOld, columnWidth: numberColumnWidth)
                .frame(width: numberColumnWidth + 1, height: height)

            ScrollView(.horizontal, showsIndicators: false) {
                DiffContentBridge(
                    rows: leftRows,
                    backgroundSide: .left
                )
                .frame(height: height)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            Rectangle().fill(MuxyTheme.border).frame(width: 1)

            DiffGutterBridge(metadata: rightMeta, filePath: filePath, mode: .singleNew, columnWidth: numberColumnWidth)
                .frame(width: numberColumnWidth + 1, height: height)

            ScrollView(.horizontal, showsIndicators: false) {
                DiffContentBridge(
                    rows: rightRows,
                    backgroundSide: .right
                )
                .frame(height: height)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(height: height)
        .frame(maxWidth: .infinity, alignment: .leading)
        .clipped()
    }
}

enum SplitDiffChunk: Identifiable {
    case divider(id: UUID, text: String)
    case codeBlock(id: UUID, leftRows: [DiffDisplayRow], rightRows: [DiffDisplayRow])

    var id: UUID {
        switch self {
        case let .divider(id, _): id
        case let .codeBlock(id, _, _): id
        }
    }
}

func buildSplitDiffChunks(from rows: [DiffDisplayRow]) -> [SplitDiffChunk] {
    let paired = SplitDiffPairedRow.pair(rows)
    var chunks: [SplitDiffChunk] = []
    var leftRows: [DiffDisplayRow] = []
    var rightRows: [DiffDisplayRow] = []

    for paired in paired {
        if paired.kind == .hunk || paired.kind == .collapsed {
            if !leftRows.isEmpty || !rightRows.isEmpty {
                padToEqualLength(&leftRows, &rightRows)
                chunks.append(.codeBlock(id: UUID(), leftRows: leftRows, rightRows: rightRows))
                leftRows = []
                rightRows = []
            }
            let rawText = paired.left?.text ?? paired.right?.text ?? ""
            let label = paired.kind == .hunk ? hunkLabel(rawText) : rawText
            chunks.append(.divider(id: UUID(), text: label))
        } else {
            leftRows.append(paired.left ?? emptyRow(kind: .context))
            rightRows.append(paired.right ?? emptyRow(kind: .context))
        }
    }

    if !leftRows.isEmpty || !rightRows.isEmpty {
        padToEqualLength(&leftRows, &rightRows)
        chunks.append(.codeBlock(id: UUID(), leftRows: leftRows, rightRows: rightRows))
    }

    return chunks
}

private func padToEqualLength(_ left: inout [DiffDisplayRow], _ right: inout [DiffDisplayRow]) {
    while left.count < right.count {
        left.append(emptyRow(kind: .context))
    }
    while right.count < left.count {
        right.append(emptyRow(kind: .context))
    }
}

private func emptyRow(kind: DiffDisplayRow.Kind) -> DiffDisplayRow {
    DiffDisplayRow(
        kind: kind,
        oldLineNumber: nil,
        newLineNumber: nil,
        oldText: nil,
        newText: nil,
        text: ""
    )
}

struct SplitDiffPairedRow: Identifiable {
    enum Kind {
        case content
        case hunk
        case collapsed
    }

    let id = UUID()
    let kind: Kind
    let left: DiffDisplayRow?
    let right: DiffDisplayRow?

    static func pair(_ rows: [DiffDisplayRow]) -> [SplitDiffPairedRow] {
        var result: [SplitDiffPairedRow] = []
        var index = 0

        while index < rows.count {
            let row = rows[index]

            switch row.kind {
            case .hunk:
                result.append(SplitDiffPairedRow(kind: .hunk, left: row, right: nil))
                index += 1

            case .collapsed:
                result.append(SplitDiffPairedRow(kind: .collapsed, left: row, right: nil))
                index += 1

            case .context:
                result.append(SplitDiffPairedRow(kind: .content, left: row, right: row))
                index += 1

            case .deletion:
                var deletions: [DiffDisplayRow] = []
                while index < rows.count, rows[index].kind == .deletion {
                    deletions.append(rows[index])
                    index += 1
                }
                var additions: [DiffDisplayRow] = []
                while index < rows.count, rows[index].kind == .addition {
                    additions.append(rows[index])
                    index += 1
                }
                let maxCount = max(deletions.count, additions.count)
                for i in 0 ..< maxCount {
                    result.append(SplitDiffPairedRow(
                        kind: .content,
                        left: i < deletions.count ? deletions[i] : nil,
                        right: i < additions.count ? additions[i] : nil
                    ))
                }

            case .addition:
                result.append(SplitDiffPairedRow(kind: .content, left: nil, right: row))
                index += 1
            }
        }

        return result
    }
}
