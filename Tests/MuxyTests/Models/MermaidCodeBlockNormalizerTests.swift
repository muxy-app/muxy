import AppKit
import Testing

@testable import Muxy

@Suite("Mermaid code block normalization")
struct MermaidCodeBlockNormalizerTests {
    @Test("normalizeLabelNewlines converts real newlines inside bracket labels")
    func normalizeLabelNewlinesConvertsRealNewlinesInsideLabels() {
        let unixInput = "graph TD\nA[Chargeback DB\n(Kafka/Flink write)] --> B\n"
        let unixOutput = MermaidCodeBlockNormalizer.normalizeLabelNewlines(in: unixInput)
        #expect(unixOutput == "graph TD\nA[Chargeback DB<br/>(Kafka/Flink write)] --> B\n")

        let windowsInput = "graph TD\r\nA[Chargeback DB\r\n(Kafka/Flink write)] --> B\r\n"
        let windowsOutput = MermaidCodeBlockNormalizer.normalizeLabelNewlines(in: windowsInput)
        #expect(windowsOutput == "graph TD\r\nA[Chargeback DB<br/>(Kafka/Flink write)] --> B\r\n")
    }

    @Test("normalizeLabelNewlines preserves newlines outside bracket labels")
    func normalizeLabelNewlinesPreservesOutsideLabelNewlines() {
        let input = "graph TD\nA[Label] --> B\nB --> C\n"
        let output = MermaidCodeBlockNormalizer.normalizeLabelNewlines(in: input)

        #expect(output == input)
    }

    @Test("normalizeLabelNewlines converts literal \\n only inside bracket labels")
    func normalizeLabelNewlinesConvertsOnlyInLabels() {
        let input = "graph TD\nA[Line1\\nLine2] --> B\nB --> C\\nD\n"
        let output = MermaidCodeBlockNormalizer.normalizeLabelNewlines(in: input)

        #expect(output == "graph TD\nA[Line1<br/>Line2] --> B\nB --> C\\nD\n")
    }

    @Test("normalizeLabelNewlines handles nested bracket text conservatively")
    func normalizeLabelNewlinesNestedBrackets() {
        let input = "flowchart LR\nA[Outer [Inner\\nLabel] text\\nmore] --> B\n"
        let output = MermaidCodeBlockNormalizer.normalizeLabelNewlines(in: input)

        #expect(output == "flowchart LR\nA[Outer [Inner<br/>Label] text<br/>more] --> B\n")
    }

    @Test("normalizeMermaidCodeBlocks only rewrites mermaid fenced blocks")
    func normalizeMermaidCodeBlocksScope() {
        let markdown = """
        Before

        ```swift
        let text = "[A\\nB]"
        ```

        ```mermaid
        graph TD
        A[Hello\\nWorld] --> B
        B --> C\\nD
        ```

        After
        """

        let output = MermaidCodeBlockNormalizer.normalizeMermaidCodeBlocks(in: markdown)

        #expect(output.contains("let text = \"[A\\nB]\""))
        #expect(output.contains("A[Hello<br/>World] --> B"))
        #expect(output.contains("B --> C\\nD"))
    }

    @Test("MarkdownRenderer shell loads Mermaid.js renderer asset")
    @MainActor
    func markdownRendererShellLoadsRendererAsset() {
        let html = MarkdownRenderer.html(filePath: nil)

        #expect(html.contains(".mermaid"))
        #expect(html.contains("muxy-asset://markdown/markdown-renderer.js"))
    }

    @Test("MarkdownRenderer shell exposes anchor block class and image base host hook")
    @MainActor
    func markdownRendererShellExposesAnchorAndImageHooks() {
        let html = MarkdownRenderer.html(filePath: nil)

        #expect(html.contains(".muxy-anchor-block"))
        #expect(html.contains("__muxyImageBaseHost"))
    }

    @Test("MarkdownRenderer themeApplyScript emits Mermaid theme variables")
    @MainActor
    func themeApplyScriptEmitsMermaidThemeVariables() {
        let script = MarkdownRenderer.themeApplyScript(
            palette: MarkdownRenderer.Palette(
                background: NSColor.black,
                foreground: NSColor.white,
                accent: NSColor.systemBlue
            )
        )

        #expect(script.contains("__muxyMermaidThemeVariables"))
        #expect(script.contains("__muxyMermaidBaseTheme = \"dark\""))
        #expect(script.contains("__muxyMermaidUseThemeVariables = true"))
    }

    @Test("MarkdownRenderer shell stays stable across palettes")
    @MainActor
    func shellStableAcrossPalettes() {
        let firstShell = MarkdownRenderer.html(filePath: "/tmp/readme.md")
        let secondShell = MarkdownRenderer.html(filePath: "/tmp/readme.md")

        #expect(firstShell == secondShell)
    }
}
