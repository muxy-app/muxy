import Foundation

enum DiffCommentSide: Hashable {
    case old
    case new
}

enum DiffCommentSubmissionState: Equatable {
    case draft
    case posting
    case posted
    case failed(String)
    case outdated
}

struct DiffInlineComment: Identifiable, Equatable {
    let id: UUID
    let cacheKey: String
    let filePath: String
    let side: DiffCommentSide
    let startLine: Int
    let endLine: Int
    var body: String
    let author: String
    let createdAt: Date
    var submissionState: DiffCommentSubmissionState

    init(
        id: UUID = UUID(),
        cacheKey: String,
        filePath: String,
        side: DiffCommentSide,
        startLine: Int,
        endLine: Int,
        body: String,
        author: String,
        createdAt: Date,
        submissionState: DiffCommentSubmissionState = .draft
    ) {
        self.id = id
        self.cacheKey = cacheKey
        self.filePath = filePath
        self.side = side
        self.startLine = startLine
        self.endLine = endLine
        self.body = body
        self.author = author
        self.createdAt = createdAt
        self.submissionState = submissionState
    }

    var lineRangeLabel: String {
        startLine == endLine ? "L\(startLine)" : "L\(startLine)–L\(endLine)"
    }
}

struct DiffCommentComposerTarget: Equatable {
    let cacheKey: String
    let filePath: String
    let side: DiffCommentSide
    let startLine: Int
    let endLine: Int
}

enum DiffCommentAnchor {
    static func lineNumber(for row: DiffDisplayRow, side: DiffCommentSide) -> Int? {
        switch side {
        case .old:
            row.oldLineNumber
        case .new:
            row.newLineNumber
        }
    }

    static func side(for row: DiffDisplayRow) -> DiffCommentSide {
        row.kind == .deletion ? .old : .new
    }

    static func resolveDisplayRowIndex(side: DiffCommentSide, line: Int, in rows: [DiffDisplayRow]) -> Int? {
        rows.firstIndex { row in
            lineNumber(for: row, side: side) == line
        }
    }

    static func snippet(for comment: DiffInlineComment, in rows: [DiffDisplayRow]) -> String {
        let texts = rows.compactMap { row -> String? in
            guard let line = lineNumber(for: row, side: comment.side),
                  line >= comment.startLine,
                  line <= comment.endLine
            else { return nil }
            return row.text
        }
        return texts.joined(separator: "\n")
    }
}
