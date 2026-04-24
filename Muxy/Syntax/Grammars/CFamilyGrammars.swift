import Foundation

struct CLikeBlockComment {
    let open: String
    let close: String
    let nestable: Bool

    static let slashStar = CLikeBlockComment(open: "/*", close: "*/", nestable: false)
}

extension SyntaxGrammar {
    static func makeCLike(
        name: String,
        extensions: [String],
        keywords: Set<String>,
        storage: Set<String> = [],
        types: Set<String> = [],
        builtins: Set<String> = [],
        extraStrings: [StringRule] = [],
        lineComments: [String] = ["//"],
        blockComment: CLikeBlockComment? = .slashStar,
        supportsHashDirectives: Bool = false,
        hashDirectiveScope: SyntaxScope = .preprocessor,
        supportsAtAttributes: Bool = false,
        atAttributeScope: SyntaxScope = .attribute,
        highlightFunctionCalls: Bool = true,
        allowDollarInIdentifier: Bool = false,
        jsxAware: Bool = false
    ) -> SyntaxGrammar {
        var identifierStart = defaultIdentifierStart
        var identifierBody = defaultIdentifierBody
        if allowDollarInIdentifier {
            identifierStart.insert("$")
            identifierBody.insert("$")
        }

        var keywordGroups: [KeywordGroup] = []
        if !keywords.isEmpty { keywordGroups.append(KeywordGroup(words: keywords, scope: .keyword)) }
        if !storage.isEmpty { keywordGroups.append(KeywordGroup(words: storage, scope: .storage)) }
        if !types.isEmpty { keywordGroups.append(KeywordGroup(words: types, scope: .type)) }
        if !builtins.isEmpty { keywordGroups.append(KeywordGroup(words: builtins, scope: .builtin)) }

        var strings: [StringRule] = [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 2, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
        ]
        strings.append(contentsOf: extraStrings)

        var blocks: [BlockCommentRule] = []
        if let blockComment {
            blocks.append(BlockCommentRule(
                id: 1,
                open: blockComment.open,
                close: blockComment.close,
                scope: .comment,
                nestable: blockComment.nestable
            ))
        }

        return SyntaxGrammar(
            name: name,
            extensions: extensions,
            caseSensitiveKeywords: true,
            lineComments: lineComments,
            lineCommentScope: .comment,
            blockComments: blocks,
            strings: strings,
            keywordGroups: keywordGroups,
            supportsNumbers: true,
            supportsHashDirectives: supportsHashDirectives,
            hashDirectiveScope: hashDirectiveScope,
            supportsAtAttributes: supportsAtAttributes,
            atAttributeScope: atAttributeScope,
            highlightFunctionCalls: highlightFunctionCalls,
            highlightAllCapsAsConstant: true,
            identifierStart: identifierStart,
            identifierBody: identifierBody,
            jsxAware: jsxAware
        )
    }

    static let swift = makeCLike(
        name: "Swift",
        extensions: ["swift"],
        keywords: [
            "as", "associatedtype", "break", "case", "catch", "class", "continue", "default", "defer",
            "deinit", "do", "else", "enum", "extension", "fallthrough", "fileprivate", "for", "func",
            "guard", "if", "import", "in", "init", "inout", "internal", "is", "let", "open", "operator",
            "private", "protocol", "public", "repeat", "return", "rethrows", "self", "Self", "static",
            "struct", "subscript", "super", "switch", "throw", "throws", "try", "typealias", "var",
            "where", "while", "async", "await", "actor", "nonisolated", "isolated", "mutating",
            "nonmutating", "package", "any", "some", "macro", "indirect", "lazy", "weak", "unowned",
            "dynamic", "convenience", "required", "override", "final", "discardableResult",
        ],
        types: [
            "Int", "UInt", "Int8", "Int16", "Int32", "Int64", "UInt8", "UInt16", "UInt32", "UInt64",
            "Float", "Double", "Bool", "String", "Character", "Array", "Dictionary", "Set", "Optional",
            "Result", "Error", "Void", "Any", "AnyObject", "AnyClass", "Substring", "Data", "URL",
            "Date", "CGFloat", "CGRect", "CGSize", "CGPoint", "NSObject", "NSString", "NSArray",
            "NSDictionary", "UUID", "TimeInterval", "Task",
        ],
        builtins: ["true", "false", "nil"],
        supportsAtAttributes: true
    )

