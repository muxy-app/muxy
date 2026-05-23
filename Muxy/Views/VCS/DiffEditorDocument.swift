import Foundation

struct DiffEditorDocument {
    let text: String
    let lineKinds: [DiffDisplayRow.Kind]
    let gutterLines: [DiffEditorGutterLine]
    let fileLineIndexes: [String: Int]

    static func unified(rows: [DiffDisplayRow]) -> DiffEditorDocument {
        var lines: [String] = []
        var kinds: [DiffDisplayRow.Kind] = []
        var gutterLines: [DiffEditorGutterLine] = []
        lines.reserveCapacity(rows.count)
        kinds.reserveCapacity(rows.count)
        gutterLines.reserveCapacity(rows.count)

        for row in rows {
            switch row.kind {
            case .hunk:
                lines.append(hunkLabel(row.text))
            case .collapsed:
                lines.append(row.text)
            case .context:
                lines.append(contentText(for: row))
            case .addition:
                lines.append(contentText(for: row))
            case .deletion:
                lines.append(contentText(for: row))
            }
            kinds.append(row.kind)
            gutterLines.append(DiffEditorGutterLine(
                kind: row.kind,
                oldLineNumber: row.oldLineNumber,
                newLineNumber: row.newLineNumber
            ))
        }

        return DiffEditorDocument(text: lines.joined(separator: "\n"), lineKinds: kinds, gutterLines: gutterLines, fileLineIndexes: [:])
    }

    static func unified(sections: [DiffEditorFileSection]) -> DiffEditorDocument {
        combined(sections: sections, side: nil)
    }

    static func splitLeft(sections: [DiffEditorFileSection]) -> DiffEditorDocument {
        combined(sections: sections, side: .left)
    }

    static func splitRight(sections: [DiffEditorFileSection]) -> DiffEditorDocument {
        combined(sections: sections, side: .right)
    }

    static func splitLeft(rows: [DiffDisplayRow]) -> DiffEditorDocument {
        split(rows: rows).left
    }

    static func splitRight(rows: [DiffDisplayRow]) -> DiffEditorDocument {
        split(rows: rows).right
    }

    private static func split(rows: [DiffDisplayRow]) -> (left: DiffEditorDocument, right: DiffEditorDocument) {
        var leftLines: [String] = []
        var rightLines: [String] = []
        var leftKinds: [DiffDisplayRow.Kind] = []
        var rightKinds: [DiffDisplayRow.Kind] = []
        var leftGutterLines: [DiffEditorGutterLine] = []
        var rightGutterLines: [DiffEditorGutterLine] = []
        let pairedRows = SplitDiffPairedRow.pair(rows)
        leftLines.reserveCapacity(pairedRows.count)
        rightLines.reserveCapacity(pairedRows.count)
        leftKinds.reserveCapacity(pairedRows.count)
        rightKinds.reserveCapacity(pairedRows.count)
        leftGutterLines.reserveCapacity(pairedRows.count)
        rightGutterLines.reserveCapacity(pairedRows.count)

        for paired in pairedRows {
            switch paired.kind {
            case .hunk:
                let text = hunkLabel(paired.left?.text ?? paired.right?.text ?? "")
                leftLines.append(text)
                rightLines.append(text)
                leftKinds.append(.hunk)
                rightKinds.append(.hunk)
                leftGutterLines.append(DiffEditorGutterLine(kind: .hunk, oldLineNumber: nil, newLineNumber: nil))
                rightGutterLines.append(DiffEditorGutterLine(kind: .hunk, oldLineNumber: nil, newLineNumber: nil))
            case .collapsed:
                let text = paired.left?.text ?? paired.right?.text ?? ""
                leftLines.append(text)
                rightLines.append(text)
                leftKinds.append(.collapsed)
                rightKinds.append(.collapsed)
                leftGutterLines.append(DiffEditorGutterLine(kind: .collapsed, oldLineNumber: nil, newLineNumber: nil))
                rightGutterLines.append(DiffEditorGutterLine(kind: .collapsed, oldLineNumber: nil, newLineNumber: nil))
            case .content:
                let leftRow = paired.left
                let rightRow = paired.right
                leftLines.append(leftRow.map(contentText) ?? "")
                rightLines.append(rightRow.map(contentText) ?? "")
                leftKinds.append(leftRow?.kind ?? .context)
                rightKinds.append(rightRow?.kind ?? .context)
                leftGutterLines.append(DiffEditorGutterLine(
                    kind: leftRow?.kind ?? .context,
                    oldLineNumber: leftRow?.oldLineNumber,
                    newLineNumber: nil
                ))
                rightGutterLines.append(DiffEditorGutterLine(
                    kind: rightRow?.kind ?? .context,
                    oldLineNumber: nil,
                    newLineNumber: rightRow?.newLineNumber
                ))
            }
        }

        return (
            DiffEditorDocument(
                text: leftLines.joined(separator: "\n"),
                lineKinds: leftKinds,
                gutterLines: leftGutterLines,
                fileLineIndexes: [:]
            ),
            DiffEditorDocument(
                text: rightLines.joined(separator: "\n"),
                lineKinds: rightKinds,
                gutterLines: rightGutterLines,
                fileLineIndexes: [:]
            )
        )
    }

