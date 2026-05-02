import Foundation

extension SyntaxGrammar {
    static let json = SyntaxGrammar(
        name: "JSON",
        extensions: ["json", "jsonc", "json5"],
        caseSensitiveKeywords: true,
        lineComments: ["//"],
        lineCommentScope: .comment,
        blockComments: [
            BlockCommentRule(id: 1, open: "/*", close: "*/", scope: .comment, nestable: false),
        ],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: ["true", "false", "null"], scope: .builtin),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: false,
        highlightAllCapsAsConstant: false,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: SyntaxGrammar.defaultIdentifierBody
    )

    static let yaml = SyntaxGrammar(
        name: "YAML",
        extensions: ["yaml", "yml"],
        caseSensitiveKeywords: true,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 2, open: "'", close: "'", escape: nil, multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "true", "false", "yes", "no", "on", "off", "null", "True", "False", "Yes", "No",
                "On", "Off", "Null", "NULL", "TRUE", "FALSE", "~",
            ], scope: .builtin),
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
            set.insert("-")
            set.insert(".")
            return set
        }(),
        yamlAware: true
    )

    static let toml = SyntaxGrammar(
        name: "TOML",
        extensions: ["toml"],
        caseSensitiveKeywords: true,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"\"\"", close: "\"\"\"", escape: "\\", multiline: true, scope: .string),
            StringRule(id: 2, open: "'''", close: "'''", escape: nil, multiline: true, scope: .string),
            StringRule(id: 3, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 4, open: "'", close: "'", escape: nil, multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: ["true", "false"], scope: .builtin),
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
            set.insert("-")
            set.insert(".")
            return set
        }()
    )

    static let ini = SyntaxGrammar(
        name: "INI",
        extensions: ["ini", "cfg", "conf", "properties", "editorconfig"],
        caseSensitiveKeywords: false,
        lineComments: [";", "#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 2, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: ["true", "false", "yes", "no", "on", "off"], scope: .builtin),
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
            set.insert("-")
            set.insert(".")
            return set
        }()
    )

    static let dotenv = SyntaxGrammar(
        name: "DotEnv",
        extensions: ["env"],
        caseSensitiveKeywords: true,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 2, open: "'", close: "'", escape: nil, multiline: false, scope: .string),
            StringRule(id: 3, open: "`", close: "`", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: ["export"], scope: .keyword),
            KeywordGroup(words: ["true", "false", "null"], scope: .builtin),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: false,
        highlightAllCapsAsConstant: true,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: SyntaxGrammar.defaultIdentifierBody
    )

    static let sql = SyntaxGrammar(
        name: "SQL",
        extensions: ["sql"],
        caseSensitiveKeywords: false,
        lineComments: ["--"],
        lineCommentScope: .comment,
        blockComments: [
            BlockCommentRule(id: 1, open: "/*", close: "*/", scope: .comment, nestable: false),
        ],
        strings: [
            StringRule(id: 1, open: "'", close: "'", escape: nil, multiline: false, scope: .string),
            StringRule(id: 2, open: "\"", close: "\"", escape: nil, multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "select", "from", "where", "insert", "into", "values", "update", "set", "delete",
                "create", "drop", "alter", "table", "index", "view", "database", "schema", "join",
                "inner", "outer", "left", "right", "full", "cross", "on", "using", "group", "by",
                "having", "order", "asc", "desc", "limit", "offset", "distinct", "union", "all",
                "case", "when", "then", "else", "end", "as", "and", "or", "not", "in", "is", "null",
                "between", "like", "exists", "any", "some", "with", "recursive", "returning",
                "constraint", "primary", "foreign", "key", "references", "unique", "default",
                "check", "cascade", "restrict", "trigger", "function", "procedure", "begin", "commit",
                "rollback", "transaction", "grant", "revoke", "truncate", "if", "exists",
            ], scope: .keyword),
            KeywordGroup(words: [
                "int", "integer", "bigint", "smallint", "tinyint", "decimal", "numeric", "real",
                "float", "double", "char", "varchar", "text", "blob", "date", "time", "timestamp",
                "datetime", "boolean", "bool", "bit", "json", "jsonb", "uuid", "serial", "bigserial",
            ], scope: .type),
            KeywordGroup(words: ["true", "false", "null"], scope: .builtin),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: true,
        highlightAllCapsAsConstant: false,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: SyntaxGrammar.defaultIdentifierBody
    )

    static let dockerfile = SyntaxGrammar(
        name: "Dockerfile",
        extensions: ["dockerfile", "Dockerfile"],
        caseSensitiveKeywords: false,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 2, open: "'", close: "'", escape: nil, multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "from", "run", "cmd", "label", "maintainer", "expose", "env", "add", "copy",
                "entrypoint", "volume", "user", "workdir", "arg", "onbuild", "stopsignal",
                "healthcheck", "shell", "as",
            ], scope: .keyword),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: false,
        highlightAllCapsAsConstant: true,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: SyntaxGrammar.defaultIdentifierBody
    )

    static let makefile = SyntaxGrammar(
        name: "Makefile",
        extensions: ["mk", "makefile", "Makefile", "gnumakefile", "GNUmakefile"],
        caseSensitiveKeywords: true,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
            StringRule(id: 2, open: "'", close: "'", escape: nil, multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "ifeq", "ifneq", "ifdef", "ifndef", "else", "endif", "include", "define", "endef",
                "export", "unexport", "override", "vpath", ".PHONY", ".SUFFIXES", ".DEFAULT",
            ], scope: .keyword),
        ],
        supportsNumbers: false,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: false,
        highlightAllCapsAsConstant: true,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: {
            var set = SyntaxGrammar.defaultIdentifierBody
            set.insert("-")
            set.insert(".")
            return set
        }()
    )

    static let graphql = SyntaxGrammar(
        name: "GraphQL",
        extensions: ["graphql", "gql"],
        caseSensitiveKeywords: true,
        lineComments: ["#"],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"\"\"", close: "\"\"\"", escape: "\\", multiline: true, scope: .docComment),
            StringRule(id: 2, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "query", "mutation", "subscription", "fragment", "on", "type", "interface", "union",
                "enum", "input", "schema", "scalar", "directive", "extend", "implements", "repeatable",
            ], scope: .keyword),
            KeywordGroup(words: [
                "Int", "Float", "String", "Boolean", "ID",
            ], scope: .type),
            KeywordGroup(words: ["true", "false", "null"], scope: .builtin),
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

    static let terraform = SyntaxGrammar(
        name: "Terraform",
        extensions: ["tf", "tfvars", "hcl"],
        caseSensitiveKeywords: true,
        lineComments: ["#", "//"],
        lineCommentScope: .comment,
        blockComments: [
            BlockCommentRule(id: 1, open: "/*", close: "*/", scope: .comment, nestable: false),
        ],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
        ],
        keywordGroups: [
            KeywordGroup(words: [
                "resource", "data", "variable", "output", "module", "provider", "terraform",
                "locals", "for_each", "count", "depends_on", "lifecycle", "dynamic", "provisioner",
                "connection", "backend", "required_providers", "required_version",
            ], scope: .keyword),
            KeywordGroup(words: [
                "string", "number", "bool", "list", "map", "set", "object", "tuple", "any",
            ], scope: .type),
            KeywordGroup(words: ["true", "false", "null"], scope: .builtin),
        ],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: true,
        highlightAllCapsAsConstant: true,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: {
            var set = SyntaxGrammar.defaultIdentifierBody
            set.insert("-")
            return set
        }()
    )

    static let csv = SyntaxGrammar(
        name: "CSV",
        extensions: ["csv", "tsv"],
        caseSensitiveKeywords: true,
        lineComments: [],
        lineCommentScope: .comment,
        blockComments: [],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: "\"", multiline: true, scope: .string),
        ],
        keywordGroups: [],
        supportsNumbers: true,
        supportsHashDirectives: false,
        hashDirectiveScope: .preprocessor,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: false,
        highlightAllCapsAsConstant: false,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: SyntaxGrammar.defaultIdentifierBody
    )
}
