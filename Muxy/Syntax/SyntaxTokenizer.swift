import Foundation

struct TokenSpan {
    let location: Int
    let length: Int
    let scope: SyntaxScope
}

struct SyntaxTokenizer {
    let grammar: SyntaxGrammar

    func tokenize(line: String, startState: LineEndState) -> (tokens: [TokenSpan], endState: LineEndState) {
        let ns = line as NSString
        let length = ns.length
        var tokens: [TokenSpan] = []
        tokens.reserveCapacity(16)
        var cursor = 0
        var state = startState

        continueBlockComment(ns: ns, length: length, cursor: &cursor, state: &state, tokens: &tokens)
        if case .inBlockComment = state { return (tokens, state) }

        continueString(ns: ns, length: length, cursor: &cursor, state: &state, tokens: &tokens)
        if case .inString = state { return (tokens, state) }

        while cursor < length {
            let codeUnit = ns.character(at: cursor)

            if codeUnit == 0x20 || codeUnit == 0x09 {
                cursor += 1
                continue
            }

            if matchLineComment(ns: ns, length: length, cursor: &cursor, tokens: &tokens) {
                break
            }

            if matchBlockCommentStart(ns: ns, length: length, cursor: &cursor, state: &state, tokens: &tokens) {
                if case .inBlockComment = state { return (tokens, state) }
                continue
            }

            if matchStringStart(ns: ns, length: length, cursor: &cursor, state: &state, tokens: &tokens) {
                if case .inString = state { return (tokens, state) }
                continue
            }

            if grammar.supportsNumbers, isNumberStart(ns: ns, length: length, at: cursor) {
                let end = scanNumber(ns: ns, length: length, from: cursor)
                if end > cursor {
                    tokens.append(TokenSpan(location: cursor, length: end - cursor, scope: .number))
                    cursor = end
                    continue
                }
            }

            if grammar.supportsAtAttributes, codeUnit == 0x40,
               let consumed = scanPrefixedIdentifier(ns: ns, length: length, from: cursor)
            {
                tokens.append(TokenSpan(location: cursor, length: consumed, scope: grammar.atAttributeScope))
                cursor += consumed
                continue
            }

            if grammar.supportsHashDirectives, codeUnit == 0x23, isAtLineStart(ns: ns, upTo: cursor),
               let consumed = scanPrefixedIdentifier(ns: ns, length: length, from: cursor)
            {
                tokens.append(TokenSpan(location: cursor, length: consumed, scope: grammar.hashDirectiveScope))
                cursor += consumed
                continue
            }

            if let identifier = scanIdentifier(ns: ns, length: length, from: cursor) {
                handleIdentifier(ns: ns, length: length, match: identifier, tokens: &tokens)
                cursor = identifier.end
                continue
            }

            cursor += 1
        }

        return (tokens, state)
    }

    private func continueBlockComment(
        ns: NSString,
        length: Int,
        cursor: inout Int,
        state: inout LineEndState,
        tokens: inout [TokenSpan]
    ) {
        guard case let .inBlockComment(id, depth) = state else { return }
        guard let block = grammar.blockComments.first(where: { $0.id == id }) else {
            state = .normal
            return
        }

        var currentDepth = depth
        let start = cursor
        var scan = cursor
        let openNS = block.open as NSString
        let closeNS = block.close as NSString

        while scan < length {
            if block.nestable, matchesSubstring(openNS, in: ns, at: scan, length: length) {
                currentDepth += 1
                scan += openNS.length
                continue
            }
            if matchesSubstring(closeNS, in: ns, at: scan, length: length) {
                if currentDepth <= 1 {
                    scan += closeNS.length
                    tokens.append(TokenSpan(location: start, length: scan - start, scope: block.scope))
                    cursor = scan
                    state = .normal
                    return
                }
                currentDepth -= 1
                scan += closeNS.length
                continue
            }
            scan += 1
        }

        tokens.append(TokenSpan(location: start, length: length - start, scope: block.scope))
        cursor = length
        state = .inBlockComment(id: id, depth: currentDepth)
    }

    private func continueString(
        ns: NSString,
        length: Int,
        cursor: inout Int,
        state: inout LineEndState,
        tokens: inout [TokenSpan]
    ) {
        guard case let .inString(id) = state else { return }
        guard let rule = grammar.strings.first(where: { $0.id == id }) else {
            state = .normal
            return
        }

        let start = cursor
        let end = scanStringBody(ns: ns, length: length, from: cursor, rule: rule)
        tokens.append(TokenSpan(location: start, length: end.position - start, scope: rule.scope))
        cursor = end.position
        state = end.closed ? .normal : .inString(id: rule.id)
    }

    private func matchLineComment(
        ns: NSString,
        length: Int,
        cursor: inout Int,
        tokens: inout [TokenSpan]
    ) -> Bool {
        for marker in grammar.lineComments {
            let markerNS = marker as NSString
            if matchesSubstring(markerNS, in: ns, at: cursor, length: length) {
                tokens.append(TokenSpan(location: cursor, length: length - cursor, scope: grammar.lineCommentScope))
                cursor = length
                return true
            }
        }
        return false
    }