    private enum SplitSide {
        case left
        case right
    }

    private static func combined(sections: [DiffEditorFileSection], side: SplitSide?) -> DiffEditorDocument {
        var lines: [String] = []
        var kinds: [DiffDisplayRow.Kind] = []
        var gutterLines: [DiffEditorGutterLine] = []
        var fileLineIndexes: [String: Int] = [:]

        for section in sections {
            if !lines.isEmpty {
                lines.append("")
                kinds.append(.collapsed)
                gutterLines.append(DiffEditorGutterLine(kind: .collapsed, oldLineNumber: nil, newLineNumber: nil))
            }
            fileLineIndexes[section.cacheKey] = lines.count
            lines.append(headerText(for: section))
            kinds.append(.hunk)
            gutterLines.append(DiffEditorGutterLine(kind: .hunk, oldLineNumber: nil, newLineNumber: nil))

            guard !section.isCollapsed else { continue }

            let document = switch side {
            case nil: unified(rows: section.rows)
            case .left: splitLeft(rows: section.rows)
            case .right: splitRight(rows: section.rows)
            }
            append(document: document, lines: &lines, kinds: &kinds, gutterLines: &gutterLines)
        }

        return DiffEditorDocument(
            text: lines.joined(separator: "\n"),
            lineKinds: kinds,
            gutterLines: gutterLines,
            fileLineIndexes: fileLineIndexes
        )
    }

    private static func append(
        document: DiffEditorDocument,
        lines: inout [String],
        kinds: inout [DiffDisplayRow.Kind],
        gutterLines: inout [DiffEditorGutterLine]
    ) {
        guard !document.text.isEmpty else { return }
        lines.append(contentsOf: document.text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init))
        kinds.append(contentsOf: document.lineKinds)
        gutterLines.append(contentsOf: document.gutterLines)
    }

    private static func headerText(for section: DiffEditorFileSection) -> String {
        let chevron = section.isCollapsed ? "▸" : "▾"
        var parts = [chevron, section.filePath]
        if section.isStaged {
            parts.append("Staged")
        }
        if section.additions > 0 {
            parts.append("+\(section.additions)")
        }
        if section.deletions > 0 {
            parts.append("-\(section.deletions)")
        }
        return parts.joined(separator: " ")
    }

    private static func contentText(for row: DiffDisplayRow) -> String {
        switch row.kind {
        case .deletion:
            row.oldText ?? ""
        case .addition:
            row.newText ?? ""
        default:
            row.newText ?? row.oldText ?? ""
        }
    }
}

struct DiffEditorGutterLine: Equatable {
    let kind: DiffDisplayRow.Kind
    let oldLineNumber: Int?
    let newLineNumber: Int?
}

struct DiffEditorFileSection {
    let filePath: String
    let cacheKey: String
    let rows: [DiffDisplayRow]
    let isCollapsed: Bool
    let additions: Int
    let deletions: Int
    let isStaged: Bool
}
