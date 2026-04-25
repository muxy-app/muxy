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
        .zig,
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
        .r,
        .julia,
        .clojure,
        .ocaml,
        .powershell,
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

    private static let hintAliases: [String: String] = [
        "objc": "m",
        "objective-c": "m",
        "objectivec": "m",
        "c++": "cpp",
        "cxx": "cpp",
        "cc": "cpp",
        "h": "c",
        "hpp": "cpp",
        "cs": "cs",
        "csharp": "cs",
        "js": "js",
        "jsx": "jsx",
        "ts": "ts",
        "tsx": "tsx",
        "javascript": "js",
        "typescript": "ts",
        "py": "py",
        "python": "py",
        "rb": "rb",
        "ruby": "rb",
        "sh": "sh",
        "bash": "sh",
        "zsh": "sh",
        "shell": "sh",
        "ps": "ps1",
        "pwsh": "ps1",
        "powershell": "ps1",
        "yml": "yml",
        "yaml": "yml",
        "md": "md",
        "markdown": "md",
        "html": "html",
        "htm": "html",
        "tf": "tf",
        "terraform": "tf",
        "rs": "rs",
        "rust": "rs",
        "go": "go",
        "golang": "go",
        "kt": "kt",
        "kotlin": "kt",
        "dockerfile": "dockerfile",
        "make": "makefile",
        "makefile": "makefile",
        "graphql": "graphql",
        "gql": "graphql",
        "vue": "vue",
        "svelte": "svelte",
        "csv": "csv",
        "json": "json",
        "toml": "toml",
        "ini": "ini",
        "sql": "sql",
        "lua": "lua",
        "perl": "pl",
        "pl": "pl",
        "elixir": "ex",
        "ex": "ex",
        "exs": "exs",
        "haskell": "hs",
        "hs": "hs",
        "r": "r",
        "julia": "jl",
        "jl": "jl",
        "clojure": "clj",
        "clj": "clj",
        "ocaml": "ml",
        "ml": "ml",
        "scala": "scala",
        "java": "java",
        "swift": "swift",
        "dart": "dart",
        "zig": "zig",
        "php": "php",
        "xml": "xml",
        "css": "css",
    ]

    static func grammar(forLanguageHint hint: String) -> SyntaxGrammar? {
        let trimmed = hint.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !trimmed.isEmpty else { return nil }
        if let grammar = extensionMap[trimmed] {
            return grammar
        }
        if let alias = hintAliases[trimmed], let grammar = extensionMap[alias] {
            return grammar
        }
        return nil
    }
}