    private func matchBlockCommentStart(
        ns: NSString,
        length: Int,
        cursor: inout Int,
        state: inout LineEndState,
        tokens: inout [TokenSpan]
    ) -> Bool {
        for block in grammar.blockComments {
            let openNS = block.open as NSString
            guard matchesSubstring(openNS, in: ns, at: cursor, length: length) else { continue }

            let start = cursor
            var scan = cursor + openNS.length
            var depth = 1
            let closeNS = block.close as NSString

            while scan < length {
                if block.nestable, matchesSubstring(openNS, in: ns, at: scan, length: length) {
                    depth += 1
                    scan += openNS.length
                    continue
                }
                if matchesSubstring(closeNS, in: ns, at: scan, length: length) {
                    if depth <= 1 {
                        scan += closeNS.length
                        tokens.append(TokenSpan(location: start, length: scan - start, scope: block.scope))
                        cursor = scan
                        return true
                    }
                    depth -= 1
                    scan += closeNS.length
                    continue
                }
                scan += 1
            }

            tokens.append(TokenSpan(location: start, length: length - start, scope: block.scope))
            cursor = length
            state = .inBlockComment(id: block.id, depth: depth)
            return true
        }
        return false
    }

    private func matchStringStart(
        ns: NSString,
        length: Int,
        cursor: inout Int,
        state: inout LineEndState,
        tokens: inout [TokenSpan]
    ) -> Bool {
        for rule in grammar.strings {
            let openNS = rule.open as NSString
            guard matchesSubstring(openNS, in: ns, at: cursor, length: length) else { continue }

            let start = cursor
            let afterOpen = cursor + openNS.length
            let end = scanStringBody(ns: ns, length: length, from: afterOpen, rule: rule)
            tokens.append(TokenSpan(location: start, length: end.position - start, scope: rule.scope))
            cursor = end.position
            state = end.closed ? .normal : (rule.multiline ? .inString(id: rule.id) : .normal)
            return true
        }
        return false
    }

    private func scanStringBody(
        ns: NSString,
        length: Int,
        from start: Int,
        rule: SyntaxGrammar.StringRule
    ) -> (position: Int, closed: Bool) {
        let closeNS = rule.close as NSString
        let escapeNS = rule.escape.map { $0 as NSString }
        var scan = start
        while scan < length {
            if let escapeNS, matchesSubstring(escapeNS, in: ns, at: scan, length: length) {
                scan += escapeNS.length
                if scan < length {
                    scan += 1
                }
                continue
            }
            if matchesSubstring(closeNS, in: ns, at: scan, length: length) {
                scan += closeNS.length
                return (scan, true)
            }
            scan += 1
        }
        return (length, false)
    }

    private func scanPrefixedIdentifier(ns: NSString, length: Int, from start: Int) -> Int? {
        var scan = start + 1
        while scan < length {
            let ch = ns.character(at: scan)
            guard let scalar = Unicode.Scalar(ch) else { break }
            let character = Character(scalar)
            if !grammar.identifierBody.contains(character) { break }
            scan += 1
        }
        let consumed = scan - start
        return consumed > 1 ? consumed : nil
    }

    private struct IdentifierMatch {
        let start: Int
        let end: Int
        let word: String
    }

    private func scanIdentifier(ns: NSString, length: Int, from start: Int) -> IdentifierMatch? {
        let ch = ns.character(at: start)
        guard let scalar = Unicode.Scalar(ch) else { return nil }
        let character = Character(scalar)
        guard grammar.identifierStart.contains(character) else { return nil }

        var scan = start + 1
        while scan < length {
            let next = ns.character(at: scan)
            guard let nextScalar = Unicode.Scalar(next) else { break }
            if !grammar.identifierBody.contains(Character(nextScalar)) { break }
            scan += 1
        }
        let word = ns.substring(with: NSRange(location: start, length: scan - start))
        return IdentifierMatch(start: start, end: scan, word: word)
    }

    private func handleIdentifier(
        ns: NSString,
        length: Int,
        match: IdentifierMatch,
        tokens: inout [TokenSpan]
    ) {
        let lookup = grammar.caseSensitiveKeywords ? match.word : match.word.lowercased()
        let spanLength = match.end - match.start
        for group in grammar.keywordGroups where group.words.contains(lookup) {
            tokens.append(TokenSpan(location: match.start, length: spanLength, scope: group.scope))
            return
        }

        if grammar.highlightAllCapsAsConstant, isAllCapsConstant(match.word) {
            tokens.append(TokenSpan(location: match.start, length: spanLength, scope: .constant))
            return
        }

        if grammar.jsxAware, isJSXTagName(ns: ns, match: match) {
            tokens.append(TokenSpan(location: match.start, length: spanLength, scope: .tag))
            return
        }

        if grammar.jsxAware, isPascalCase(match.word) {
            tokens.append(TokenSpan(location: match.start, length: spanLength, scope: .type))
            return
        }

        if grammar.highlightFunctionCalls {
            var probe = match.end
            while probe < length {
                let ch = ns.character(at: probe)
                if ch == 0x20 || ch == 0x09 {
                    probe += 1
                    continue
                }
                break
            }
            if probe < length, ns.character(at: probe) == 0x28 {
                tokens.append(TokenSpan(location: match.start, length: spanLength, scope: .function))
            }
        }
    }

