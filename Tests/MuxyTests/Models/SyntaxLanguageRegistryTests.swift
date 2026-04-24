import Foundation
import Testing

@testable import Muxy

@Suite("SyntaxLanguageRegistry")
struct SyntaxLanguageRegistryTests {
    @Test("recognizes common extensions")
    func commonExtensions() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "foo.swift")?.name == "Swift")
        #expect(SyntaxLanguageRegistry.grammar(forFile: "foo.py")?.name == "Python")
        #expect(SyntaxLanguageRegistry.grammar(forFile: "foo.ts")?.name == "TypeScript")
        #expect(SyntaxLanguageRegistry.grammar(forFile: "foo.rs")?.name == "Rust")
        #expect(SyntaxLanguageRegistry.grammar(forFile: "foo.json")?.name == "JSON")
    }

    @Test("case insensitive on extension")
    func caseInsensitive() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "FOO.SWIFT")?.name == "Swift")
    }

    @Test("recognizes Dockerfile by filename")
    func dockerfileByName() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "Dockerfile")?.name == "Dockerfile")
    }

    @Test("recognizes Makefile by filename")
    func makefileByName() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "Makefile")?.name == "Makefile")
    }

    @Test("unknown extension returns nil")
    func unknownExtension() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "foo.unknownext") == nil)
    }

    @Test("path with directories still resolves")
    func pathWithDirectories() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "/a/b/c/foo.go")?.name == "Go")
    }

    @Test("recognizes JSX and TSX via JavaScript/TypeScript")
    func jsxTsxRecognized() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "App.jsx")?.name == "JavaScript")
        #expect(SyntaxLanguageRegistry.grammar(forFile: "App.tsx")?.name == "TypeScript")
    }

    @Test("recognizes Vue and Svelte")
    func vueSvelte() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "App.vue")?.name == "Vue")
        #expect(SyntaxLanguageRegistry.grammar(forFile: "App.svelte")?.name == "Svelte")
    }

    @Test("recognizes GraphQL")
    func graphql() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "schema.graphql")?.name == "GraphQL")
        #expect(SyntaxLanguageRegistry.grammar(forFile: "query.gql")?.name == "GraphQL")
    }

    @Test("recognizes Terraform")
    func terraform() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "main.tf")?.name == "Terraform")
        #expect(SyntaxLanguageRegistry.grammar(forFile: "prod.tfvars")?.name == "Terraform")
    }

    @Test("recognizes CSV")
    func csv() {
        #expect(SyntaxLanguageRegistry.grammar(forFile: "data.csv")?.name == "CSV")
        #expect(SyntaxLanguageRegistry.grammar(forFile: "data.tsv")?.name == "CSV")
    }
}
