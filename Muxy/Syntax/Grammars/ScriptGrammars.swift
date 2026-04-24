import Foundation

extension SyntaxGrammar {
    static let python = SyntaxGrammar(
        name: "Python",
        extensions: ["py", "pyi", "pyw"],
        caseSensitiveKeywords: true,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"\"\"", close: "\"\"\"", escape: "\\", multiline: true, scope: .docComment),
            StringRule(id: 2, open: "'''", close: "'''", escape: "\\", multiline: true, scope: .docComment),
            StringRule(id: 3, open: "f\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 4, open: "f'", close: "'", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 5, open: "r\"", close: "\"", escape: nil, multiline: false, scope: .string),
            StringRule(id: 6, open: "r'", close: "'", escape: nil, multiline: false, scope: .string),
            StringRule(id: 7, open: "b\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 8, open: "b'", close: "'", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 9, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 10, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
                "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in",
                "is", "lambda", "match", "case", "nonlocal", "not", "or", "pass", "raise", "return",
                "try", "while", "with", "yield",
            ], scope: .keyword),
            KeywordGroup(words: [
                "True", "False", "None", "NotImplemented", "Ellipsis",
            ], scope: .builtin),
            KeywordGroup(words: [
                "int", "float", "str", "bool", "list", "dict", "tuple", "set", "frozenset", "bytes",
                "bytearray", "complex", "range", "object", "type",
            ], scope: .type),
            KeywordGroup(words: [
                "abs", "all", "any", "ascii", "bin", "breakpoint", "callable", "chr", "classmethod",
                "compile", "delattr", "dir", "divmod", "enumerate", "eval", "exec", "exit", "filter",
                "format", "getattr", "globals", "hasattr", "hash", "help", "hex", "id", "input",
                "isinstance", "issubclass", "iter", "len", "locals", "map", "max", "memoryview", "min",
                "next", "oct", "open", "ord", "pow", "print", "property", "quit", "repr", "reversed",
                "round", "setattr", "slice", "sorted", "staticmethod", "sum", "super", "vars", "zip",
                "self", "cls",
            ], scope: .builtin),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: true,
        atAttributeScope: .attribute,
        highlightFunctionCalls: true,
        highlightAllCapsAsConstant: true,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: SyntaxGrammar.defaultIdentifierBody
    )

    static let ruby: SyntaxGrammar = {
        var identifierStart = SyntaxGrammar.defaultIdentifierStart
        var identifierBody = SyntaxGrammar.defaultIdentifierBody
        identifierBody.insert("?")
        identifierBody.insert("!")
        return SyntaxGrammar(
            name: "Ruby",
            extensions: ["rb", "rake", "gemspec"],
            caseSensitiveKeywords: true,
            lineComments: ["#"],
            lineCommentScope: .comment,
            blockComments: [
                BlockCommentRule(id: 1, open: "=begin", close: "=end", scope: .comment, nestable: false),
            ],
            strings: [
                StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: true, scope: .string),
                StringRule(id: 2, open: "'", close: "'", escape: "\\", multiline: true, scope: .string),
                StringRule(id: 3, open: "%w(", close: ")", escape: "\\", multiline: true, scope: .string),
                StringRule(id: 4, open: "%Q(", close: ")", escape: "\\", multiline: true, scope: .string),
            ],
            keywordGroups: [
                KeywordGroup(words: [
                    "BEGIN", "END", "alias", "and", "begin", "break", "case", "class", "def", "defined?",
                    "do", "else", "elsif", "end", "ensure", "false", "for", "if", "in", "module",
                    "next", "nil", "not", "or", "redo", "rescue", "retry", "return", "self", "super",
                    "then", "true", "undef", "unless", "until", "when", "while", "yield", "require",
                    "require_relative", "include", "extend", "attr_accessor", "attr_reader",
                    "attr_writer", "private", "protected", "public", "lambda", "proc",
                ], scope: .keyword),
                KeywordGroup(words: ["true", "false", "nil", "self"], scope: .builtin),
            ],
            supportsNumbers: true,
            supportsHashDirectives: false,
            hashDirectiveScope: .preprocessor,
            supportsAtAttributes: true,
            atAttributeScope: .variable,
            highlightFunctionCalls: true,
            highlightAllCapsAsConstant: true,
            identifierStart: identifierStart,
            identifierBody: identifierBody
        )
    }()

    static let lua = SyntaxGrammar(
        name: "Lua",
        extensions: ["lua"],
        caseSensitiveKeywords: true,
        lineComments: ["--"],
        lineCommentScope: .comment,
        blockComments: [
            BlockCommentRule(id: 1, open: "--[[", close: "]]", scope: .comment, nestable: false),
        ],
        strings: [
            StringRule(id: 1, open: "[[", close: "]]", escape: nil, multiline: true, scope: .string),
            StringRule(id: 2, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 3, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "and", "break", "do", "else", "elseif", "end", "for", "function", "goto", "if", "in",
                "local", "not", "or", "repeat", "return", "then", "until", "while",
            ], scope: .keyword),
            KeywordGroup(words: ["true", "false", "nil"], scope: .builtin),
            KeywordGroup(words: [
                "assert", "collectgarbage", "dofile", "error", "getmetatable", "ipairs", "load",
                "loadfile", "next", "pairs", "pcall", "print", "rawequal", "rawget", "rawlen",
                "rawset", "require", "select", "setmetatable", "tonumber", "tostring", "type",
                "unpack", "xpcall", "self",
            ], scope: .builtin),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: true,
        highlightAllCapsAsConstant: true,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: SyntaxGrammar.defaultIdentifierBody
    )

    static let shell: SyntaxGrammar = {
        var identifierStart = SyntaxGrammar.defaultIdentifierStart
        var identifierBody = SyntaxGrammar.defaultIdentifierBody
        identifierStart.insert("$")
        identifierBody.insert("$")
        return SyntaxGrammar(
            name: "Shell",
            extensions: ["sh", "bash", "zsh", "ksh", "fish", "bashrc", "zshrc", "profile"],
            caseSensitiveKeywords: true,
            lineComments: ["#"],
            lineCommentScope: .comment,
            blockComments: [],
            strings: [
                StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: true, scope: .string),
                StringRule(id: 2, open: "'", close: "'", escape: nil, multiline: true, scope: .string),
            ],
            keywordGroups: [
                KeywordGroup(words: [
                    "if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while", "until",
                    "case", "esac", "function", "return", "break", "continue", "select", "time",
                    "local", "export", "unset", "readonly", "declare", "typeset", "eval", "exec",
                    "source", "alias", "unalias", "exit", "trap", "set", "shift",
                ], scope: .keyword),
                KeywordGroup(words: [
                    "echo", "printf", "read", "cd", "pwd", "ls", "cat", "grep", "sed", "awk", "cut",
                    "sort", "uniq", "head", "tail", "wc", "find", "xargs", "tr", "test",
                ], scope: .builtin),
                KeywordGroup(words: ["true", "false"], scope: .builtin),
            ],
            supportsNumbers: true,
            supportsHashDirectives: false,
            hashDirectiveScope: .preprocessor,
            supportsAtAttributes: false,
            atAttributeScope: .attribute,
            highlightFunctionCalls: false,
            highlightAllCapsAsConstant: true,
            identifierStart: identifierStart,
            identifierBody: identifierBody
        )
    }()

    static let perl: SyntaxGrammar = {
        var identifierStart = SyntaxGrammar.defaultIdentifierStart
        var identifierBody = SyntaxGrammar.defaultIdentifierBody
        identifierStart.insert("$")
        identifierStart.insert("@")
        identifierStart.insert("%")
        identifierBody.insert("$")
        return SyntaxGrammar(
            name: "Perl",
            extensions: ["pl", "pm", "perl"],
            caseSensitiveKeywords: true,
            lineComments: ["#"],
            lineCommentScope: .comment,
            blockComments: [],
            strings: [
                StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
                StringRule(id: 2, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
            ],
            keywordGroups: [
                KeywordGroup(words: [
                    "and", "cmp", "continue", "do", "else", "elsif", "eq", "exp", "for", "foreach",
                    "ge", "gt", "if", "le", "lock", "lt", "m", "ne", "no", "not", "or", "package",
                    "q", "qq", "qr", "qw", "qx", "return", "s", "sub", "tr", "unless", "until",
                    "while", "xor", "y", "use", "require", "my", "our", "local",
                ], scope: .keyword),
                KeywordGroup(words: [
                    "print", "printf", "say", "warn", "die", "defined", "exists", "delete", "keys",
                    "values", "sort", "reverse", "map", "grep", "chomp", "chop", "split", "join",
                    "length", "substr", "index", "rindex", "sprintf", "scalar", "ref", "bless",
                    "open", "close", "read", "write", "eof", "shift", "unshift", "push", "pop",
                    "splice",
                ], scope: .builtin),
            ],
            supportsNumbers: true,
            supportsHashDirectives: false,
            hashDirectiveScope: .preprocessor,
            supportsAtAttributes: false,
            atAttributeScope: .attribute,
            highlightFunctionCalls: true,
            highlightAllCapsAsConstant: true,
            identifierStart: identifierStart,
            identifierBody: identifierBody
        )
    }()

    static let elixir = SyntaxGrammar(
        name: "Elixir",
        extensions: ["ex", "exs"],
        caseSensitiveKeywords: true,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"\"\"", close: "\"\"\"", escape: "\\", multiline: true, scope: .docComment),
            StringRule(id: 2, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 3, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "def", "defp", "defmodule", "defstruct", "defmacro", "defmacrop", "defguard",
                "defguardp", "defprotocol", "defimpl", "defexception", "do", "end", "if", "unless",
                "else", "case", "cond", "when", "for", "with", "fn", "try", "catch", "rescue",
                "after", "and", "or", "not", "in", "import", "alias", "require", "use", "raise",
                "throw", "receive", "quote", "unquote", "unquote_splicing",
            ], scope: .keyword),
            KeywordGroup(words: ["true", "false", "nil"], scope: .builtin),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: true,
        atAttributeScope: .attribute,
        highlightFunctionCalls: true,
        highlightAllCapsAsConstant: true,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: SyntaxGrammar.defaultIdentifierBody
    )

    static let haskell = SyntaxGrammar(
        name: "Haskell",
        extensions: ["hs", "lhs"],
        caseSensitiveKeywords: true,
        lineComments: ["--"],
        lineCommentScope: .comment,
        blockComments: [
            BlockCommentRule(id: 1, open: "{-", close: "-}", scope: .comment, nestable: true),
        ],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 2, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "case", "class", "data", "default", "deriving", "do", "else", "family", "forall",
                "foreign", "hiding", "if", "import", "in", "infix", "infixl", "infixr", "instance",
                "let", "module", "newtype", "of", "then", "type", "where", "_",
            ], scope: .keyword),
            KeywordGroup(words: ["True", "False", "Nothing", "Just", "Left", "Right"], scope: .builtin),
            KeywordGroup(words: [
                "Int", "Integer", "Float", "Double", "Bool", "Char", "String", "Maybe", "Either",
                "IO", "Ordering", "Functor", "Monad", "Applicative", "Show", "Eq", "Ord",
            ], scope: .type),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: false,
        highlightAllCapsAsConstant: false,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: {
            var set = SyntaxGrammar.defaultIdentifierBody
            set.insert("'")
            return set
        }()
    )

    static let r: SyntaxGrammar = {
        var identifierBody = SyntaxGrammar.defaultIdentifierBody
        identifierBody.insert(".")
        return SyntaxGrammar(
            name: "R",
            extensions: ["r", ".rprofile", ".renviron"],
            caseSensitiveKeywords: true,
            lineComments: ["#"],
            lineCommentScope: .comment,
            blockComments: [],
            strings: [
                StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
                StringRule(id: 2, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
                StringRule(id: 3, open: "`", close: "`", escape: nil, multiline: false, scope: .string),
            ],
            keywordGroups: [
                KeywordGroup(words: [
                    "if", "else", "for", "while", "repeat", "function", "return", "break", "next",
                    "in", "library", "require", "source",
                ], scope: .keyword),
                KeywordGroup(words: [
                    "TRUE", "FALSE", "T", "F", "NULL", "NA", "NA_integer_", "NA_real_", "NA_character_",
                    "NA_complex_", "Inf", "NaN",
                ], scope: .builtin),
                KeywordGroup(words: [
                    "c", "list", "vector", "matrix", "data.frame", "factor", "array", "length", "dim",
                    "names", "nrow", "ncol", "print", "cat", "paste", "paste0", "sprintf", "lapply",
                    "sapply", "mapply", "apply", "Reduce", "Filter", "Map", "do.call", "sum", "mean",
                    "median", "min", "max", "range", "seq", "seq_len", "seq_along", "rep", "which",
                    "any", "all", "is.null", "is.na", "is.numeric", "is.character",
                ], scope: .builtin),
            ],
            supportsNumbers: true,
            supportsHashDirectives: false,
            hashDirectiveScope: .preprocessor,
            supportsAtAttributes: false,
            atAttributeScope: .attribute,
            highlightFunctionCalls: true,
            highlightAllCapsAsConstant: true,
            identifierStart: SyntaxGrammar.defaultIdentifierStart,
            identifierBody: identifierBody
        )
    }()

    static let julia = SyntaxGrammar(
        name: "Julia",
        extensions: ["jl"],
        caseSensitiveKeywords: true,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [
            BlockCommentRule(id: 1, open: "#=", close: "=#", scope: .comment, nestable: true),
        ],
        strings: [
            StringRule(id: 1, open: "\"\"\"", close: "\"\"\"", escape: "\\", multiline: true, scope: .string),
            StringRule(id: 2, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 3, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "abstract", "baremodule", "begin", "break", "catch", "const", "continue", "do", "else",
                "elseif", "end", "export", "finally", "for", "function", "global", "if",
                "import", "in", "isa", "let", "local", "macro", "module", "mutable", "primitive",
                "quote", "return", "struct", "try", "type", "using", "where", "while",
            ], scope: .keyword),
            KeywordGroup(words: ["true", "false", "nothing", "missing", "Inf", "NaN"], scope: .builtin),
            KeywordGroup(words: [
                "Int", "Int8", "Int16", "Int32", "Int64", "Int128", "UInt", "UInt8", "UInt16", "UInt32",
                "UInt64", "UInt128", "Float16", "Float32", "Float64", "Bool", "Char", "String", "Symbol",
                "Array", "Vector", "Matrix", "Tuple", "NamedTuple", "Dict", "Set", "Number", "Integer",
                "AbstractFloat", "AbstractString", "Any", "Nothing", "Missing",
            ], scope: .type),
            KeywordGroup(words: [
                "println", "print", "show", "error", "throw", "length", "size", "push!", "pop!",
                "append!", "insert!", "delete!", "get", "haskey", "keys", "values", "map", "filter",
                "reduce", "sum", "prod", "min", "max", "sort", "reverse", "collect", "enumerate",
                "zip", "typeof", "eltype",
            ], scope: .builtin),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: true,
        atAttributeScope: .attribute,
        highlightFunctionCalls: true,
        highlightAllCapsAsConstant: true,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: {
            var set = SyntaxGrammar.defaultIdentifierBody
            set.insert("!")
            return set
        }()
    )

    static let clojure: SyntaxGrammar = {
        var identifierStart = SyntaxGrammar.defaultIdentifierStart
        identifierStart.insert("-")
        identifierStart.insert("+")
        identifierStart.insert("*")
        identifierStart.insert("?")
        identifierStart.insert("!")
        var identifierBody = SyntaxGrammar.defaultIdentifierBody
        identifierBody.insert("-")
        identifierBody.insert("?")
        identifierBody.insert("!")
        identifierBody.insert("*")
        identifierBody.insert("/")
        identifierBody.insert(".")
        return SyntaxGrammar(
            name: "Clojure",
            extensions: ["clj", "cljs", "cljc", "edn", "boot"],
            caseSensitiveKeywords: true,
            lineComments: [";"],
            lineCommentScope: .comment,
            blockComments: [],
            strings: [
                StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: true, scope: .string),
            ],
            keywordGroups: [
                KeywordGroup(words: [
                    "def", "defn", "defn-", "defmacro", "defmulti", "defmethod", "defprotocol",
                    "defrecord", "deftype", "defstruct", "definterface", "defonce", "fn", "let",
                    "letfn", "if", "if-not", "if-let", "when", "when-not", "when-let", "cond", "condp",
                    "case", "do", "loop", "recur", "for", "doseq", "dotimes", "while", "quote",
                    "var", "ns", "in-ns", "require", "use", "import", "refer", "try", "catch",
                    "finally", "throw", "reify", "proxy", "binding", "set!", "new",
                ], scope: .keyword),
                KeywordGroup(words: ["true", "false", "nil"], scope: .builtin),
                KeywordGroup(words: [
                    "map", "filter", "reduce", "apply", "partial", "comp", "identity", "constantly",
                    "first", "rest", "next", "cons", "conj", "assoc", "dissoc", "get", "get-in",
                    "update", "update-in", "merge", "count", "empty?", "seq", "vec", "vector", "list",
                    "hash-map", "hash-set", "str", "print", "println", "prn", "pr", "range", "repeat",
                    "take", "drop", "concat", "into", "some", "every?", "not", "not=",
                ], scope: .builtin),
            ],
            supportsNumbers: true,
            supportsHashDirectives: false,
            hashDirectiveScope: .preprocessor,
            supportsAtAttributes: false,
            atAttributeScope: .attribute,
            highlightFunctionCalls: true,
            highlightAllCapsAsConstant: false,
            identifierStart: identifierStart,
            identifierBody: identifierBody
        )
    }()

    static let ocaml = SyntaxGrammar(
        name: "OCaml",
        extensions: ["ml", "mli"],
        caseSensitiveKeywords: true,
        lineComments: [],
        lineCommentScope: .comment,
        blockComments: [
            BlockCommentRule(id: 1, open: "(*", close: "*)", scope: .comment, nestable: true),
        ],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: true, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "and", "as", "assert", "asr", "begin", "class", "constraint", "do", "done", "downto",
                "else", "end", "exception", "external", "false", "for", "fun", "function", "functor",
                "if", "in", "include", "inherit", "initializer", "land", "lazy", "let", "lor", "lsl",
                "lsr", "lxor", "match", "method", "mod", "module", "mutable", "new", "nonrec",
                "object", "of", "open", "or", "private", "rec", "sig", "struct", "then", "to", "true",
                "try", "type", "val", "virtual", "when", "while", "with",
            ], scope: .keyword),
            KeywordGroup(words: ["true", "false", "None", "Some", "Ok", "Error"], scope: .builtin),
            KeywordGroup(words: [
                "int", "float", "bool", "char", "string", "unit", "list", "array", "option", "ref",
                "result", "bytes",
            ], scope: .type),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: true,
        highlightAllCapsAsConstant: false,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: {
            var set = SyntaxGrammar.defaultIdentifierBody
            set.insert("'")
            return set
        }()
    )

    static let powershell: SyntaxGrammar = {
        var identifierStart = SyntaxGrammar.defaultIdentifierStart
        identifierStart.insert("$")
        var identifierBody = SyntaxGrammar.defaultIdentifierBody
        identifierBody.insert("$")
        identifierBody.insert("-")
        return SyntaxGrammar(
            name: "PowerShell",
            extensions: ["ps1", "psm1", "psd1"],
            caseSensitiveKeywords: false,
            lineComments: ["#"],
            lineCommentScope: .comment,
            blockComments: [
                BlockCommentRule(id: 1, open: "<#", close: "#>", scope: .comment, nestable: false),
            ],
            strings: [
                StringRule(id: 1, open: "@\"", close: "\"@", escape: "`", multiline: true, scope: .string),
                StringRule(id: 2, open: "@'", close: "'@", escape: nil, multiline: true, scope: .string),
                StringRule(id: 3, open: "\"", close: "\"", escape: "`", multiline: false, scope: .string),
                StringRule(id: 4, open: "'", close: "'", escape: nil, multiline: false, scope: .string),
            ],
            keywordGroups: [
                KeywordGroup(words: [
                    "begin", "break", "catch", "class", "continue", "data", "define", "do", "dynamicparam",
                    "else", "elseif", "end", "enum", "exit", "filter", "finally", "for", "foreach",
                    "from", "function", "hidden", "if", "in", "param", "process", "return", "static",
                    "switch", "throw", "trap", "try", "until", "using", "var", "while", "workflow",
                ], scope: .keyword),
                KeywordGroup(words: [
                    "true", "false", "null",
                ], scope: .builtin),
                KeywordGroup(words: [
                    "Write-Host", "Write-Output", "Write-Error", "Write-Warning", "Write-Verbose",
                    "Write-Debug", "Get-Content", "Set-Content", "Add-Content", "Get-ChildItem",
                    "Get-Item", "Set-Item", "Remove-Item", "Copy-Item", "Move-Item", "New-Item",
                    "Test-Path", "Join-Path", "Split-Path", "Resolve-Path", "Get-Location",
                    "Set-Location", "Push-Location", "Pop-Location", "Invoke-Expression",
                    "Invoke-Command", "Invoke-WebRequest", "Invoke-RestMethod", "Select-Object",
                    "Where-Object", "ForEach-Object", "Sort-Object", "Group-Object", "Measure-Object",
                    "Get-Process", "Stop-Process", "Start-Process", "Get-Service", "Start-Service",
                    "Stop-Service", "Get-Member", "Get-Command", "Get-Help",
                ], scope: .builtin),
            ],
            supportsNumbers: true,
            supportsHashDirectives: false,
            hashDirectiveScope: .preprocessor,
            supportsAtAttributes: false,
            atAttributeScope: .attribute,
            highlightFunctionCalls: true,
            highlightAllCapsAsConstant: true,
            identifierStart: identifierStart,
            identifierBody: identifierBody
        )
    }()
}