    static let objectiveC = makeCLike(
        name: "Objective-C",
        extensions: ["m", "mm", "h"],
        keywords: [
            "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
            "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
            "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
            "union", "unsigned", "void", "volatile", "while", "self", "super", "id",
            "@interface", "@implementation", "@end", "@protocol", "@property", "@synthesize",
            "@class", "@selector", "@autoreleasepool", "@try", "@catch", "@finally", "@throw",
            "@synchronized", "@encode", "@import",
        ],
        types: [
            "NSString", "NSArray", "NSDictionary", "NSNumber", "NSObject", "NSInteger", "NSUInteger",
            "BOOL", "CGFloat", "CGRect", "CGSize", "CGPoint", "IBOutlet", "IBAction", "instancetype",
        ],
        builtins: ["YES", "NO", "nil", "Nil", "NULL", "true", "false"],
        supportsHashDirectives: true,
        supportsAtAttributes: true,
        atAttributeScope: .keyword
    )

    static let c = makeCLike(
        name: "C",
        extensions: ["c"],
        keywords: [
            "auto", "break", "case", "const", "continue", "default", "do", "else", "enum", "extern",
            "for", "goto", "if", "inline", "register", "restrict", "return", "sizeof", "static",
            "struct", "switch", "typedef", "union", "volatile", "while", "_Alignas", "_Alignof",
            "_Atomic", "_Bool", "_Complex", "_Generic", "_Imaginary", "_Noreturn", "_Static_assert",
            "_Thread_local",
        ],
        types: [
            "char", "double", "float", "int", "long", "short", "signed", "unsigned", "void",
            "size_t", "ssize_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t",
            "int32_t", "int64_t", "intptr_t", "uintptr_t", "ptrdiff_t", "FILE", "bool",
        ],
        builtins: ["NULL", "true", "false"],
        supportsHashDirectives: true
    )

    static let cpp = makeCLike(
        name: "C++",
        extensions: ["cpp", "cxx", "cc", "hpp", "hxx", "hh"],
        keywords: [
            "alignas", "alignof", "and", "and_eq", "asm", "auto", "bitand", "bitor", "break",
            "case", "catch", "class", "compl", "concept", "const", "consteval", "constexpr",
            "constinit", "const_cast", "continue", "co_await", "co_return", "co_yield", "decltype",
            "default", "delete", "do", "dynamic_cast", "else", "enum", "explicit", "export", "extern",
            "final", "for", "friend", "goto", "if", "inline", "mutable", "namespace", "new",
            "noexcept", "not", "not_eq", "operator", "or", "or_eq", "override", "private", "protected",
            "public", "register", "reinterpret_cast", "requires", "return", "sizeof", "static",
            "static_assert", "static_cast", "struct", "switch", "template", "this", "thread_local",
            "throw", "try", "typedef", "typeid", "typename", "union", "using", "virtual", "volatile",
            "while", "xor", "xor_eq",
        ],
        types: [
            "bool", "char", "char8_t", "char16_t", "char32_t", "double", "float", "int", "long",
            "short", "signed", "unsigned", "void", "wchar_t", "size_t", "ssize_t", "uint8_t",
            "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t", "int64_t",
            "string", "string_view", "vector", "map", "unordered_map", "set", "array", "pair",
            "tuple", "optional", "variant", "shared_ptr", "unique_ptr", "weak_ptr",
        ],
        builtins: ["nullptr", "true", "false", "NULL"],
        supportsHashDirectives: true
    )

