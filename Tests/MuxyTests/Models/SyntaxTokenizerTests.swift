import Foundation
import Testing

@testable import Muxy

@Suite("SyntaxTokenizer")
@MainActor
struct SyntaxTokenizerTests {
    private func tokenize(
        _ line: String,
        grammar: SyntaxGrammar,
        state: LineEndState = .normal
    ) -> (tokens: [TokenSpan], endState: LineEndState) {
        SyntaxTokenizer(grammar: grammar).tokenize(line: line, startState: state)
    }

    private func scopes(_ tokens: [TokenSpan]) -> [SyntaxScope] {
        tokens.map(\.scope)
    }

    @Test("Swift keywords are tagged")
    func swiftKeywords() {
        let result = tokenize("let x = 1", grammar: .swift)
        #expect(result.tokens.contains(where: { $0.scope == .keyword }))
        #expect(result.tokens.contains(where: { $0.scope == .number }))
    }

    @Test("Swift double-quoted string is a single string token")
    func swiftString() {
        let result = tokenize("\"hello\"", grammar: .swift)
        #expect(result.tokens.count == 1)
        #expect(result.tokens.first?.scope == .string)
        #expect(result.tokens.first?.location == 0)
        #expect(result.tokens.first?.length == 7)
        #expect(result.endState == .normal)
    }

    @Test("Swift line comment consumes rest of line")
    func swiftLineComment() {
        let result = tokenize("let x = 1 // trailing", grammar: .swift)
        #expect(result.tokens.last?.scope == .comment)
    }

    @Test("Swift block comment single-line")
    func swiftBlockCommentSingleLine() {
        let result = tokenize("/* hello */", grammar: .swift)
        #expect(result.tokens.count == 1)
        #expect(result.tokens.first?.scope == .comment)
        #expect(result.endState == .normal)
    }

    @Test("Swift block comment leaves inBlockComment state open across line")
    func swiftBlockCommentOpen() {
        let result = tokenize("/* open", grammar: .swift)
        #expect(result.tokens.last?.scope == .comment)
        if case let .inBlockComment(id, depth) = result.endState {
            #expect(depth == 1)
            #expect(id == 1)
        } else {
            Issue.record("expected inBlockComment state, got \(result.endState)")
        }
    }

    @Test("Continuation of Swift block comment closes on */")
    func swiftBlockCommentContinuation() {
        let result = tokenize(
            "still comment */ after",
            grammar: .swift,
            state: .inBlockComment(id: 1, depth: 1)
        )
        #expect(result.endState == .normal)
        #expect(result.tokens.first?.scope == .comment)
    }

    @Test("Rust nestable block comments track depth")
    func rustNestedBlockComments() {
        let open = tokenize("/* outer /* inner */ still", grammar: .rust)
        if case let .inBlockComment(_, depth) = open.endState {
            #expect(depth == 1)
        } else {
            Issue.record("expected inBlockComment, got \(open.endState)")
        }
    }

    @Test("Python triple-quoted string spans multiple lines")
    func pythonTripleQuote() {
        let first = tokenize("\"\"\"doc start", grammar: .python)
        if case let .inString(id) = first.endState {
            #expect(id == 1)
        } else {
            Issue.record("expected inString, got \(first.endState)")
        }
        let second = tokenize("still doc\"\"\"", grammar: .python, state: first.endState)
        #expect(second.endState == .normal)
    }

    @Test("Python f-string is recognized")
    func pythonFString() {
        let result = tokenize("f\"x={x}\"", grammar: .python)
        #expect(result.tokens.first?.scope == .string)
    }

    @Test("Numbers: hex, binary, decimal with underscores")
    func numberForms() {
        let hex = tokenize("0xFF_FF", grammar: .swift)
        #expect(hex.tokens.first?.scope == .number)
        #expect(hex.tokens.first?.length == 7)

        let bin = tokenize("0b1010_1100", grammar: .swift)
        #expect(bin.tokens.first?.scope == .number)

        let decimal = tokenize("1_234.5e-3", grammar: .swift)
        #expect(decimal.tokens.first?.scope == .number)
    }

    @Test("All-caps identifier tagged as constant when enabled")
    func allCapsConstant() {
        let result = tokenize("FOO_BAR", grammar: .swift)
        #expect(result.tokens.first?.scope == .constant)
    }

    @Test("Function call heuristic tags identifier before (")
    func functionCallHeuristic() {
        let result = tokenize("foo()", grammar: .swift)
        #expect(result.tokens.contains(where: { $0.scope == .function }))
    }

    @Test("@attribute tagged with attribute scope")
    func atAttribute() {
        let result = tokenize("@MainActor", grammar: .swift)
        #expect(result.tokens.first?.scope == .attribute)
    }

    @Test("#include tagged with preprocessor scope in C")
    func hashDirective() {
        let result = tokenize("#include <stdio.h>", grammar: .c)
        #expect(result.tokens.first?.scope == .preprocessor)
    }

    @Test("SQL keywords are case-insensitive")
    func sqlCaseInsensitive() {
        let upper = tokenize("SELECT * FROM t", grammar: .sql)
        let lower = tokenize("select * from t", grammar: .sql)
        let upperKw = upper.tokens.filter { $0.scope == .keyword }.count
        let lowerKw = lower.tokens.filter { $0.scope == .keyword }.count
        #expect(upperKw == lowerKw)
        #expect(upperKw >= 2)
    }

