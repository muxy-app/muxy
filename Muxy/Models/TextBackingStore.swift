import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "TextBackingStore")

@MainActor
final class TextBackingStore {
    private(set) var lines: [String] = [""]
    private(set) var lineCharCounts: [Int] = [0]
    private var pendingTrailingFragment = ""

    var lineCount: Int { lines.count }

    func loadFromText(_ text: String) {
        let split = text.split(separator: "\n", omittingEmptySubsequences: false)
        lines = split.map(String.init)
        lineCharCounts = lines.map { ($0 as NSString).length }
        pendingTrailingFragment = ""
    }

    func appendText(_ chunk: String) {
        guard !chunk.isEmpty else { return }
        let combined = pendingTrailingFragment + chunk
        let split = combined.split(separator: "\n", omittingEmptySubsequences: false)
        guard !split.isEmpty else { return }

        let mergeFragment = String(split[0])
        if !lines.isEmpty {
            let mergedIndex = lines.count - 1
            lines[mergedIndex] += mergeFragment
            lineCharCounts[mergedIndex] = (lines[mergedIndex] as NSString).length
        } else {
            lines.append(mergeFragment)
            lineCharCounts.append((mergeFragment as NSString).length)
        }

        if split.count > 1 {
            for i in 1 ..< split.count - 1 {
                let line = String(split[i])
                lines.append(line)
                lineCharCounts.append((line as NSString).length)
            }
            pendingTrailingFragment = String(split[split.count - 1])
            lines.append(pendingTrailingFragment)
            lineCharCounts.append((pendingTrailingFragment as NSString).length)
        } else {
            pendingTrailingFragment = lines.last ?? ""
        }
    }

    func finishLoading() {
        pendingTrailingFragment = ""
    }

    func line(at index: Int) -> String {
        guard index >= 0, index < lines.count else { return "" }
        return lines[index]
    }

    func charCount(forLine index: Int) -> Int {
        guard index >= 0, index < lineCharCounts.count else { return 0 }
        return lineCharCounts[index]
    }

    func textForRange(_ range: Range<Int>) -> String {
        let clamped = max(0, range.lowerBound) ..< min(lines.count, range.upperBound)
        guard !clamped.isEmpty else { return "" }
        return lines[clamped].joined(separator: "\n")
    }

    func fullText() -> String {
        lines.joined(separator: "\n")
    }

    func replaceLines(in range: Range<Int>, with newLines: [String]) -> [String] {
        let clamped = max(0, range.lowerBound) ..< min(lines.count, range.upperBound)
        let old = Array(lines[clamped])
        lines.replaceSubrange(clamped, with: newLines)
        lineCharCounts.replaceSubrange(clamped, with: newLines.map { ($0 as NSString).length })
        return old
    }

    struct SearchMatch {
        let lineIndex: Int
        let range: NSRange
    }

    func search(needle: String, caseSensitive: Bool, useRegex: Bool) -> [SearchMatch] {
        guard !needle.isEmpty else { return [] }
        var matches: [SearchMatch] = []

        if useRegex {
            var options: NSRegularExpression.Options = [.anchorsMatchLines]
            if !caseSensitive { options.insert(.caseInsensitive) }
            guard let regex = try? NSRegularExpression(pattern: needle, options: options) else { return [] }

            for (lineIndex, line) in lines.enumerated() {
                let nsLine = line as NSString
                let lineRange = NSRange(location: 0, length: nsLine.length)
                regex.enumerateMatches(in: line, range: lineRange) { match, _, _ in
                    guard let match, match.range.length > 0 else { return }
                    matches.append(SearchMatch(lineIndex: lineIndex, range: match.range))
                }
            }
        } else {
            var options: NSString.CompareOptions = []
            if !caseSensitive { options.insert(.caseInsensitive) }

            for (lineIndex, line) in lines.enumerated() {
                let nsLine = line as NSString
                var searchRange = NSRange(location: 0, length: nsLine.length)
                while searchRange.location < nsLine.length {
                    let found = nsLine.range(of: needle, options: options, range: searchRange)
                    guard found.location != NSNotFound else { break }
                    matches.append(SearchMatch(lineIndex: lineIndex, range: found))
                    searchRange.location = found.location + found.length
                    searchRange.length = nsLine.length - searchRange.location
                }
            }
        }

        return matches
    }
}