    static let csharp = makeCLike(
        name: "C#",
        extensions: ["cs"],
        keywords: [
            "abstract", "as", "base", "break", "case", "catch", "checked", "class", "const",
            "continue", "default", "delegate", "do", "else", "enum", "event", "explicit", "extern",
            "finally", "fixed", "for", "foreach", "goto", "if", "implicit", "in", "interface",
            "internal", "is", "lock", "namespace", "new", "operator", "out", "override", "params",
            "private", "protected", "public", "readonly", "ref", "return", "sealed", "sizeof",
            "stackalloc", "static", "struct", "switch", "this", "throw", "try", "typeof", "unchecked",
            "unsafe", "using", "virtual", "volatile", "while", "async", "await", "yield", "get",
            "set", "add", "remove", "where", "select", "from", "group", "into", "orderby", "join",
            "let", "on", "equals", "by", "ascending", "descending", "dynamic", "var", "record",
            "init", "with", "global", "nameof",
        ],
        types: [
            "bool", "byte", "char", "decimal", "double", "float", "int", "long", "object", "sbyte",
            "short", "string", "uint", "ulong", "ushort", "void", "nint", "nuint",
        ],
        builtins: ["true", "false", "null"],
        supportsHashDirectives: true
    )

    static let java = makeCLike(
        name: "Java",
        extensions: ["java"],
        keywords: [
            "abstract", "assert", "break", "case", "catch", "class", "const", "continue", "default",
            "do", "else", "enum", "extends", "final", "finally", "for", "goto", "if", "implements",
            "import", "instanceof", "interface", "native", "new", "package", "private", "protected",
            "public", "return", "static", "strictfp", "super", "switch", "synchronized", "this",
            "throw", "throws", "transient", "try", "var", "volatile", "while", "yield", "record",
            "sealed", "permits", "non-sealed",
        ],
        types: [
            "boolean", "byte", "char", "double", "float", "int", "long", "short", "void", "String",
            "Integer", "Long", "Boolean", "Object", "Class", "List", "Map", "Set", "ArrayList",
            "HashMap", "HashSet",
        ],
        builtins: ["true", "false", "null"],
        supportsAtAttributes: true
    )

    static let kotlin = makeCLike(
        name: "Kotlin",
        extensions: ["kt", "kts"],
        keywords: [
            "as", "break", "class", "continue", "do", "else", "false", "for", "fun", "if", "in",
            "interface", "is", "null", "object", "package", "return", "super", "this", "throw",
            "true", "try", "typealias", "typeof", "val", "var", "when", "while", "by", "catch",
            "constructor", "delegate", "dynamic", "field", "file", "finally", "get", "import",
            "init", "param", "property", "receiver", "set", "setparam", "value", "where", "actual",
            "abstract", "annotation", "companion", "const", "crossinline", "data", "enum", "expect",
            "external", "final", "infix", "inline", "inner", "internal", "lateinit", "noinline",
            "open", "operator", "out", "override", "private", "protected", "public", "reified",
            "sealed", "suspend", "tailrec", "vararg",
        ],
        types: [
            "Any", "Boolean", "Byte", "Char", "Double", "Float", "Int", "Long", "Nothing", "Short",
            "String", "Unit", "Array", "List", "Map", "Set", "Pair", "Triple",
        ],
        builtins: ["true", "false", "null"],
        supportsAtAttributes: true,
        allowDollarInIdentifier: true
    )

    static let scala = makeCLike(
        name: "Scala",
        extensions: ["scala", "sc"],
        keywords: [
            "abstract", "case", "catch", "class", "def", "do", "else", "extends", "false", "final",
            "finally", "for", "forSome", "if", "implicit", "import", "lazy", "match", "new", "null",
            "object", "override", "package", "private", "protected", "return", "sealed", "super",
            "this", "throw", "trait", "try", "true", "type", "val", "var", "while", "with", "yield",
            "given", "using", "enum", "export", "extension", "derives", "end", "then",
        ],
        types: [
            "Any", "AnyRef", "AnyVal", "Boolean", "Byte", "Char", "Double", "Float", "Int", "Long",
            "Nothing", "Short", "String", "Unit", "Array", "List", "Map", "Option", "Seq", "Set",
            "Either", "Future", "Try",
        ],
        builtins: ["true", "false", "null"],
        supportsAtAttributes: true
    )

