import Foundation

extension SyntaxGrammar {
    static let html: SyntaxGrammar = {
        var identifierStart = SyntaxGrammar.defaultIdentifierStart
        var identifierBody = SyntaxGrammar.defaultIdentifierBody
        identifierStart.insert("<")
        identifierStart.insert("/")
        identifierBody.insert("-")
        return SyntaxGrammar(
            name: "HTML",
            extensions: ["html", "htm", "xhtml"],
            caseSensitiveKeywords: false,
            lineComments: [],
            lineCommentScope: .comment,
            blockComments: [
                BlockCommentRule(id: 1, open: "<!--", close: "-->", scope: .comment, nestable: false),
            ],
            strings: [
                StringRule(id: 1, open: "\"", close: "\"", escape: nil, multiline: false, scope: .attributeValue),
                StringRule(id: 2, open: "'", close: "'", escape: nil, multiline: false, scope: .attributeValue),
            ],
            keywordGroups: [
                KeywordGroup(words: [
                    "html", "head", "body", "title", "meta", "link", "script", "style", "div", "span",
                    "a", "p", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "table", "tr",
                    "td", "th", "thead", "tbody", "tfoot", "form", "input", "textarea", "button",
                    "select", "option", "img", "video", "audio", "source", "canvas", "svg", "path",
                    "header", "footer", "nav", "section", "article", "aside", "main", "figure",
                    "figcaption", "details", "summary", "dialog", "iframe", "br", "hr", "code",
                    "pre", "em", "strong", "b", "i", "u", "small", "mark", "label", "fieldset",
                    "legend", "datalist", "picture",
                ], scope: .tag),
            ],
            supportsNumbers: false,
            supportsHashDirectives: false,
            hashDirectiveScope: .preprocessor,
            supportsAtAttributes: false,
            atAttributeScope: .attribute,
            highlightFunctionCalls: false,
            highlightAllCapsAsConstant: false,
            identifierStart: identifierStart,
            identifierBody: identifierBody
        )
    }()

