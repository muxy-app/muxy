import AppKit

struct SyntaxHighlightResult {
    let ranges: [(NSRange, NSColor)]
}

@MainActor
final class SyntaxHighlightExtension {
    let fileExtension: String

    init(fileExtension: String) {
        self.fileExtension = fileExtension
    }

    func computeHighlightsAsync(text: String, range: NSRange) async -> SyntaxHighlightResult {
        let resolvedRules = resolveRules()
        return await Task.detached(priority: .userInitiated) {
            let highlights = Self.computeHighlightsSync(text: text, range: range, rules: resolvedRules)
            return SyntaxHighlightResult(ranges: highlights)
        }.value
    }

    func computeHighlightsSync(text: String, range: NSRange) -> SyntaxHighlightResult {
        let resolvedRules = resolveRules()
        let highlights = Self.computeHighlightsSync(text: text, range: range, rules: resolvedRules)
        return SyntaxHighlightResult(ranges: highlights)
    }

    private func resolveRules() -> [ResolvedRule] {
        SyntaxRules.forExtension(fileExtension).compactMap { rule -> ResolvedRule? in
            guard let regex = rule.regex else { return nil }
            return ResolvedRule(regex: regex, color: rule.color(), captureGroup: rule.captureGroup)
        }
    }

    nonisolated private static func computeHighlightsSync(
        text: String,
        range: NSRange,
        rules: [ResolvedRule]
    ) -> [(NSRange, NSColor)] {
        guard !text.isEmpty, range.length > 0 else { return [] }
        var results: [(NSRange, NSColor)] = []
        for rule in rules {
            rule.regex.enumerateMatches(in: text, range: range) { match, _, _ in
                guard let matchRange = match?.range(at: rule.captureGroup) else { return }
                results.append((matchRange, rule.color))
            }
        }
        return results
    }
}

private struct ResolvedRule {
    let regex: NSRegularExpression
    let color: NSColor
    let captureGroup: Int
}

private struct SyntaxRule {
    let pattern: String
    let color: @MainActor () -> NSColor
    let options: NSRegularExpression.Options
    let captureGroup: Int
    let regex: NSRegularExpression?

    init(
        pattern: String,
        color: @escaping @MainActor () -> NSColor,
        options: NSRegularExpression.Options = [],
        captureGroup: Int = 0
    ) {
        self.pattern = pattern
        self.color = color
        self.options = options
        self.captureGroup = captureGroup
        self.regex = try? NSRegularExpression(pattern: pattern, options: options)
    }
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
        case "java": java
        case "kt",
             "kts": kotlin
        case "cs": csharp
        case "php": php
        case "lua": lua
        case "dart": dart
        case "ex",
             "exs": elixir
        case "hs": haskell
        case "scala",
             "sc": scala
        case "zig": zig
        case "sql": sql
        case "xml",
             "plist",
             "xib",
             "storyboard",
             "svg": xml
        case "r": rLang
        case "pl",
             "pm": perl
        case "m",
             "mm": objc
        case "Dockerfile",
             "dockerfile": dockerfile
        case "Makefile",
             "makefile",
             "GNUmakefile": makefile
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

    private static func ciKw(_ words: [String], color: @escaping @MainActor () -> NSColor = keyword) -> SyntaxRule {
        SyntaxRule(
            pattern: "\\b(?:\(words.joined(separator: "|")))\\b",
            color: color,
            options: .caseInsensitive
        )
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

    static let java: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString, sqString,
        kw([
            "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char",
            "class", "const", "continue", "default", "do", "double", "else", "enum",
            "extends", "final", "finally", "float", "for", "goto", "if", "implements",
            "import", "instanceof", "int", "interface", "long", "native", "new",
            "package", "private", "protected", "public", "return", "short", "static",
            "strictfp", "super", "switch", "synchronized", "this", "throw", "throws",
            "transient", "try", "void", "volatile", "while", "true", "false", "null",
            "var", "yield", "record", "sealed", "permits", "non-sealed",
        ]),
        SyntaxRule(pattern: "@[a-zA-Z_][a-zA-Z0-9_.]*", color: function),
        numberLit, funcCall,
    ]