    static let go = makeCLike(
        name: "Go",
        extensions: ["go"],
        keywords: [
            "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough",
            "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range",
            "return", "select", "struct", "switch", "type", "var",
        ],
        types: [
            "bool", "byte", "complex64", "complex128", "error", "float32", "float64", "int", "int8",
            "int16", "int32", "int64", "rune", "string", "uint", "uint8", "uint16", "uint32",
            "uint64", "uintptr", "any",
        ],
        builtins: [
            "true", "false", "nil", "iota", "append", "cap", "close", "copy", "delete", "len",
            "make", "new", "panic", "print", "println", "recover",
        ],
        extraStrings: [
            SyntaxGrammar.StringRule(id: 3, open: "`", close: "`", escape: nil, multiline: true, scope: .string),
        ]
    )

    static let rust = makeCLike(
        name: "Rust",
        extensions: ["rs"],
        keywords: [
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box",
            "do", "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
            "union",
        ],
        types: [
            "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "str", "u8",
            "u16", "u32", "u64", "u128", "usize", "String", "Vec", "Option", "Result", "Box",
            "HashMap", "HashSet", "BTreeMap", "BTreeSet", "Rc", "Arc", "RefCell", "Cell", "Mutex",
        ],
        builtins: ["true", "false", "None", "Some", "Ok", "Err"],
        extraStrings: [
            SyntaxGrammar.StringRule(id: 3, open: "b\"", close: "\"", escape: "\\", multiline: false, scope: .string),
        ],
        blockComment: CLikeBlockComment(open: "/*", close: "*/", nestable: true),
        supportsAtAttributes: false
    )

    static let zig = makeCLike(
        name: "Zig",
        extensions: ["zig", "zon"],
        keywords: [
            "addrspace", "align", "allowzero", "and", "anyframe", "anytype", "asm", "async", "await",
            "break", "callconv", "catch", "comptime", "const", "continue", "defer", "else", "enum",
            "errdefer", "error", "export", "extern", "fn", "for", "if", "inline", "linksection",
            "noalias", "noinline", "nosuspend", "opaque", "or", "orelse", "packed", "pub", "resume",
            "return", "struct", "suspend", "switch", "test", "threadlocal", "try", "union", "unreachable",
            "usingnamespace", "var", "volatile", "while",
        ],
        types: [
            "bool", "void", "noreturn", "type", "anyerror", "anyopaque", "comptime_int", "comptime_float",
            "isize", "usize", "i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64", "i128", "u128",
            "f16", "f32", "f64", "f80", "f128", "c_char", "c_short", "c_ushort", "c_int", "c_uint",
            "c_long", "c_ulong", "c_longlong", "c_ulonglong", "c_longdouble",
        ],
        builtins: ["true", "false", "null", "undefined"],
        blockComment: nil,
        supportsAtAttributes: true,
        atAttributeScope: .builtin
    )

    static let dart = makeCLike(
        name: "Dart",
        extensions: ["dart"],
        keywords: [
            "abstract", "as", "assert", "async", "await", "break", "case", "catch", "class", "const",
            "continue", "covariant", "default", "deferred", "do", "dynamic", "else", "enum", "export",
            "extends", "extension", "external", "factory", "final", "finally", "for", "Function",
            "get", "hide", "if", "implements", "import", "in", "interface", "is", "late", "library",
            "mixin", "new", "null", "on", "operator", "part", "required", "rethrow", "return", "set",
            "show", "static", "super", "switch", "sync", "this", "throw", "try", "typedef", "var",
            "void", "while", "with", "yield",
        ],
        types: [
            "bool", "double", "int", "num", "String", "List", "Map", "Set", "Object", "Iterable",
            "Future", "Stream",
        ],
        builtins: ["true", "false", "null"],
        extraStrings: [
            SyntaxGrammar.StringRule(
                id: 3,
                open: "\"\"\"",
                close: "\"\"\"",
                escape: "\\",
                multiline: true,
                scope: .string
            ),
            SyntaxGrammar.StringRule(id: 4, open: "'''", close: "'''", escape: "\\", multiline: true, scope: .string),
        ],
        supportsAtAttributes: true,
        allowDollarInIdentifier: true
    )

