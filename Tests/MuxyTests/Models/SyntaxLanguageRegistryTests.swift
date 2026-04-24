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
}