    static let xml = SyntaxGrammar(
        name: "XML",
        extensions: ["xml", "xsd", "xsl", "xslt", "plist", "pbxproj", "storyboard", "xib", "svg"],
        caseSensitiveKeywords: false,
        lineComments: [],
        lineCommentScope: .comment,
        blockComments: [
            BlockCommentRule(id: 1, open: "<!--", close: "-->", scope: .comment, nestable: false),
            BlockCommentRule(id: 2, open: "<?", close: "?>", scope: .preprocessor, nestable: false),
            BlockCommentRule(id: 3, open: "<![CDATA[", close: "]]>", scope: .string, nestable: false),
        ],
        strings: [
            StringRule(id: 1, open: "\"", close: "\"", escape: nil, multiline: false, scope: .attributeValue),
            StringRule(id: 2, open: "'", close: "'", escape: nil, multiline: false, scope: .attributeValue),
        ],
        keywordGroups: [],
        supportsNumbers: false,
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
            set.insert(":")
            return set
        }()
    )

    static let css: SyntaxGrammar = {
        var identifierBody = SyntaxGrammar.defaultIdentifierBody
        identifierBody.insert("-")
        return SyntaxGrammar(
            name: "CSS",
            extensions: ["css", "scss", "sass", "less"],
            caseSensitiveKeywords: false,
            lineComments: ["//"],
            lineCommentScope: .comment,
            blockComments: [
                BlockCommentRule(id: 1, open: "/*", close: "*/", scope: .comment, nestable: false),
            ],
            strings: [
                StringRule(id: 1, open: "\"", close: "\"", escape: "\\", multiline: false, scope: .string),
                StringRule(id: 2, open: "'", close: "'", escape: "\\", multiline: false, scope: .string),
            ],
            keywordGroups: [
                KeywordGroup(words: [
                    "important", "inherit", "initial", "unset", "revert", "auto", "none", "normal",
                    "bold", "italic", "underline", "block", "inline", "flex", "grid", "absolute",
                    "relative", "fixed", "static", "sticky", "hidden", "visible", "pointer",
                    "transparent", "center", "left", "right", "top", "bottom", "middle",
                ], scope: .builtin),
            ],
            supportsNumbers: true,
            supportsHashDirectives: false,
            hashDirectiveScope: .preprocessor,
            supportsAtAttributes: true,
            atAttributeScope: .keyword,
            highlightFunctionCalls: true,
            highlightAllCapsAsConstant: false,
            identifierStart: SyntaxGrammar.defaultIdentifierStart,
            identifierBody: identifierBody
        )
    }()

    static let markdown = SyntaxGrammar(
        name: "Markdown",
        extensions: ["md", "markdown", "mdx", "mdown"],
        caseSensitiveKeywords: false,
        lineComments: [],
        lineCommentScope: .comment,
        blockComments: [
            BlockCommentRule(id: 1, open: "```", close: "```", scope: .string, nestable: false),
            BlockCommentRule(id: 2, open: "~~~", close: "~~~", scope: .string, nestable: false),
            BlockCommentRule(id: 3, open: "<!--", close: "-->", scope: .comment, nestable: false),
        ],
        strings: [
            StringRule(id: 1, open: "`", close: "`", escape: nil, multiline: false, scope: .string),
        ],
        keywordGroups: [],
        supportsNumbers: false,
        supportsHashDirectives: false,
        hashDirectiveScope: .heading,
        supportsAtAttributes: false,
        atAttributeScope: .attribute,
        highlightFunctionCalls: false,
        highlightAllCapsAsConstant: false,
        identifierStart: SyntaxGrammar.defaultIdentifierStart,
        identifierBody: SyntaxGrammar.defaultIdentifierBody
    )

    private static func makeHTMLLike(
        name: String,
        extensions: [String],
        extraTags: [String]
    ) -> SyntaxGrammar {
        var identifierStart = SyntaxGrammar.defaultIdentifierStart
        var identifierBody = SyntaxGrammar.defaultIdentifierBody
        identifierStart.insert("<")
        identifierStart.insert("/")
        identifierBody.insert("-")
        let baseTags = [
            "html", "head", "body", "title", "meta", "link", "script", "style", "div", "span",
            "a", "p", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "table", "tr",
            "td", "th", "thead", "tbody", "tfoot", "form", "input", "textarea", "button",
            "select", "option", "img", "video", "audio", "source", "canvas", "svg", "path",
            "header", "footer", "nav", "section", "article", "aside", "main", "figure",
            "figcaption", "details", "summary", "dialog", "iframe", "br", "hr", "code",
            "pre", "em", "strong", "b", "i", "u", "small", "mark", "label", "fieldset",
            "legend", "datalist", "picture", "template", "slot",
        ]
        return SyntaxGrammar(
            name: name,
            extensions: extensions,
            caseSensitiveKeywords: false,
            lineComments: [],
            lineCommentScope: .comment,
            blockComments: [
                BlockCommentRule(id: 1, open: "<!--", close: "-->", scope: .comment, nestable: false),
            ],
            strings: [
                StringRule(id: 1, open: "\"", close: "\"", escape: nil, multiline: false, scope: .attributeValue),
                StringRule(id: 2, open: "'", close: "'", escape: nil, multiline: false, scope: .attributeValue),
            ],
            keywordGroups: [
                KeywordGroup(words: Set(baseTags + extraTags), scope: .tag),
            ],
            supportsNumbers: false,
            supportsHashDirectives: false,
            hashDirectiveScope: .preprocessor,
            supportsAtAttributes: false,
            atAttributeScope: .attribute,
            highlightFunctionCalls: false,
            highlightAllCapsAsConstant: false,
            identifierStart: identifierStart,
            identifierBody: identifierBody
        )
    }

    static let vue = makeHTMLLike(
        name: "Vue",
        extensions: ["vue"],
        extraTags: ["transition", "transition-group", "keep-alive", "teleport", "suspense", "component"]
    )

    static let svelte = makeHTMLLike(
        name: "Svelte",
        extensions: ["svelte"],
        extraTags: [
            "svelte:self", "svelte:component", "svelte:element", "svelte:window",
            "svelte:document", "svelte:body", "svelte:head", "svelte:options", "svelte:fragment",
        ]
    )
}
