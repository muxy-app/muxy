import AppKit
import Foundation

@MainActor
enum BrowserAnnotationSender {
    static func send(
        annotation: BrowserAnnotation,
        from state: BrowserTabState,
        markSent: () -> Void
    ) {
        let markdown = renderMarkdown(annotation: annotation)
        TranscriptInserter.insert(
            text: markdown,
            into: NSApp.keyWindow?.firstResponder,
            appendReturn: false
        )
        markSent()
    }

    static func renderMarkdown(annotation: BrowserAnnotation) -> String {
        var lines: [String] = []
        lines.append("@muxy-browser: \(annotation.pageURL)")
        if !annotation.pageTitle.isEmpty {
            lines.append("- page: \(annotation.pageTitle)")
        }
        lines.append("- selector: `\(annotation.selector)`")
        if !annotation.xpath.isEmpty {
            lines.append("- xpath: `\(annotation.xpath)`")
        }
        if !annotation.textSnippet.isEmpty {
            lines.append("- text: \"\(annotation.textSnippet)\"")
        }
        let bbox = String(
            format: "(x=%.0f, y=%.0f, w=%.0f, h=%.0f)",
            annotation.rect.origin.x,
            annotation.rect.origin.y,
            annotation.rect.width,
            annotation.rect.height
        )
        lines.append("- bbox: \(bbox)")
        let viewport = String(format: "%.0f×%.0f", annotation.viewportWidth, annotation.viewportHeight)
        lines.append("- viewport: \(viewport)")
        for override in annotation.styleOverrides {
            let original = override.originalValue.isEmpty ? "default" : override.originalValue
            lines.append("- style override: \(override.property.cssName): \(original) → \(override.value)")
        }
        let comment = annotation.comment.trimmingCharacters(in: .whitespacesAndNewlines)
        if !comment.isEmpty {
            lines.append("- comment: \"\(comment)\"")
        }
        return lines.joined(separator: "\n")
    }
}