    private func isJSXTagName(ns: NSString, match: IdentifierMatch) -> Bool {
        var probe = match.start - 1
        if probe >= 0, ns.character(at: probe) == 0x2F {
            probe -= 1
        }
        guard probe >= 0, ns.character(at: probe) == 0x3C else { return false }
        if probe - 1 >= 0 {
            let prior = ns.character(at: probe - 1)
            if isIdentifierBodyCodeUnit(prior) { return false }
        }
        return true
    }

    private func isIdentifierBodyCodeUnit(_ ch: unichar) -> Bool {
        guard let scalar = Unicode.Scalar(ch) else { return false }
        return grammar.identifierBody.contains(Character(scalar))
    }

    private func isPascalCase(_ word: String) -> Bool {
        guard let first = word.first, first.isUppercase else { return false }
        var hasLower = false
        for character in word where character.isLowercase {
            hasLower = true
            break
        }
        return hasLower
    }

    private func isAllCapsConstant(_ word: String) -> Bool {
        guard word.count >= 2 else { return false }
        var hasLetter = false
        for character in word {
            if character == "_" { continue }
            if character.isNumber { continue }
            if character.isUppercase {
                hasLetter = true
                continue
            }
            return false
        }
        return hasLetter
    }

    private func isNumberStart(ns: NSString, length: Int, at index: Int) -> Bool {
        let ch = ns.character(at: index)
        if ch >= 0x30, ch <= 0x39 { return true }
        if ch == 0x2E, index + 1 < length {
            let next = ns.character(at: index + 1)
            return next >= 0x30 && next <= 0x39
        }
        return false
    }

    private func scanNumber(ns: NSString, length: Int, from start: Int) -> Int {
        var scan = start
        let first = ns.character(at: scan)
        if first == 0x30, scan + 1 < length {
            let next = ns.character(at: scan + 1)
            if next == 0x78 || next == 0x58 {
                scan += 2
                while scan < length, isHexDigit(ns.character(at: scan)) || ns.character(at: scan) == 0x5F {
                    scan += 1
                }
                return scan
            }
            if next == 0x62 || next == 0x42 {
                scan += 2
                while scan < length {
                    let ch = ns.character(at: scan)
                    if ch == 0x30 || ch == 0x31 || ch == 0x5F { scan += 1 } else { break }
                }
                return scan
            }
            if next == 0x6F || next == 0x4F {
                scan += 2
                while scan < length {
                    let ch = ns.character(at: scan)
                    if ch >= 0x30, ch <= 0x37 { scan += 1 } else if ch == 0x5F { scan += 1 } else { break }
                }
                return scan
            }
        }

        var sawDot = false
        var sawExp = false
        while scan < length {
            let ch = ns.character(at: scan)
            if ch >= 0x30, ch <= 0x39 {
                scan += 1
                continue
            }
            if ch == 0x5F {
                scan += 1
                continue
            }
            if ch == 0x2E, !sawDot, !sawExp {
                sawDot = true
                scan += 1
                continue
            }
            if ch == 0x65 || ch == 0x45, !sawExp {
                sawExp = true
                scan += 1
                if scan < length {
                    let signCh = ns.character(at: scan)
                    if signCh == 0x2B || signCh == 0x2D { scan += 1 }
                }
                continue
            }
            break
        }

        while scan < length {
            let ch = ns.character(at: scan)
            guard let scalar = Unicode.Scalar(ch) else { break }
            let character = Character(scalar)
            if grammar.identifierBody.contains(character), !(ch >= 0x30 && ch <= 0x39) {
                scan += 1
                continue
            }
            break
        }

        return scan
    }

    private func isHexDigit(_ ch: unichar) -> Bool {
        if ch >= 0x30, ch <= 0x39 { return true }
        if ch >= 0x41, ch <= 0x46 { return true }
        if ch >= 0x61, ch <= 0x66 { return true }
        return false
    }

    private func isAtLineStart(ns: NSString, upTo index: Int) -> Bool {
        var scan = 0
        while scan < index {
            let ch = ns.character(at: scan)
            if ch != 0x20, ch != 0x09 { return false }
            scan += 1
        }
        return true
    }

    private func matchesSubstring(_ needle: NSString, in haystack: NSString, at index: Int, length: Int) -> Bool {
        let needleLength = needle.length
        guard needleLength > 0, index + needleLength <= length else { return false }
        for offset in 0 ..< needleLength where haystack.character(at: index + offset) != needle.character(at: offset) {
            return false
        }
        return true
    }
}
