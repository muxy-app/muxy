import AppKit

@MainActor
protocol EditorExtension: AnyObject {
    var identifier: String { get }

    func didMount(context: EditorRenderContext)

    func willUnmount(context: EditorRenderContext)

    func renderViewport(context: EditorRenderContext, lineRange: Range<Int>)

    func applyIncremental(context: EditorRenderContext, lineRange: Range<Int>, edit: EditorTextEdit)

    func textDidChange(context: EditorRenderContext)

    func selectionDidChange(context: EditorRenderContext)

    func geometryDidChange(context: EditorRenderContext)
}

extension EditorExtension {
    func didMount(context _: EditorRenderContext) {}
    func willUnmount(context _: EditorRenderContext) {}
    func renderViewport(context _: EditorRenderContext, lineRange _: Range<Int>) {}
    func applyIncremental(context _: EditorRenderContext, lineRange _: Range<Int>, edit _: EditorTextEdit) {}
    func textDidChange(context _: EditorRenderContext) {}
    func selectionDidChange(context _: EditorRenderContext) {}
    func geometryDidChange(context _: EditorRenderContext) {}
}
