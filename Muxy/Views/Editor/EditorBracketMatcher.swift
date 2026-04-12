import Foundation

struct EditorBracketMatcher {
    struct BracketMatch {
        let first: Int
        let second: Int
    }

    private static let bracketScanLimit = 5000

    func findMatch(in content: NSString, cursor: Int) -> BracketMatch? {
        let length = content.length

        if cursor < length {
            let char = character(at: cursor, in: content)
            if let match = findMatchingBracket(for: char, at: cursor, in: content) {
                return BracketMatch(first: cursor, second: match)
            }
        }

        if cursor > 0 {
            let prev = cursor - 1
            let char = character(at: prev, in: content)
            if let match = findMatchingBracket(for: char, at: prev, in: content) {
                return BracketMatch(first: prev, second: match)
            }
        }

        return nil
    }

    private func findMatchingBracket(for char: Character, at location: Int, in content: NSString) -> Int? {
        let openers: [Character: Character] = ["(": ")", "[": "]", "{": "}"]
        let closers: [Character: Character] = [")": "(", "]": "[", "}": "{"]

        if let match = openers[char] {
            return scanForward(from: location + 1, open: char, close: match, in: content)
        }
        if let match = closers[char] {
            return scanBackward(from: location - 1, open: match, close: char, in: content)
        }
        return nil
    }

    private func scanForward(from start: Int, open: Character, close: Character, in content: NSString) -> Int? {
        let length = content.length
        let end = min(length, start + Self.bracketScanLimit)
        var depth = 1
        var state = BracketScanState()
        var index = start
        while index < end {
            let ch = character(at: index, in: content)
            let next = index + 1 < length ? character(at: index + 1, in: content) : nil
            state.advance(current: ch, next: next)
            if state.isInSkipRegion {
                index += 1
                continue
            }
            if ch == open {
                depth += 1
            } else if ch == close {
                depth -= 1
                if depth == 0 { return index }
            }
            index += 1
        }
        return nil
    }

    private func scanBackward(from start: Int, open: Character, close: Character, in content: NSString) -> Int? {
        guard start >= 0 else { return nil }
        let scanStart = max(0, start - Self.bracketScanLimit)

        var skipMask: [Bool] = []
        skipMask.reserveCapacity(start - scanStart + 1)
        var state = BracketScanState()
        var i = scanStart
        while i <= start {
            let ch = character(at: i, in: content)
            let next = i + 1 < content.length ? character(at: i + 1, in: content) : nil
            state.advance(current: ch, next: next)
            skipMask.append(state.isInSkipRegion)
            i += 1
        }

        var depth = 1
        var index = start
        while index >= scanStart {
            let maskIndex = index - scanStart
            if skipMask[maskIndex] {
                index -= 1
                continue
            }
            let ch = character(at: index, in: content)
            if ch == close {
                depth += 1
            } else if ch == open {
                depth -= 1
                if depth == 0 { return index }
            }
            index -= 1
        }
        return nil
    }

    private func character(at index: Int, in content: NSString) -> Character {
        guard let scalar = UnicodeScalar(content.character(at: index)) else {
            return "\u{FFFD}"
        }
        return Character(scalar)
    }
}

struct BracketScanState {
    private var inSingleQuote = false
    private var inDoubleQuote = false
    private var inLineComment = false
    private var inBlockComment = false
    private var escaped = false
    private var pendingBlockCommentExit = false

    var isInSkipRegion: Bool {
        inSingleQuote || inDoubleQuote || inLineComment || inBlockComment
    }

    mutating func advance(current: Character, next: Character?) {
        if inBlockComment {
            if pendingBlockCommentExit {
                pendingBlockCommentExit = false
                inBlockComment = false
                return
            }
            if current == "*", next == "/" {
                pendingBlockCommentExit = true
            }
            return
        }
        if inLineComment {
            if current == "\n" { inLineComment = false }
            return
        }
        if escaped {
            escaped = false
            return
        }
        if inSingleQuote {
            if current == "\\" { escaped = true
                return
            }
            if current == "'" { inSingleQuote = false }
            return
        }
        if inDoubleQuote {
            if current == "\\" { escaped = true
                return
            }
            if current == "\"" { inDoubleQuote = false }
            return
        }
        if current == "/", next == "/" {
            inLineComment = true
            return
        }
        if current == "/", next == "*" {
            inBlockComment = true
            return
        }
        if current == "\"" {
            inDoubleQuote = true
            return
        }
        if current == "'" {
            inSingleQuote = true
            return
        }
    }
}
