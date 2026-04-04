import Foundation

struct ParsedDiffRows {
    let rows: [DiffDisplayRow]
    let additions: Int
    let deletions: Int
}

enum GitDiffParser {
    static func parseRows(_ patch: String) -> ParsedDiffRows {
        var rows: [DiffDisplayRow] = []
        var oldLineNumber = 0
        var newLineNumber = 0
        var inHunk = false
        var additions = 0
        var deletions = 0

        for rawLine in patch.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline) {
            let line = String(rawLine)

            if line.hasPrefix("@@") {
                inHunk = true
                let (oldStart, newStart) = parseHunkHeader(line)
                oldLineNumber = oldStart
                newLineNumber = newStart
                rows.append(DiffDisplayRow(
                    kind: .hunk,
                    oldLineNumber: nil,
                    newLineNumber: nil,
                    oldText: nil,
                    newText: nil,
                    text: line
                ))
                continue
            }

            guard inHunk else { continue }

            if line.hasPrefix(" ") {
                let content = String(line.dropFirst())
                rows.append(DiffDisplayRow(
                    kind: .context,
                    oldLineNumber: oldLineNumber,
                    newLineNumber: newLineNumber,
                    oldText: content,
                    newText: content,
                    text: " \(content)"
                ))
                oldLineNumber += 1
                newLineNumber += 1
                continue
            }

            if line.hasPrefix("-") {
                let content = String(line.dropFirst())
                rows.append(DiffDisplayRow(
                    kind: .deletion,
                    oldLineNumber: oldLineNumber,
                    newLineNumber: nil,
                    oldText: content,
                    newText: nil,
                    text: "-\(content)"
                ))
                oldLineNumber += 1
                deletions += 1
                continue
            }

            if line.hasPrefix("+") {
                let content = String(line.dropFirst())
                rows.append(DiffDisplayRow(
                    kind: .addition,
                    oldLineNumber: nil,
                    newLineNumber: newLineNumber,
                    oldText: nil,
                    newText: content,
                    text: "+\(content)"
                ))
                newLineNumber += 1
                additions += 1
                continue
            }
        }

        return ParsedDiffRows(rows: rows, additions: additions, deletions: deletions)
    }

    static func collapseContextRows(_ rows: [DiffDisplayRow]) -> [DiffDisplayRow] {
        var output: [DiffDisplayRow] = []
        var index = 0
        let leadingContext = 3
        let trailingContext = 3
        let collapseThreshold = 12

        while index < rows.count {
            let row = rows[index]
            if row.kind != .context {
                output.append(row)
                index += 1
                continue
            }

            var end = index
            while end < rows.count, rows[end].kind == .context {
                end += 1
            }
            let runLength = end - index

            if runLength <= collapseThreshold {
                output.append(contentsOf: rows[index ..< end])
            } else {
                let startKeepEnd = index + leadingContext
                let endKeepStart = end - trailingContext
                output.append(contentsOf: rows[index ..< startKeepEnd])
                output.append(DiffDisplayRow(
                    kind: .collapsed,
                    oldLineNumber: nil,
                    newLineNumber: nil,
                    oldText: nil,
                    newText: nil,
                    text: "\(runLength - leadingContext - trailingContext) unmodified lines"
                ))
                output.append(contentsOf: rows[endKeepStart ..< end])
            }
            index = end
        }

        return output
    }

    static func parseHunkHeader(_ line: String) -> (Int, Int) {
        let parts = line.split(separator: " ")
        guard parts.count >= 3 else { return (0, 0) }

        let oldPart = String(parts[1])
        let newPart = String(parts[2])

        let oldNumber = parseHunkNumber(oldPart)
        let newNumber = parseHunkNumber(newPart)
        return (oldNumber, newNumber)
    }

    static func parseHunkNumber(_ token: String) -> Int {
        let cleaned = token.trimmingCharacters(in: CharacterSet(charactersIn: "-+,"))
        guard let start = cleaned.split(separator: ",").first else { return 0 }
        return Int(start) ?? 0
    }
}