    static let kotlin: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString, sqString,
        SyntaxRule(pattern: "\"\"\"[\\s\\S]*?\"\"\"", color: string, options: .dotMatchesLineSeparators),
        kw([
            "abstract", "actual", "annotation", "as", "break", "by", "catch", "class",
            "companion", "const", "constructor", "continue", "crossinline", "data",
            "delegate", "do", "else", "enum", "expect", "external", "false", "final",
            "finally", "for", "fun", "get", "if", "import", "in", "infix", "init",
            "inline", "inner", "interface", "internal", "is", "lateinit", "noinline",
            "null", "object", "open", "operator", "out", "override", "package",
            "private", "protected", "public", "reified", "return", "sealed", "set",
            "super", "suspend", "tailrec", "this", "throw", "true", "try", "typealias",
            "val", "var", "vararg", "when", "where", "while",
        ]),
        SyntaxRule(pattern: "@[a-zA-Z_][a-zA-Z0-9_.]*", color: function),
        numberLit, funcCall,
    ]

    static let csharp: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString, sqString,
        SyntaxRule(pattern: "@\"(?:[^\"]|\"\")*\"", color: string),
        kw([
            "abstract", "as", "base", "bool", "break", "byte", "case", "catch", "char",
            "checked", "class", "const", "continue", "decimal", "default", "delegate",
            "do", "double", "else", "enum", "event", "explicit", "extern", "false",
            "finally", "fixed", "float", "for", "foreach", "goto", "if", "implicit",
            "in", "int", "interface", "internal", "is", "lock", "long", "namespace",
            "new", "null", "object", "operator", "out", "override", "params", "private",
            "protected", "public", "readonly", "ref", "return", "sbyte", "sealed",
            "short", "sizeof", "stackalloc", "static", "string", "struct", "switch",
            "this", "throw", "true", "try", "typeof", "uint", "ulong", "unchecked",
            "unsafe", "ushort", "using", "var", "virtual", "void", "volatile", "while",
            "async", "await", "record", "init", "required", "global",
        ]),
        SyntaxRule(pattern: "\\[\\w+(?:\\([^)]*\\))?\\]", color: function),
        numberLit, funcCall,
    ]

    static let php: [SyntaxRule] = [
        lineComment("//"), lineComment("#"), blockComment,
        dqString, sqString,
        kw([
            "abstract", "and", "array", "as", "break", "callable", "case", "catch",
            "class", "clone", "const", "continue", "declare", "default", "die", "do",
            "echo", "else", "elseif", "empty", "enddeclare", "endfor", "endforeach",
            "endif", "endswitch", "endwhile", "eval", "exit", "extends", "final",
            "finally", "fn", "for", "foreach", "function", "global", "goto", "if",
            "implements", "include", "include_once", "instanceof", "insteadof",
            "interface", "isset", "list", "match", "namespace", "new", "or", "print",
            "private", "protected", "public", "readonly", "require", "require_once",
            "return", "static", "switch", "throw", "trait", "try", "unset", "use",
            "var", "while", "xor", "yield", "true", "false", "null", "enum",
        ]),
        SyntaxRule(pattern: "\\$[a-zA-Z_][a-zA-Z0-9_]*", color: type),
        numberLit, funcCall,
    ]

    static let lua: [SyntaxRule] = [
        SyntaxRule(pattern: "--\\[\\[[\\s\\S]*?\\]\\]", color: comment, options: .dotMatchesLineSeparators),
        lineComment("--"),
        SyntaxRule(pattern: "\\[\\[[\\s\\S]*?\\]\\]", color: string, options: .dotMatchesLineSeparators),
        dqString, sqString,
        kw([
            "and", "break", "do", "else", "elseif", "end", "false", "for", "function",
            "goto", "if", "in", "local", "nil", "not", "or", "repeat", "return",
            "then", "true", "until", "while",
        ]),
        numberLit, funcCall,
    ]

    static let dart: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString, sqString,
        SyntaxRule(pattern: "\"\"\"[\\s\\S]*?\"\"\"", color: string, options: .dotMatchesLineSeparators),
        SyntaxRule(pattern: "'''[\\s\\S]*?'''", color: string, options: .dotMatchesLineSeparators),
        kw([
            "abstract", "as", "assert", "async", "await", "base", "break", "case",
            "catch", "class", "const", "continue", "covariant", "default", "deferred",
            "do", "dynamic", "else", "enum", "export", "extends", "extension",
            "external", "factory", "false", "final", "finally", "for", "Function",
            "get", "hide", "if", "implements", "import", "in", "interface", "is",
            "late", "library", "mixin", "new", "null", "on", "operator", "part",
            "required", "rethrow", "return", "sealed", "set", "show", "static",
            "super", "switch", "sync", "this", "throw", "true", "try", "typedef",
            "var", "void", "when", "while", "with", "yield",
        ]),
        SyntaxRule(pattern: "@[a-zA-Z_][a-zA-Z0-9_.]*", color: function),
        numberLit, funcCall,
    ]

    static let elixir: [SyntaxRule] = [
        lineComment("#"),
        SyntaxRule(pattern: "\"\"\"[\\s\\S]*?\"\"\"", color: string, options: .dotMatchesLineSeparators),
        dqString, sqString,
        kw([
            "after", "alias", "and", "case", "catch", "cond", "def", "defp",
            "defmodule", "defstruct", "defprotocol", "defimpl", "defmacro", "defmacrop",
            "defguard", "defdelegate", "defexception", "defoverridable", "do", "else",
            "end", "false", "fn", "for", "if", "import", "in", "nil", "not", "or",
            "quote", "raise", "receive", "require", "rescue", "true", "try", "unless",
            "unquote", "use", "when", "with",
        ]),
        SyntaxRule(pattern: ":[a-zA-Z_][a-zA-Z0-9_]*", color: string),
        SyntaxRule(pattern: "@[a-zA-Z_][a-zA-Z0-9_]*", color: type),
        numberLit, funcCall,
    ]

    static let haskell: [SyntaxRule] = [
        SyntaxRule(pattern: "\\{-[\\s\\S]*?-\\}", color: comment, options: .dotMatchesLineSeparators),
        lineComment("--"),
        dqString, sqString,
        kw([
            "as", "case", "class", "data", "default", "deriving", "do", "else",
            "forall", "foreign", "hiding", "if", "import", "in", "infix", "infixl",
            "infixr", "instance", "let", "module", "newtype", "of", "qualified",
            "then", "type", "where", "True", "False",
        ]),
        SyntaxRule(pattern: "\\b[A-Z][a-zA-Z0-9_']*", color: type),
        numberLit, funcCall,
    ]

    static let scala: [SyntaxRule] = [
        lineComment("//"), blockComment, dqString, sqString,
        SyntaxRule(pattern: "\"\"\"[\\s\\S]*?\"\"\"", color: string, options: .dotMatchesLineSeparators),
        kw([
            "abstract", "case", "catch", "class", "def", "do", "else", "enum",
            "export", "extends", "extension", "false", "final", "finally", "for",
            "given", "if", "implicit", "import", "lazy", "match", "new", "null",
            "object", "override", "package", "private", "protected", "return",
            "sealed", "super", "then", "this", "throw", "trait", "true", "try",
            "type", "val", "var", "while", "with", "yield",
        ]),
        SyntaxRule(pattern: "@[a-zA-Z_][a-zA-Z0-9_.]*", color: function),
        numberLit, funcCall,
    ]

    static let zig: [SyntaxRule] = [
        lineComment("//"), dqString, sqString,
        kw([
            "addrspace", "align", "allowzero", "and", "anyframe", "anytype", "asm",
            "async", "await", "break", "callconv", "catch", "comptime", "const",
            "continue", "defer", "else", "enum", "errdefer", "error", "export",
            "extern", "false", "fn", "for", "if", "inline", "linksection",
            "noalias", "nosuspend", "null", "opaque", "or", "orelse", "packed",
            "pub", "resume", "return", "struct", "suspend", "switch", "test",
            "threadlocal", "true", "try", "undefined", "union", "unreachable",
            "usingnamespace", "var", "volatile", "while",
        ]),
        SyntaxRule(pattern: "@[a-zA-Z_][a-zA-Z0-9_]*", color: function),
        numberLit, funcCall,
    ]

    static let sql: [SyntaxRule] = [
        lineComment("--"), blockComment,
        dqString, sqString,
        ciKw([
            "ADD", "ALL", "ALTER", "AND", "AS", "ASC", "BEGIN", "BETWEEN", "BY",
            "CASCADE", "CASE", "CHECK", "COLUMN", "COMMIT", "CONSTRAINT", "CREATE",
            "CROSS", "DATABASE", "DEFAULT", "DELETE", "DESC", "DISTINCT", "DROP",
            "ELSE", "END", "EXCEPT", "EXISTS", "FALSE", "FETCH", "FOREIGN", "FROM",
            "FULL", "GRANT", "GROUP", "HAVING", "IF", "IN", "INDEX", "INNER", "INSERT",
            "INTERSECT", "INTO", "IS", "JOIN", "KEY", "LEFT", "LIKE", "LIMIT", "NOT",
            "NULL", "OFFSET", "ON", "OR", "ORDER", "OUTER", "PRIMARY", "REFERENCES",
            "REPLACE", "RETURNING", "REVOKE", "RIGHT", "ROLLBACK", "SELECT", "SET",
            "TABLE", "THEN", "TO", "TRANSACTION", "TRUE", "TRUNCATE", "UNION",
            "UNIQUE", "UPDATE", "USING", "VALUES", "VIEW", "WHEN", "WHERE", "WITH",
        ]),
        ciKw([
            "INT", "INTEGER", "BIGINT", "SMALLINT", "TINYINT", "FLOAT", "DOUBLE",
            "DECIMAL", "NUMERIC", "REAL", "CHAR", "VARCHAR", "TEXT", "BLOB", "DATE",
            "TIME", "TIMESTAMP", "DATETIME", "BOOLEAN", "SERIAL", "UUID",
        ], color: type),
        numberLit,
    ]

    static let xml: [SyntaxRule] = [
        SyntaxRule(pattern: "<!--[\\s\\S]*?-->", color: comment, options: .dotMatchesLineSeparators),
        SyntaxRule(pattern: "<\\?.*?\\?>", color: function),
        SyntaxRule(pattern: "<!\\w+[^>]*>", color: function),
        SyntaxRule(pattern: "</?[a-zA-Z_][a-zA-Z0-9_:.-]*", color: keyword),
        SyntaxRule(pattern: "/?>", color: keyword),
        SyntaxRule(pattern: "\\b[a-zA-Z_:][a-zA-Z0-9_:.-]*(?==)", color: function),
        dqString, sqString,
    ]

    static let rLang: [SyntaxRule] = [
        lineComment("#"), dqString, sqString,
        kw([
            "break", "else", "for", "function", "if", "in", "next", "repeat",
            "return", "while", "TRUE", "FALSE", "NULL", "NA", "NA_integer_",
            "NA_real_", "NA_complex_", "NA_character_", "Inf", "NaN",
            "library", "require", "source",
        ]),
        SyntaxRule(pattern: "<-|->|<<-|->>", color: keyword),
        numberLit, funcCall,
    ]

    static let perl: [SyntaxRule] = [
        lineComment("#"), dqString, sqString,
        kw([
            "chomp", "chop", "chr", "crypt", "die", "do", "dump", "each", "else",
            "elsif", "eval", "exit", "for", "foreach", "goto", "grep", "if", "keys",
            "last", "local", "map", "my", "next", "no", "our", "package", "pop",
            "print", "push", "redo", "require", "return", "say", "shift", "sort",
            "sub", "tr", "unless", "unshift", "until", "use", "values", "while",
        ]),
        SyntaxRule(pattern: "\\$[a-zA-Z_][a-zA-Z0-9_]*", color: type),
        SyntaxRule(pattern: "@[a-zA-Z_][a-zA-Z0-9_]*", color: type),
        SyntaxRule(pattern: "%[a-zA-Z_][a-zA-Z0-9_]*", color: type),
        numberLit, funcCall,
    ]

    private static let objcDirectives = [
        "interface", "implementation", "protocol", "end", "property",
        "synthesize", "dynamic", "selector", "encode", "class",
        "public", "private", "protected", "optional", "required",
        "throw", "try", "catch", "finally", "autoreleasepool",
        "synchronized", "import",
    ]

    static let objc: [SyntaxRule] = cLang + [
        kw([
            "class", "id", "self", "super", "nil", "Nil", "YES", "NO",
            "instancetype", "nonatomic", "strong", "weak", "copy", "assign",
            "readonly", "readwrite", "atomic", "retain", "nullable", "nonnull",
        ]),
        SyntaxRule(
            pattern: "@(?:\(objcDirectives.joined(separator: "|")))\\b",
            color: keyword
        ),
        SyntaxRule(pattern: "@\"(?:[^\"\\\\]|\\\\.)*\"", color: string),
    ]

    static let dockerfile: [SyntaxRule] = [
        lineComment("#"), dqString, sqString,
        kw([
            "FROM", "AS", "RUN", "CMD", "LABEL", "MAINTAINER", "EXPOSE", "ENV",
            "ADD", "COPY", "ENTRYPOINT", "VOLUME", "USER", "WORKDIR", "ARG",
            "ONBUILD", "STOPSIGNAL", "HEALTHCHECK", "SHELL",
        ]),
        SyntaxRule(pattern: "\\$\\{?[a-zA-Z_][a-zA-Z0-9_]*\\}?", color: type),
        numberLit,
    ]

    static let makefile: [SyntaxRule] = [
        lineComment("#"), dqString, sqString,
        SyntaxRule(pattern: "^[a-zA-Z_][a-zA-Z0-9_.-]*\\s*(?::?=|\\?=|\\+=)", color: type, options: .anchorsMatchLines),
        SyntaxRule(pattern: "^[a-zA-Z_][a-zA-Z0-9_./-]*(?=\\s*:(?!=))", color: function, options: .anchorsMatchLines),
        SyntaxRule(pattern: "\\$[({][a-zA-Z_][a-zA-Z0-9_]*[)}]", color: type),
        SyntaxRule(pattern: "\\$[@<^?*%+]", color: type),
        kw([
            "ifeq", "ifneq", "ifdef", "ifndef", "else", "endif", "define",
            "endef", "include", "override", "export", "unexport", "vpath",
            ".PHONY", ".DEFAULT", ".PRECIOUS", ".INTERMEDIATE", ".SECONDARY",
            ".SUFFIXES", ".DELETE_ON_ERROR",
        ]),
    ]
}
