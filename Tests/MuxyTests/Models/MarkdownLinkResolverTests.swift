import Foundation
import Testing

@testable import Muxy

@Suite("MarkdownLinkResolver")
struct MarkdownLinkResolverTests {
    private let projectPath = "/Users/example/project"
    private let currentFilePath = "/Users/example/project/docs/README.md"

    @Test("external browser schemes resolve as external URLs")
    func externalURLs() throws {
        #expect(MarkdownLinkResolver.resolve(
            href: "https://example.com/docs",
            currentFilePath: currentFilePath,
            projectPath: projectPath
        ) == .external(try #require(URL(string: "https://example.com/docs"))))

        #expect(MarkdownLinkResolver.resolve(
            href: "mailto:support@example.com",
            currentFilePath: currentFilePath,
            projectPath: projectPath
        ) == .external(try #require(URL(string: "mailto:support@example.com"))))
    }

    @Test("relative markdown links resolve from the current markdown directory")
    func relativeLinks() {
        let resolution = MarkdownLinkResolver.resolve(
            href: "features/editor.md#Markdown%20preview",
            currentFilePath: currentFilePath,
            projectPath: projectPath
        )

        #expect(resolution == .internalFile(
            path: "/Users/example/project/docs/features/editor.md",
            fragment: "Markdown preview"
        ))
    }

    @Test("parent relative links can target files inside the project")
    func parentRelativeLinks() {
        let resolution = MarkdownLinkResolver.resolve(
            href: "../CONTRIBUTING.md",
            currentFilePath: currentFilePath,
            projectPath: projectPath
        )

        #expect(resolution == .internalFile(path: "/Users/example/project/CONTRIBUTING.md", fragment: nil))
    }

    @Test("root relative links resolve from the project root")
    func rootRelativeLinks() {
        let resolution = MarkdownLinkResolver.resolve(
            href: "/docs/features/editor.md",
            currentFilePath: currentFilePath,
            projectPath: projectPath
        )

        #expect(resolution == .internalFile(path: "/Users/example/project/docs/features/editor.md", fragment: nil))
    }

    @Test("same document fragments stay in the current preview")
    func sameDocumentFragment() {
        let resolution = MarkdownLinkResolver.resolve(
            href: "#markdown-preview",
            currentFilePath: currentFilePath,
            projectPath: projectPath
        )

        #expect(resolution == .sameDocumentFragment("markdown-preview"))
    }

    @Test("file URLs inside the project resolve as internal files")
    func fileURLInsideProject() {
        let resolution = MarkdownLinkResolver.resolve(
            href: "file:///Users/example/project/docs/Guide.md#Install",
            currentFilePath: currentFilePath,
            projectPath: projectPath
        )

        #expect(resolution == .internalFile(path: "/Users/example/project/docs/Guide.md", fragment: "Install"))
    }

    @Test("links escaping the project are unsupported")
    func rejectsEscapes() {
        let resolution = MarkdownLinkResolver.resolve(
            href: "../../outside.md",
            currentFilePath: currentFilePath,
            projectPath: projectPath
        )

        #expect(resolution == .unsupported)
    }

    @Test("file URLs outside the project are unsupported")
    func rejectsOutsideFileURLs() {
        let resolution = MarkdownLinkResolver.resolve(
            href: "file:///Users/example/.ssh/config",
            currentFilePath: currentFilePath,
            projectPath: projectPath
        )

        #expect(resolution == .unsupported)
    }
}
