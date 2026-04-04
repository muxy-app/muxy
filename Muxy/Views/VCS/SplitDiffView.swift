import SwiftUI

struct SplitDiffView: View {
    let rows: [DiffDisplayRow]
    let filePath: String
    let pairedRows: [SplitDiffPairedRow]

    init(rows: [DiffDisplayRow], filePath: String) {
        self.rows = rows
        self.filePath = filePath
        pairedRows = SplitDiffPairedRow.pair(rows)
    }

    var body: some View {
        LazyVStack(spacing: 0) {
            ForEach(pairedRows) { paired in
                switch paired.kind {
                case .hunk,
                     .collapsed:
                    hunkOrCollapsedRow(paired)
                case .content:
                    contentRow(paired)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func hunkOrCollapsedRow(_ paired: SplitDiffPairedRow) -> some View {
        let rawText = paired.left?.text ?? paired.right?.text ?? ""
        let label = paired.kind == .hunk ? hunkLabel(rawText) : rawText
        return DiffSectionDivider(text: label)
    }

    private func contentRow(_ paired: SplitDiffPairedRow) -> some View {
        let lineNumber = paired.right?.newLineNumber ?? paired.left?.oldLineNumber
        return DiffLineRow(filePath: filePath, lineNumber: lineNumber) {
            HStack(spacing: 0) {
                splitCell(
                    number: paired.left?.oldLineNumber,
                    text: paired.left?.oldText,
                    changeKind: paired.left?.kind ?? .context,
                    isLeft: true
                )
                Rectangle().fill(MuxyTheme.border).frame(width: 1)
                splitCell(
                    number: paired.right?.newLineNumber,
                    text: paired.right?.newText,
                    changeKind: paired.right?.kind ?? .context,
                    isLeft: false
                )
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func splitCell(
        number: Int?,
        text: String?,
        changeKind: DiffDisplayRow.Kind,
        isLeft: Bool
    ) -> some View {
        let highlightKind: CodeHighlightedText.ChangeKind = switch changeKind {
        case .deletion: .deletion
        case .addition: .addition
        default: .context
        }
        let bgKind: DiffDisplayRow.Kind = isLeft
            ? (changeKind == .deletion ? .deletion : .context)
            : (changeKind == .addition ? .addition : .context)

        return HStack(spacing: 0) {
            Text(number.map(String.init) ?? "")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
                .frame(width: 42, alignment: .trailing)
                .padding(.trailing, 6)
                .background(.clear)
                .overlay(alignment: .trailing) {
                    Rectangle().fill(MuxyTheme.border).frame(width: 1)
                }

            CodeHighlightedText(text: text ?? "", kind: highlightKind)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 8)
                .padding(.vertical, 2)
        }
        .background(rowBackground(bgKind, side: isLeft ? .left : .right))
    }
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
