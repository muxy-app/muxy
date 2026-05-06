import AppKit

@MainActor
protocol LineNumberGutterHost: AnyObject {
    var viewportState: ViewportState? { get }
    var containerView: ViewportContainerView? { get }
    var scrollView: NSScrollView? { get }
    var textView: NSTextView? { get }
    var leadingGutterWidth: CGFloat { get set }
    var lineWrappingEnabled: Bool { get }

    func refreshViewport(force: Bool)
}

@MainActor
final class LineNumberGutterExtension: EditorExtension {
    let identifier = "line-number-gutter"

    private weak var host: LineNumberGutterHost?
    private var gutter: LineNumberGutterView?
    private var observers: [NSObjectProtocol] = []

    init(host: LineNumberGutterHost) {
        self.host = host
    }

    func didMount(context: EditorRenderContext) {
        installGutter(context: context)
    }

    func willUnmount(context _: EditorRenderContext) {
        removeGutter()
    }

    func renderViewport(context: EditorRenderContext, lineRange _: Range<Int>) {
        ensureInstalled(context: context)
        applyMetrics(context: context)
    }

    func applyIncremental(context: EditorRenderContext, lineRange _: Range<Int>, edit _: EditorTextEdit) {
        ensureInstalled(context: context)
        applyMetrics(context: context)
    }

    func textDidChange(context: EditorRenderContext) {
        applyMetrics(context: context)
    }

    func geometryDidChange(context: EditorRenderContext) {
        applyMetrics(context: context)
    }

    private func ensureInstalled(context: EditorRenderContext) {
        guard gutter == nil else { return }
        installGutter(context: context)
    }

    private func installGutter(context: EditorRenderContext) {
        guard gutter == nil,
              let host,
              let container = host.containerView,
              let scrollView = host.scrollView
        else { return }

        let view = LineNumberGutterView()
        view.wantsLayer = true
        view.autoresizingMask = []
        view.clipView = scrollView.contentView
        applyAppearance(to: view, context: context)
        view.frame = NSRect(
            x: scrollView.contentView.bounds.origin.x,
            y: 0,
            width: view.intrinsicWidth,
            height: container.frame.height
        )
        container.addSubview(view, positioned: .below, relativeTo: nil)
        gutter = view

        applyTextOffset(width: view.intrinsicWidth)
        observeScrollChanges(scrollView: scrollView)
        view.needsDisplay = true
    }

    private func removeGutter() {
        for observer in observers {
            NotificationCenter.default.removeObserver(observer)
        }
        observers.removeAll()
        gutter?.removeFromSuperview()
        gutter = nil
        applyTextOffset(width: 0)
    }

    private func applyMetrics(context: EditorRenderContext) {
        guard let view = gutter else { return }
        applyAppearance(to: view, context: context)
        layoutGutter()
        view.needsDisplay = true
    }

    private func applyAppearance(to view: LineNumberGutterView, context: EditorRenderContext) {
        let palette = EditorThemePalette.active
        view.font = monospacedFont(for: context.editorSettings)
        view.lineHeight = context.viewport.estimatedLineHeight
        view.topInset = context.textView.textContainerInset.height
        view.totalLines = context.backingStore.lineCount
        view.foregroundColor = palette.foreground.withAlphaComponent(0.45)
        view.backgroundColor = palette.background
        view.borderColor = palette.foreground.withAlphaComponent(0.08)
        view.wrappingEnabled = host?.lineWrappingEnabled ?? false
        view.heightMap = context.viewport.heightMap
    }

    private func layoutGutter() {
        guard let view = gutter,
              let container = host?.containerView,
              let scrollView = host?.scrollView
        else { return }
        let width = view.intrinsicWidth
        let frame = NSRect(
            x: scrollView.contentView.bounds.origin.x,
            y: 0,
            width: width,
            height: container.frame.height
        )
        if view.frame != frame {
            view.frame = frame
        }
        applyTextOffset(width: width)
    }

    private func applyTextOffset(width: CGFloat) {
        guard let host else { return }
        guard host.leadingGutterWidth != width else { return }
        host.leadingGutterWidth = width
        host.refreshViewport(force: true)
    }

