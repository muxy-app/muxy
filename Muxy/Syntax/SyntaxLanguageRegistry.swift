import Foundation

enum SyntaxLanguageRegistry {
    private static let allGrammars: [SyntaxGrammar] = [
        .swift,
        .objectiveC,
        .c,
        .cpp,
        .csharp,
        .java,
        .kotlin,
        .scala,
        .go,
        .rust,
        .dart,
        .javascript,
        .typescript,
        .php,
        .python,
        .ruby,
        .lua,
        .shell,
        .perl,
        .elixir,
        .haskell,
        .html,
        .xml,
        .css,
        .markdown,
        .vue,
        .svelte,
        .graphql,
        .terraform,
        .csv,
        .json,
        .yaml,
        .toml,
        .ini,
        .sql,
        .dockerfile,
        .makefile,
    ]

    private static let extensionMap: [String: SyntaxGrammar] = {
        var map: [String: SyntaxGrammar] = [:]
        for grammar in allGrammars {
            for ext in grammar.extensions {
                map[ext.lowercased()] = grammar
            }
        }
        return map
    }()

    static func grammar(forFile filename: String) -> SyntaxGrammar? {
        let url = URL(fileURLWithPath: filename)
        let ext = url.pathExtension.lowercased()
        if !ext.isEmpty, let grammar = extensionMap[ext] {
            return grammar
        }
        let name = url.lastPathComponent.lowercased()
        if let grammar = extensionMap[name] {
            return grammar
        }
        return nil
    }
}
