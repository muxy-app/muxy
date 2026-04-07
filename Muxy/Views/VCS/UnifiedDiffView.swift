import SwiftUI

struct UnifiedDiffView: View {
    let rows: [DiffDisplayRow]
    let filePath: String

    private var chunks: [DiffChunk] {
        buildDiffChunks(from: rows)
    }

    private var numberColumnWidth: CGFloat {
        lineNumberWidth(for: maxLineNumber(in: rows))
    }

    var body: some View {
        LazyVStack(spacing: 0) {
            ForEach(chunks) { chunk in
                switch chunk {
                case let .divider(_, text):
                    DiffSectionDivider(text: text)
                case let .codeBlock(_, blockRows):
                    unifiedCodeBlock(blockRows)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var gutterWidth: CGFloat {
        numberColumnWidth * 2 + 2 + DiffGutterNSView.prefixColumnWidth
    }

    private func unifiedCodeBlock(_ blockRows: [DiffDisplayRow]) -> some View {
        let height = CGFloat(blockRows.count) * diffLineHeight
        let metadata = buildDiffMetadata(from: blockRows)
        return HStack(alignment: .top, spacing: 0) {
            DiffGutterBridge(metadata: metadata, filePath: filePath, mode: .unified, columnWidth: numberColumnWidth)
                .frame(width: gutterWidth, height: height)

            ScrollView(.horizontal, showsIndicators: false) {
                DiffContentBridge(
                    rows: blockRows,
                    backgroundSide: .both
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
