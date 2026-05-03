import AppKit

@MainActor
protocol CurrentLineHighlightHost: AnyObject {
    var viewportState: ViewportState? { get }
    var containerView: ViewportContainerView? { get }
    var scrollView: NSScrollView? { get }
    var textView: NSTextView? { get }
    var lineWrappingEnabled: Bool { get }
}

@MainActor
final class CurrentLineHighlightExtension: EditorExtension {
    let identifier = "current-line-highlight"

    private weak var host: CurrentLineHighlightHost?
    private var highlightView: CurrentLineHighlightView?
    private var lastAppliedFrame: NSRect = .zero
    private var lastAppliedHidden = true
    private var lastAppliedColor: CGColor?

    init(host: CurrentLineHighlightHost) {
        self.host = host
    }

    func didMount(context: EditorRenderContext) {
        installHighlight()
        refreshHighlight(context: context)
    }

    func willUnmount(context _: EditorRenderContext) {
        removeHighlight()
    }

    func renderViewport(context: EditorRenderContext, lineRange _: Range<Int>) {
        ensureInstalled()
        refreshHighlight(context: context)
    }

    func applyIncremental(context: EditorRenderContext, lineRange _: Range<Int>, edit _: EditorTextEdit) {
        ensureInstalled()
        repositionHighlight(context: context)
    }

    func selectionDidChange(context: EditorRenderContext) {
        repositionHighlight(context: context)
    }

    private func ensureInstalled() {
        guard highlightView == nil else { return }
        installHighlight()
    }

    private func installHighlight() {
        guard highlightView == nil,
              let host,
              let container = host.containerView
        else { return }

        let view = CurrentLineHighlightView()
        view.wantsLayer = true
        view.autoresizingMask = []
        view.frame = .zero
        view.isHidden = true
        container.addSubview(view, positioned: .above, relativeTo: nil)
        highlightView = view
        lastAppliedFrame = .zero
        lastAppliedHidden = true
        lastAppliedColor = nil
    }

    private func removeHighlight() {
        highlightView?.removeFromSuperview()
        highlightView = nil
        lastAppliedColor = nil
    }

    private func refreshHighlight(context: EditorRenderContext) {
        guard let view = highlightView else { return }
        applyAppearance(to: view)
        repositionHighlight(context: context)
    }

    private func repositionHighlight(context: EditorRenderContext) {
        guard let view = highlightView,
              let host,
              let container = host.containerView,
              let viewport = host.viewportState
        else { return }

        let backingLine = max(0, context.state.cursorLine - 1)
        guard let localLine = viewport.viewportLine(forBackingStoreLine: backingLine) else {
            if !lastAppliedHidden {
                view.isHidden = true
                lastAppliedHidden = true
            }
            return
        }

        let frame = computeHighlightFrame(
            context: context,
            container: container,
            viewport: viewport,
            localLine: localLine
        )

        if frame != lastAppliedFrame {
            view.frame = frame
            lastAppliedFrame = frame
        }
        if lastAppliedHidden {
            view.isHidden = false
            lastAppliedHidden = false
        }
    }

    private func computeHighlightFrame(
        context: EditorRenderContext,
        container: ViewportContainerView,
        viewport: ViewportState,
        localLine: Int
    ) -> NSRect {
        let topInset = context.textView.textContainerInset.height
        let yOffset = viewport.viewportYOffset()
        let lineHeight = viewport.estimatedLineHeight
        let width = container.frame.width

        guard host?.lineWrappingEnabled == true,
              let layoutManager = context.textView.layoutManager,
              let textContainer = context.textView.textContainer,
              let storage = context.textView.textStorage,
              storage.length > 0
        else {
            let originY = yOffset + topInset + CGFloat(localLine) * lineHeight
            return NSRect(x: 0, y: originY, width: width, height: lineHeight)
        }

        let nsString = storage.string as NSString
        var location = 0
        var currentLine = 0
        while currentLine < localLine, location < nsString.length {
            let lineRange = nsString.lineRange(for: NSRange(location: location, length: 0))
            location = NSMaxRange(lineRange)
            currentLine += 1
        }
        guard location <= nsString.length else {
            let originY = yOffset + topInset + CGFloat(localLine) * lineHeight
            return NSRect(x: 0, y: originY, width: width, height: lineHeight)
        }

        let lineRange = nsString.lineRange(for: NSRange(location: location, length: 0))
        let glyphRange = layoutManager.glyphRange(forCharacterRange: lineRange, actualCharacterRange: nil)
        guard glyphRange.length > 0 else {
            let originY = yOffset + topInset + CGFloat(localLine) * lineHeight
            return NSRect(x: 0, y: originY, width: width, height: lineHeight)
        }

        let bounding = layoutManager.boundingRect(forGlyphRange: glyphRange, in: textContainer)
        let originY = yOffset + topInset + bounding.minY
        return NSRect(x: 0, y: originY, width: width, height: max(lineHeight, bounding.height))
    }

    private func applyAppearance(to view: CurrentLineHighlightView) {
        let palette = EditorThemePalette.active
        let color = palette.foreground.withAlphaComponent(0.08).cgColor
        if let lastAppliedColor, CFEqual(lastAppliedColor, color) { return }
        view.layer?.backgroundColor = color
        lastAppliedColor = color
    }
}

private final class CurrentLineHighlightView: NSView {
    override var isFlipped: Bool { true }

    override func hitTest(_: NSPoint) -> NSView? {
        nil
    }
}