    @Test("JSON recognizes true/false/null as builtin")
    func jsonBuiltins() {
        let result = tokenize("true false null", grammar: .json)
        let builtins = result.tokens.filter { $0.scope == .builtin }
        #expect(builtins.count == 3)
    }

    @Test("Shell treats single-quoted strings with no escape")
    func shellSingleQuote() {
        let result = tokenize("echo 'it\\'s'", grammar: .shell)
        #expect(result.tokens.contains(where: { $0.scope == .string }))
    }

    @Test("Long line over threshold returns no tokens and preserves state")
    func longLineSkipped() {
        let line = String(repeating: "a", count: SyntaxHighlighter.longLineThreshold + 1)
        let highlighter = SyntaxHighlighter(grammar: .swift)
        let store = TextBackingStore()
        store.loadFromText(line)
        _ = highlighter.applyEdit(startLine: 0, oldLineCount: 0, newLineCount: 1, backingStore: store)
        #expect(highlighter.tokens(forLine: 0)?.isEmpty == true)
    }

    @Test("GraphQL tags keywords and types")
    func graphqlKeywordsAndTypes() {
        let result = tokenize("type Query { id: ID }", grammar: .graphql)
        #expect(result.tokens.contains(where: { $0.scope == .keyword }))
        #expect(result.tokens.contains(where: { $0.scope == .type }))
    }

    @Test("Terraform recognizes resource keyword and string")
    func terraformBasic() {
        let result = tokenize(
            "resource \"aws_instance\" \"web\" {",
            grammar: .terraform
        )
        #expect(result.tokens.contains(where: { $0.scope == .keyword }))
        #expect(result.tokens.contains(where: { $0.scope == .string }))
    }

    @Test("CSV quoted field is a string, plain fields not keywords")
    func csvQuoted() {
        let result = tokenize("a,b,\"c,d\"", grammar: .csv)
        #expect(result.tokens.contains(where: { $0.scope == .string }))
        #expect(!result.tokens.contains(where: { $0.scope == .keyword }))
    }

    @Test("Vue and Svelte tokenize attribute strings")
    func vueSvelteAttributeStrings() {
        let vueResult = tokenize("<div class=\"foo\">", grammar: .vue)
        #expect(vueResult.tokens.contains(where: { $0.scope == .attributeValue }))
        let svelteResult = tokenize("<div class=\"bar\">", grammar: .svelte)
        #expect(svelteResult.tokens.contains(where: { $0.scope == .attributeValue }))
    }

    @Test("Vue and Svelte recognize block comments")
    func vueSvelteComments() {
        let vueResult = tokenize("<!-- comment -->", grammar: .vue)
        #expect(vueResult.tokens.contains(where: { $0.scope == .comment }))
        let svelteResult = tokenize("<!-- comment -->", grammar: .svelte)
        #expect(svelteResult.tokens.contains(where: { $0.scope == .comment }))
    }

    @Test("TSX: component name in opening tag is tagged")
    func tsxOpeningTag() {
        let result = tokenize("<Dialog open={open}>", grammar: .typescript)
        let dialog = result.tokens.first { span in
            let s = (("<Dialog open={open}>" as NSString).substring(with: NSRange(location: span.location, length: span.length)))
            return s == "Dialog"
        }
        #expect(dialog?.scope == .tag)
    }

    @Test("TSX: component name in closing tag is tagged")
    func tsxClosingTag() {
        let line = "</DialogContent>"
        let result = tokenize(line, grammar: .typescript)
        let dc = result.tokens.first { span in
            let s = (line as NSString).substring(with: NSRange(location: span.location, length: span.length))
            return s == "DialogContent"
        }
        #expect(dc?.scope == .tag)
    }

    @Test("TSX: lowercase HTML tag is tagged")
    func tsxLowercaseTag() {
        let line = "<form onSubmit={x}>"
        let result = tokenize(line, grammar: .typescript)
        let form = result.tokens.first { span in
            let s = (line as NSString).substring(with: NSRange(location: span.location, length: span.length))
            return s == "form"
        }
        #expect(form?.scope == .tag)
    }

    @Test("TSX: PascalCase identifier outside JSX is tagged as type")
    func tsxPascalCaseAsType() {
        let line = "const x: MyType = new MyClass()"
        let result = tokenize(line, grammar: .typescript)
        let myType = result.tokens.first { span in
            let s = (line as NSString).substring(with: NSRange(location: span.location, length: span.length))
            return s == "MyType"
        }
        #expect(myType?.scope == .type)
    }

    @Test("TSX: ALL_CAPS still wins over PascalCase rule")
    func tsxAllCapsPriority() {
        let line = "const X = FOO"
        let result = tokenize(line, grammar: .typescript)
        let foo = result.tokens.first { span in
            let s = (line as NSString).substring(with: NSRange(location: span.location, length: span.length))
            return s == "FOO"
        }
        #expect(foo?.scope == .constant)
    }

    @Test("TSX: less-than comparison not mistaken for tag")
    func tsxComparisonNotTag() {
        let line = "if (a < B) {}"
        let result = tokenize(line, grammar: .typescript)
        let b = result.tokens.first { span in
            let s = (line as NSString).substring(with: NSRange(location: span.location, length: span.length))
            return s == "B"
        }
        #expect(b?.scope != .tag)
    }

    @Test("Escape sequence inside string does not end it early")
    func escapeInString() {
        let result = tokenize("\"a\\\"b\"", grammar: .swift)
        #expect(result.tokens.count == 1)
        #expect(result.tokens.first?.scope == .string)
        #expect(result.endState == .normal)
    }
}
