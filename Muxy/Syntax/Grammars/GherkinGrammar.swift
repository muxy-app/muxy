import Foundation

extension SyntaxGrammar {
    static let gherkin = SyntaxGrammar(
        name: "Gherkin",
        extensions: ["feature"],
        caseSensitiveKeywords: true,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"\"\"", close: "\"\"\"", escape: nil, multiline: true, scope: .docComment),
            StringRule(id: 2, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 3, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: ["Feature"], scope: .heading),
            KeywordGroup(words: [
                "Background", "Rule",
                "Scenario", "Example", "Examples", "Scenarios",
                "Outline", "Template",
            ], scope: .keyword),
            KeywordGroup(words: [
                "Given", "When", "Then", "And", "But",
            ], scope: .builtin),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: true,
        atAttributeScope: .attribute,
        highlightFunctionCalls: false,
        highlightAllCapsAsConstant: false,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: {
            var set = SyntaxGrammar.defaultIdentifierBody
            set.insert("-")
            return set
        }()
    )
}
