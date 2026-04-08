import AppKit

@MainActor
final class SyntaxHighlightExtension {
    let fileExtension: String

    init(fileExtension: String) {
        self.fileExtension = fileExtension
    }

    func applyTextAttributes(to storage: NSTextStorage, fullRange: NSRange) {
        let text = storage.string
        guard !text.isEmpty else { return }
        let rules = SyntaxRules.forExtension(fileExtension)
        for rule in rules {
            guard let regex = try? NSRegularExpression(pattern: rule.pattern, options: rule.options) else {
                continue
            }
            regex.enumerateMatches(in: text, range: fullRange) { match, _, _ in
                guard let matchRange = match?.range(at: rule.captureGroup) else { return }
                storage.addAttribute(.foregroundColor, value: rule.color(), range: matchRange)
            }
        }
    }
}

private struct SyntaxRule {
    let pattern: String
    let color: @MainActor () -> NSColor
    var options: NSRegularExpression.Options = []
    var captureGroup: Int = 0
}

private enum SyntaxRules {
    static func forExtension(_ ext: String) -> [SyntaxRule] {
        switch ext {
        case "swift": swift
        case "js",
             "jsx",
             "mjs",
             "cjs": javascript
        case "ts",
             "tsx",
             "mts": typescript
        case "py": python
        case "rb": ruby
        case "go": go
        case "rs": rust
        case "c",
             "h": cLang
        case "cpp",
             "cc",
             "cxx",
             "hpp": cpp
        case "json": json
        case "html",
             "htm": html
        case "css",
             "scss": css
        case "sh",
             "bash",
             "zsh": shell
        case "yaml",
             "yml": yaml
        case "toml": toml
        case "md",
             "markdown": markdown
        default: []
        }
    }

    private static var comment: @MainActor () -> NSColor {
        { GhosttyService.shared.paletteColor(at: 8) ?? .systemGray }
    }

    private static var string: @MainActor () -> NSColor {
        { GhosttyService.shared.paletteColor(at: 2) ?? .systemGreen }
    }

    private static var keyword: @MainActor () -> NSColor {
        { GhosttyService.shared.paletteColor(at: 4) ?? .systemBlue }
    }

    private static var number: @MainActor () -> NSColor {
        { GhosttyService.shared.paletteColor(at: 3) ?? .systemYellow }
    }

    private static var type: @MainActor () -> NSColor {
        { GhosttyService.shared.paletteColor(at: 5) ?? .systemPurple }
    }

    private static var function: @MainActor () -> NSColor {
        { GhosttyService.shared.paletteColor(at: 6) ?? .systemCyan }
    }

    private static func lineComment(_ prefix: String) -> SyntaxRule {
        SyntaxRule(pattern: "\(prefix).*$", color: comment, options: .anchorsMatchLines)
    }

    private static var blockComment: SyntaxRule {
        SyntaxRule(pattern: "/\\*[\\s\\S]*?\\*/", color: comment, options: .dotMatchesLineSeparators)
    }

    private static var dqString: SyntaxRule {
        SyntaxRule(pattern: "\"(?:[^\"\\\\]|\\\\.)*\"", color: string)
    }

    private static var sqString: SyntaxRule {
        SyntaxRule(pattern: "'(?:[^'\\\\]|\\\\.)*'", color: string)
    }

    private static var btString: SyntaxRule {
        SyntaxRule(pattern: "`(?:[^`\\\\]|\\\\.)*`", color: string)
    }

    private static var numberLit: SyntaxRule {
        SyntaxRule(
            pattern: "\\b(?:0[xXbBoO])?[0-9][0-9a-fA-F_]*\\.?[0-9a-fA-F_]*(?:[eEpP][+-]?[0-9_]+)?\\b",
            color: number
        )
    }

    private static func kw(_ words: [String]) -> SyntaxRule {
        SyntaxRule(pattern: "\\b(?:\(words.joined(separator: "|")))\\b", color: keyword)
    }

    private static var funcCall: SyntaxRule {
        SyntaxRule(pattern: "\\b([a-zA-Z_][a-zA-Z0-9_]*)\\s*\\(", color: function, captureGroup: 1)
    }

