import Foundation

enum LineEndState: Equatable {
    case normal
    case inBlockComment(id: Int, depth: Int)
    case inString(id: Int)
}

struct SyntaxGrammar {
    struct StringRule {
        let id: Int
        let open: String
        let close: String
        let escape: String?
        let multiline: Bool
        let scope: SyntaxScope
        let interpolation: String?
    }

    struct BlockCommentRule {
        let id: Int
        let open: String
        let close: String
        let scope: SyntaxScope
        let nestable: Bool
    }

    struct KeywordGroup {
        let words: Set<String>
        let scope: SyntaxScope
    }

    let name: String
    let extensions: [String]
    let caseSensitiveKeywords: Bool
    let lineComments: [String]
    let lineCommentScope: SyntaxScope
    let blockComments: [BlockCommentRule]
    let strings: [StringRule]
    let keywordGroups: [KeywordGroup]
    let operatorCharacters: Set<Character>
    let supportsNumbers: Bool
    let supportsHashDirectives: Bool
    let hashDirectiveScope: SyntaxScope
    let supportsAtAttributes: Bool
    let atAttributeScope: SyntaxScope
    let highlightFunctionCalls: Bool
    let highlightAllCapsAsConstant: Bool
    let identifierStart: Set<Character>
    let identifierBody: Set<Character>

    static let defaultIdentifierStart: Set<Character> = {
        var set = Set<Character>()
        for scalar in UnicodeScalar("a").value ... UnicodeScalar("z").value {
            if let c = UnicodeScalar(scalar) { set.insert(Character(c)) }
        }
        for scalar in UnicodeScalar("A").value ... UnicodeScalar("Z").value {
            if let c = UnicodeScalar(scalar) { set.insert(Character(c)) }
        }
        set.insert("_")
        return set
    }()

    static let defaultIdentifierBody: Set<Character> = {
        var set = defaultIdentifierStart
        for scalar in UnicodeScalar("0").value ... UnicodeScalar("9").value {
            if let c = UnicodeScalar(scalar) { set.insert(Character(c)) }
        }
        return set
    }()

    static let defaultOperatorCharacters: Set<Character> = Set("+-*/%=<>!&|^~?:")
}