    static let javascript = makeCLike(
        name: "JavaScript",
        extensions: ["js", "jsx", "mjs", "cjs"],
        keywords: [
            "break", "case", "catch", "class", "continue", "debugger", "default", "delete", "do",
            "else", "export", "extends", "finally", "for", "from", "function", "if", "import", "in",
            "instanceof", "new", "of", "return", "super", "switch", "this", "throw", "try", "typeof",
            "void", "while", "with", "yield", "async", "await", "static", "get", "set",
        ],
        storage: ["var", "let", "const"],
        types: [],
        builtins: [
            "true", "false", "null", "undefined", "NaN", "Infinity", "globalThis", "window",
            "document", "console", "Math", "JSON", "Object", "Array", "String", "Number", "Boolean",
            "Symbol", "BigInt", "Promise", "Map", "Set", "WeakMap", "WeakSet", "Date", "RegExp",
            "Error", "TypeError", "RangeError", "SyntaxError",
        ],
        extraStrings: [
            SyntaxGrammar.StringRule(id: 3, open: "`", close: "`", escape: "\\", multiline: true, scope: .string),
        ],
        allowDollarInIdentifier: true,
        jsxAware: true
    )

    static let typescript = makeCLike(
        name: "TypeScript",
        extensions: ["ts", "tsx", "mts", "cts"],
        keywords: [
            "break", "case", "catch", "class", "continue", "debugger", "default", "delete", "do",
            "else", "export", "extends", "finally", "for", "from", "function", "if", "import", "in",
            "instanceof", "new", "of", "return", "super", "switch", "this", "throw", "try", "typeof",
            "void", "while", "with", "yield", "async", "await", "static", "get", "set", "abstract",
            "as", "asserts", "constructor", "declare", "enum", "implements", "interface", "is",
            "keyof", "module", "namespace", "override", "private", "protected", "public", "readonly",
            "require", "satisfies", "type",
        ],
        storage: ["var", "let", "const"],
        types: [
            "any", "bigint", "boolean", "never", "null", "number", "object", "string", "symbol",
            "undefined", "unknown", "void", "Array", "Record", "Partial", "Required", "Readonly",
            "Pick", "Omit", "Promise",
        ],
        builtins: [
            "true", "false", "null", "undefined", "NaN", "Infinity", "globalThis", "window",
            "document", "console", "Math", "JSON",
        ],
        extraStrings: [
            SyntaxGrammar.StringRule(id: 3, open: "`", close: "`", escape: "\\", multiline: true, scope: .string),
        ],
        supportsAtAttributes: true,
        allowDollarInIdentifier: true,
        jsxAware: true
    )

    static let php = makeCLike(
        name: "PHP",
        extensions: ["php", "phtml"],
        keywords: [
            "abstract", "and", "array", "as", "break", "callable", "case", "catch", "class", "clone",
            "const", "continue", "declare", "default", "die", "do", "echo", "else", "elseif", "empty",
            "enddeclare", "endfor", "endforeach", "endif", "endswitch", "endwhile", "enum", "exit",
            "extends", "final", "finally", "fn", "for", "foreach", "function", "global", "goto", "if",
            "implements", "include", "include_once", "instanceof", "insteadof", "interface", "isset",
            "list", "match", "namespace", "new", "or", "print", "private", "protected", "public",
            "readonly", "require", "require_once", "return", "static", "switch", "throw", "trait",
            "try", "unset", "use", "var", "while", "xor", "yield", "self", "parent",
        ],
        types: [
            "bool", "boolean", "float", "int", "integer", "string", "void", "iterable", "object",
            "mixed", "never", "false", "null", "true",
        ],
        builtins: ["true", "false", "null", "TRUE", "FALSE", "NULL"],
        supportsHashDirectives: false,
        supportsAtAttributes: false,
        allowDollarInIdentifier: true
    )
}
