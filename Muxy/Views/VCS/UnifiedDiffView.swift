import SwiftUI

struct UnifiedDiffView: View {
    let rows: [DiffDisplayRow]
    let filePath: String

    var body: some View {
        LazyVStack(spacing: 0) {
            ForEach(rows) { row in
                if row.kind == .hunk || row.kind == .collapsed {
                    DiffSectionDivider(text: row.kind == .hunk ? hunkLabel(row.text) : row.text)
                } else {
                    DiffLineRow(filePath: filePath, lineNumber: row.newLineNumber ?? row.oldLineNumber) {
                        HStack(spacing: 0) {
                            numberCell(row.oldLineNumber)
                            numberCell(row.newLineNumber)
                            lineContent(row)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal, 8)
                        }
                        .frame(minHeight: 24)
                        .background(rowBackground(row.kind, side: .both))
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func numberCell(_ number: Int?) -> some View {
        Text(number.map(String.init) ?? "")
            .font(.system(size: 11, design: .monospaced))
            .foregroundStyle(MuxyTheme.fgDim)
            .frame(width: 42, alignment: .trailing)
            .padding(.trailing, 6)
            .background(.clear)
            .overlay(alignment: .trailing) {
                Rectangle().fill(MuxyTheme.border).frame(width: 1)
            }
    }

    @ViewBuilder
    private func lineContent(_ row: DiffDisplayRow) -> some View {
        switch row.kind {
        case .context:
            CodeHighlightedText(text: row.newText ?? "", kind: .context)
                .padding(.vertical, 2)
        case .addition:
            CodeHighlightedText(text: row.newText ?? "", kind: .addition)
                .padding(.vertical, 2)
        case .deletion:
            CodeHighlightedText(text: row.oldText ?? "", kind: .deletion)
                .padding(.vertical, 2)
        case .hunk,
             .collapsed:
            EmptyView()
        }
    }
}