    private func observeScrollChanges(scrollView: NSScrollView) {
        let center = NotificationCenter.default
        let bounds = center.addObserver(
            forName: NSView.boundsDidChangeNotification,
            object: scrollView.contentView,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.handleScrollOrResize() }
        }
        let frame = center.addObserver(
            forName: NSView.frameDidChangeNotification,
            object: scrollView.contentView,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.handleScrollOrResize() }
        }
        observers = [bounds, frame]
    }

    private func handleScrollOrResize() {
        layoutGutter()
        gutter?.needsDisplay = true
    }

    private func monospacedFont(for settings: EditorSettings) -> NSFont {
        let base = settings.resolvedFont
        let size = max(9, base.pointSize - 1)
        if base.isFixedPitch {
            return NSFont(name: base.fontName, size: size) ?? NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
        }
        return NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
    }
}

private final class LineNumberGutterView: NSView {
    var font: NSFont = .monospacedSystemFont(ofSize: 12, weight: .regular)
    var lineHeight: CGFloat = 16
    var topInset: CGFloat = 0
    var totalLines: Int = 1
    var foregroundColor: NSColor = .secondaryLabelColor
    var backgroundColor: NSColor = .clear
    var borderColor: NSColor = .separatorColor
    weak var clipView: NSClipView?

    var wrappingEnabled: Bool = false
    var heightMap: HeightMap?

    override var isFlipped: Bool { true }

    private let horizontalPadding: CGFloat = 8

    var intrinsicWidth: CGFloat {
        let digits = max(2, String(max(1, totalLines)).count)
        let sample = String(repeating: "0", count: digits)
        let width = (sample as NSString).size(withAttributes: [.font: font]).width
        return ceil(width + horizontalPadding * 2)
    }

    override func draw(_ dirtyRect: NSRect) {
        backgroundColor.setFill()
        dirtyRect.fill()

        let attributes: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: foregroundColor,
        ]

        if wrappingEnabled, heightMap != nil {
            drawWrapped(dirtyRect: dirtyRect, attributes: attributes)
        } else {
            drawUniform(dirtyRect: dirtyRect, attributes: attributes)
        }

        guard let clipView else { return }
        let visibleInGutter = clipView.convert(clipView.bounds, to: self)
        let borderTop = max(bounds.minY, visibleInGutter.minY)
        let borderBottom = min(bounds.maxY, visibleInGutter.maxY)
        guard borderBottom > borderTop else { return }

        borderColor.setStroke()
        let path = NSBezierPath()
        path.move(to: NSPoint(x: bounds.maxX - 0.5, y: borderTop))
        path.line(to: NSPoint(x: bounds.maxX - 0.5, y: borderBottom))
        path.lineWidth = 1
        path.stroke()
    }

    private func drawUniform(dirtyRect: NSRect, attributes: [NSAttributedString.Key: Any]) {
        guard lineHeight > 0, totalLines > 0 else { return }

        let firstLine = max(0, Int(floor((dirtyRect.minY - topInset) / lineHeight)))
        let lastLine = min(totalLines - 1, Int(ceil((dirtyRect.maxY - topInset) / lineHeight)))
        guard firstLine <= lastLine else { return }

        let availableWidth = bounds.width - horizontalPadding * 2

        for line in firstLine ... lastLine {
            let label = String(line + 1) as NSString
            let labelSize = label.size(withAttributes: attributes)
            let originX = horizontalPadding + max(0, availableWidth - labelSize.width)
            let originY = topInset + CGFloat(line) * lineHeight + (lineHeight - labelSize.height) / 2
            label.draw(at: NSPoint(x: originX, y: originY), withAttributes: attributes)
        }
    }

    private func drawWrapped(dirtyRect: NSRect, attributes: [NSAttributedString.Key: Any]) {
        guard let heightMap, totalLines > 0 else { return }

        let topY = max(0, dirtyRect.minY - topInset)
        let bottomY = max(topY, dirtyRect.maxY - topInset)
        let firstLocation = heightMap.lineAtY(topY)
        let lastLocation = heightMap.lineAtY(bottomY)
        let firstLine = max(0, min(firstLocation.line, totalLines - 1))
        let lastLine = max(firstLine, min(lastLocation.line, totalLines - 1))
        let availableWidth = bounds.width - horizontalPadding * 2

        for line in firstLine ... lastLine {
            let lineTop = topInset + heightMap.heightAbove(line: line)
            let lineHeightForLine = heightMap.heightOfLine(line)
            let label = String(line + 1) as NSString
            let labelSize = label.size(withAttributes: attributes)
            let originX = horizontalPadding + max(0, availableWidth - labelSize.width)
            let originY = lineTop + (lineHeightForLine - labelSize.height) / 2
            if originY + labelSize.height >= dirtyRect.minY, originY <= dirtyRect.maxY {
                label.draw(at: NSPoint(x: originX, y: originY), withAttributes: attributes)
            }
        }
    }
}