    static let swift: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString,
        kw([
            "import", "class", "struct", "enum", "protocol", "extension", "func", "var", "let",
            "if", "else", "guard", "switch", "case", "default", "for", "while", "repeat",
            "return", "break", "continue", "throw", "throws", "rethrows", "try", "catch",
            "do", "in", "where", "as", "is", "self", "Self", "super", "init", "deinit",
            "true", "false", "nil", "static", "final", "private", "fileprivate", "internal",
            "public", "open", "override", "mutating", "weak", "unowned", "lazy", "async",
            "await", "actor", "nonisolated", "some", "any", "typealias", "inout",
        ]),
        numberLit, funcCall,
    ]

    static let javascript: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString, sqString, btString,
        kw([
            "const", "let", "var", "function", "class", "extends", "return", "if", "else",
            "for", "while", "do", "switch", "case", "default", "break", "continue",
            "throw", "try", "catch", "finally", "new", "delete", "typeof", "instanceof",
            "import", "export", "from", "as", "async", "await", "yield", "of", "in",
            "true", "false", "null", "undefined", "this", "super", "void",
        ]),
        numberLit, funcCall,
    ]

    static let typescript: [SyntaxRule] = javascript + [
        kw([
            "type",
            "interface",
            "enum",
            "namespace",
            "abstract",
            "declare",
            "readonly",
            "keyof",
            "infer",
            "never",
            "unknown",
            "any",
        ]),
    ]

    static let python: [SyntaxRule] = [
        lineComment("#"),
        SyntaxRule(pattern: "\"\"\"[\\s\\S]*?\"\"\"", color: string, options: .dotMatchesLineSeparators),
        SyntaxRule(pattern: "'''[\\s\\S]*?'''", color: string, options: .dotMatchesLineSeparators),
        dqString, sqString,
        kw([
            "def", "class", "return", "if", "elif", "else", "for", "while", "break",
            "continue", "pass", "raise", "try", "except", "finally", "with", "as",
            "import", "from", "lambda", "yield", "global", "nonlocal", "assert", "del",
            "and", "or", "not", "is", "in", "True", "False", "None", "async", "await",
        ]),
        numberLit,
        SyntaxRule(pattern: "@[a-zA-Z_][a-zA-Z0-9_.]*", color: function),
        funcCall,
    ]

    static let ruby: [SyntaxRule] = [
        lineComment("#"), dqString, sqString,
        kw([
            "def", "end", "class", "module", "return", "if", "elsif", "else", "unless",
            "for", "while", "until", "do", "begin", "rescue", "ensure", "raise", "yield",
            "require", "require_relative", "include", "extend", "self", "super",
            "true", "false", "nil", "and", "or", "not", "then", "when", "case", "in",
        ]),
        SyntaxRule(pattern: ":[a-zA-Z_][a-zA-Z0-9_]*", color: string),
        numberLit, funcCall,
    ]

    static let go: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString, btString, sqString,
        kw([
            "break", "case", "chan", "const", "continue", "default", "defer", "else",
            "fallthrough", "for", "func", "go", "goto", "if", "import", "interface",
            "map", "package", "range", "return", "select", "struct", "switch", "type",
            "var", "true", "false", "nil", "iota",
        ]),
        numberLit, funcCall,
    ]

    static let rust: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString, sqString,
        kw([
            "as", "async", "await", "break", "const", "continue", "crate", "dyn",
            "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
            "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
            "self", "Self", "static", "struct", "super", "trait", "true", "type",
            "unsafe", "use", "where", "while",
        ]),
        SyntaxRule(pattern: "#\\[.*?\\]", color: function),
        numberLit, funcCall,
    ]

    static let cLang: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString, sqString,
        SyntaxRule(
            pattern: "#\\s*(?:include|define|ifdef|ifndef|endif|pragma|if|else|elif|undef)\\b.*$",
            color: function, options: .anchorsMatchLines
        ),
        kw([
            "auto", "break", "case", "char", "const", "continue", "default", "do",
            "double", "else", "enum", "extern", "float", "for", "goto", "if", "inline",
            "int", "long", "register", "return", "short", "signed", "sizeof",
            "static", "struct", "switch", "typedef", "union", "unsigned", "void",
            "volatile", "while", "NULL", "true", "false",
        ]),
        numberLit, funcCall,
    ]

    static let cpp: [SyntaxRule] = cLang + [
        kw([
            "class",
            "namespace",
            "template",
            "typename",
            "this",
            "new",
            "delete",
            "try",
            "catch",
            "throw",
            "virtual",
            "override",
            "final",
            "public",
            "private",
            "protected",
            "using",
            "nullptr",
            "constexpr",
            "noexcept",
        ]),
    ]

    static let json: [SyntaxRule] = [
        SyntaxRule(pattern: "\"(?:[^\"\\\\]|\\\\.)*\"\\s*:", color: function),
        dqString, numberLit,
        kw(["true", "false", "null"]),
    ]

    static let html: [SyntaxRule] = [
        SyntaxRule(pattern: "<!--[\\s\\S]*?-->", color: comment, options: .dotMatchesLineSeparators),
        SyntaxRule(pattern: "</?\\w+", color: keyword),
        SyntaxRule(pattern: "/?>", color: keyword),
        SyntaxRule(pattern: "\\b[a-zA-Z-]+(?==)", color: function),
        dqString, sqString,
    ]

    static let css: [SyntaxRule] = [
        blockComment,
        SyntaxRule(pattern: "[.#][a-zA-Z_][a-zA-Z0-9_-]*", color: function),
        SyntaxRule(pattern: "@[a-zA-Z-]+", color: keyword),
        SyntaxRule(pattern: "[a-zA-Z-]+(?=\\s*:)", color: type),
        dqString, sqString, numberLit,
    ]

    static let shell: [SyntaxRule] = [
        lineComment("#"), dqString, sqString,
        kw([
            "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case",
            "esac", "in", "function", "return", "exit", "local", "export", "source",
            "true", "false",
        ]),
        SyntaxRule(pattern: "\\$\\{?[a-zA-Z_][a-zA-Z0-9_]*\\}?", color: type),
        numberLit,
    ]

    static let yaml: [SyntaxRule] = [
        lineComment("#"),
        SyntaxRule(pattern: "^[a-zA-Z_][a-zA-Z0-9_./-]*(?=\\s*:)", color: function, options: .anchorsMatchLines),
        dqString, sqString,
        kw(["true", "false", "null", "yes", "no"]),
        numberLit,
    ]

    static let toml: [SyntaxRule] = [
        lineComment("#"),
        SyntaxRule(pattern: "\\[\\[?[^\\]]+\\]\\]?", color: function),
        SyntaxRule(pattern: "^[a-zA-Z_][a-zA-Z0-9_.-]*(?=\\s*=)", color: type, options: .anchorsMatchLines),
        dqString, sqString,
        kw(["true", "false"]),
        numberLit,
    ]

    static let markdown: [SyntaxRule] = [
        SyntaxRule(pattern: "^#{1,6}\\s+.*$", color: keyword, options: .anchorsMatchLines),
        SyntaxRule(pattern: "\\*\\*[^*]+\\*\\*", color: keyword),
        SyntaxRule(pattern: "`[^`]+`", color: string),
        SyntaxRule(pattern: "\\[([^\\]]+)\\]\\([^)]+\\)", color: function),
    ]
}
