import AppKit
import Foundation

struct TerminalTextSnapshot {
    let text: String
    let viewSize: CGSize
    let cellSize: CGSize?
}

struct TerminalQuickSelectMatch: Identifiable {
    let id = UUID()
    let text: String
    let label: String
    let frame: CGRect
}

enum TerminalQuickSelectInput {
    case character(String, shifted: Bool)
    case delete
    case escape
}

enum TerminalQuickSelectResult: Equatable {
    case none
    case copy(String, paste: Bool)
    case dismiss
}

@MainActor
@Observable
final class TerminalQuickSelectState {
    var isVisible = false
    var matches: [TerminalQuickSelectMatch] = []
    var prefix = ""
    var status = ""

    func activate(snapshot: TerminalTextSnapshot?) {
        guard let snapshot else {
            status = "No terminal text available"
            isVisible = true
            matches = []
            prefix = ""
            return
        }

        let found = Self.matches(in: snapshot)
        matches = found
        prefix = ""
        status = found.isEmpty ? "No matches" : "Type a label to copy, Shift-label to paste"
        isVisible = true
    }

    func dismiss() {
        isVisible = false
        matches = []
        prefix = ""
        status = ""
    }

    func handle(_ input: TerminalQuickSelectInput) -> TerminalQuickSelectResult {
        switch input {
        case .escape:
            dismiss()
            return .dismiss
        case .delete:
            guard !prefix.isEmpty else { return .none }
            prefix.removeLast()
            return .none
        case let .character(value, shifted):
            let next = prefix + value.lowercased()
            let candidates = matches.filter { $0.label.hasPrefix(next) }
            guard !candidates.isEmpty else {
                NSSound.beep()
                return .none
            }

            prefix = next
            if let exact = candidates.first(where: { $0.label == next }) {
                let text = exact.text
                let shouldPaste = shifted
                dismiss()
                return .copy(text, paste: shouldPaste)
            }

            return .none
        }
    }

    private static func matches(in snapshot: TerminalTextSnapshot) -> [TerminalQuickSelectMatch] {
        let lines = snapshot.text.components(separatedBy: .newlines)
        let candidates = matchCandidates(lines: lines)
        let labels = labels(count: candidates.count)
        let fallbackLineCount = max(lines.count, 1)
        let fallbackColumnCount = max(lines.map(\.count).max() ?? 1, 1)
        let cellWidth = snapshot.cellSize?.width ?? snapshot.viewSize.width / CGFloat(fallbackColumnCount)
        let cellHeight = snapshot.cellSize?.height ?? snapshot.viewSize.height / CGFloat(fallbackLineCount)

        return zip(candidates, labels).map { candidate, label in
            TerminalQuickSelectMatch(
                text: candidate.text,
                label: label,
                frame: CGRect(
                    x: CGFloat(candidate.column) * cellWidth,
                    y: CGFloat(candidate.line) * cellHeight,
                    width: max(CGFloat(candidate.length) * cellWidth, cellWidth),
                    height: cellHeight
                )
            )
        }
    }

    private static func matchCandidates(lines: [String]) -> [TerminalQuickSelectCandidate] {
        var result: [TerminalQuickSelectCandidate] = []
        var seen = Set<String>()
        var occupiedRanges: [Int: [Range<String.Index>]] = [:]
        for lineIndex in lines.indices {
            let line = lines[lineIndex]
            for pattern in patterns {
                for range in ranges(matching: pattern, in: line) {
                    if occupiedRanges[lineIndex, default: []].contains(where: { $0.overlaps(range) }) {
                        continue
                    }
                    let text = String(line[range]).trimmedQuickSelectText
                    guard !text.isEmpty else { continue }
                    let column = line.distance(from: line.startIndex, to: range.lowerBound)
                    let length = text.count
                    let key = "\(lineIndex):\(column):\(length)"
                    guard seen.insert(key).inserted else { continue }
                    result.append(TerminalQuickSelectCandidate(
                        text: text,
                        line: lineIndex,
                        column: column,
                        length: length
                    ))
                    occupiedRanges[lineIndex, default: []].append(range)
                }
            }
        }
        return Array(result.prefix(702))
    }

    private static func ranges(matching pattern: NSRegularExpression, in line: String) -> [Range<String.Index>] {
        let fullRange = NSRange(line.startIndex ..< line.endIndex, in: line)
        return pattern.matches(in: line, range: fullRange).compactMap { match in
            Range(match.range, in: line)
        }
    }

    private static func labels(count: Int) -> [String] {
        let alphabet = TerminalSettings.quickSelectLabels
        guard count > alphabet.count else { return Array(alphabet.prefix(count)) }
        var length = 2
        while Int(pow(Double(alphabet.count), Double(length))) < count {
            length += 1
        }
        return Array(labelCombinations(alphabet: alphabet, length: length).prefix(count))
    }

    private static func labelCombinations(alphabet: [String], length: Int) -> [String] {
        guard length > 1 else { return alphabet }
        let tails = labelCombinations(alphabet: alphabet, length: length - 1)
        return alphabet.flatMap { first in
            tails.map { first + $0 }
        }
    }

    private static let patterns: [NSRegularExpression] = [
        regex(#"https?://[^\s<>"']+"#),
        regex(#"(?<![\w.-])(?:~|\.{1,2}|/)[A-Za-z0-9_@%+=:,./~#-]+(?::[0-9]+)?"#),
        regex(#"\b[0-9a-fA-F]{7,40}\b"#),
        regex(#"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b"#),
        regex(#"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}(?::[0-9]+)?\b"#),
        regex(#"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b"#),
    ]

    private static func regex(_ pattern: String) -> NSRegularExpression {
        do {
            return try NSRegularExpression(pattern: pattern)
        } catch {
            fatalError("Invalid quick select pattern: \(pattern)")
        }
    }
}

private struct TerminalQuickSelectCandidate {
    let text: String
    let line: Int
    let column: Int
    let length: Int
}

private extension String {
    var trimmedQuickSelectText: String {
        var value = self
        let trailing = CharacterSet(charactersIn: ".,;:)]}>\"'")
        while let scalar = value.unicodeScalars.last, trailing.contains(scalar) {
            value.removeLast()
        }
        return value
    }
}
