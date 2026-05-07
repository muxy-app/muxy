import AppKit

@MainActor
protocol LineNumberGutterHost: AnyObject {
    var viewportState: ViewportState? { get }
    var scrollView: NSScrollView? { get }
    var scrollContainer: EditorScrollContainer? { get }
    var textView: NSTextView? { get }
    var lineWrappingEnabled: Bool { get }
}

@MainActor
final class LineNumberGutterExtension: EditorExtension {
    let identifier = "line-number-gutter"

    private weak var host: LineNumberGutterHost?
    private var gutterView: LineNumberGutterView?
    private var scrollObserver: NSObjectProtocol?

    init(host: LineNumberGutterHost) {
        self.host = host
    }

    func didMount(context: EditorRenderContext) {
        install(context: context)
    }

    func willUnmount(context _: EditorRenderContext) {
        remove()
    }

    func renderViewport(context: EditorRenderContext, lineRange _: Range<Int>) {
        ensureInstalled(context: context)
        update(context: context)
    }

    func applyIncremental(context: EditorRenderContext, lineRange _: Range<Int>, edit _: EditorTextEdit) {
        ensureInstalled(context: context)
        update(context: context)
    }

    func textDidChange(context: EditorRenderContext) {
        update(context: context)
    }

    func geometryDidChange(context: EditorRenderContext) {
        update(context: context)
    }

    private func ensureInstalled(context: EditorRenderContext) {
        guard gutterView == nil else { return }
        install(context: context)
    }

    private func install(context: EditorRenderContext) {
        guard gutterView == nil,
              let host,
              let container = host.scrollContainer,
              let scrollView = host.scrollView
        else { return }
        let view = LineNumberGutterView()
        view.scrollView = scrollView
        applyState(to: view, context: context)
        container.setGutter(view)
        gutterView = view
        scrollView.contentView.postsBoundsChangedNotifications = true
        scrollObserver = NotificationCenter.default.addObserver(
            forName: NSView.boundsDidChangeNotification,
            object: scrollView.contentView,
            queue: .main
        ) { [weak view] _ in
            MainActor.assumeIsolated { view?.needsDisplay = true }
        }
    }

    private func remove() {
        if let scrollObserver {
            NotificationCenter.default.removeObserver(scrollObserver)
            self.scrollObserver = nil
        }
        host?.scrollContainer?.setGutter(nil)
        gutterView = nil
    }

    private func update(context: EditorRenderContext) {
        guard let view = gutterView else { return }
        let oldWidth = view.preferredWidth
        applyState(to: view, context: context)
        let newWidth = view.preferredWidth
        if abs(newWidth - oldWidth) > 0.5 {
            host?.scrollContainer?.gutterWidthDidChange()
        }
        view.needsDisplay = true
    }

    private func applyState(to view: LineNumberGutterView, context: EditorRenderContext) {
        let palette = EditorThemePalette.active
        view.labelFont = gutterLabelFont(for: context.editorSettings)
        view.foregroundColor = palette.foreground.withAlphaComponent(0.45)
        view.fillColor = palette.background
        view.borderColor = palette.foreground.withAlphaComponent(0.08)
        view.lineHeight = max(1, context.viewport.estimatedLineHeight)
        view.topInset = context.textView.textContainerInset.height
        view.totalLines = max(1, context.backingStore.lineCount)
        view.heightMap = context.viewport.heightMap
        view.wrappingEnabled = host?.lineWrappingEnabled ?? false
    }

    private func gutterLabelFont(for settings: EditorSettings) -> NSFont {
        let base = settings.resolvedFont
        let size = max(9, base.pointSize - 1)
        if base.isFixedPitch {
            return NSFont(name: base.fontName, size: size) ?? NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
        }
        return NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
    }
}

@MainActor
final class LineNumberGutterView: NSView {
    weak var scrollView: NSScrollView?

    var labelFont: NSFont = .monospacedSystemFont(ofSize: 12, weight: .regular)
    var foregroundColor: NSColor = .secondaryLabelColor
    var fillColor: NSColor = .clear
    var borderColor: NSColor = .separatorColor
    var lineHeight: CGFloat = 16
    var topInset: CGFloat = 0
    var totalLines: Int = 1
    var heightMap: HeightMap?
    var wrappingEnabled: Bool = false

    private let horizontalPadding: CGFloat = 8
    private let minimumDigitCount: Int = 2

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.masksToBounds = true
        autoresizingMask = []
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var isFlipped: Bool { true }

    override func hitTest(_: NSPoint) -> NSView? {
        nil
    }

    var preferredWidth: CGFloat {
        let digits = max(minimumDigitCount, String(max(1, totalLines)).count)
        let sample = String(repeating: "0", count: digits)
        let width = (sample as NSString).size(withAttributes: [.font: labelFont]).width
        return ceil(width + horizontalPadding * 2)
    }

    override func draw(_: NSRect) {
        guard let scrollView else { return }
        let scrollY = scrollView.contentView.bounds.origin.y

        fillColor.setFill()
        bounds.fill()

        let attributes: [NSAttributedString.Key: Any] = [
            .font: labelFont,
            .foregroundColor: foregroundColor,
        ]

        if wrappingEnabled, heightMap != nil {
            drawWrapped(scrollY: scrollY, attributes: attributes)
        } else {
            drawUniform(scrollY: scrollY, attributes: attributes)
        }

        drawTrailingBorder()
    }

    private func drawUniform(scrollY: CGFloat, attributes: [NSAttributedString.Key: Any]) {
        guard lineHeight > 0, totalLines > 0 else { return }
        let topDocY = scrollY
        let bottomDocY = scrollY + bounds.height
        let firstLine = max(0, Int(floor((topDocY - topInset) / lineHeight)))
        let lastLine = min(totalLines - 1, Int(ceil((bottomDocY - topInset) / lineHeight)))
        guard firstLine <= lastLine else { return }

        let availableWidth = bounds.width - horizontalPadding * 2

        for line in firstLine ... lastLine {
            let docTop = topInset + CGFloat(line) * lineHeight
            let gutterY = docTop - scrollY
            let label = String(line + 1) as NSString
            let labelSize = label.size(withAttributes: attributes)
            let originX = horizontalPadding + max(0, availableWidth - labelSize.width)
            let originY = gutterY + (lineHeight - labelSize.height) / 2
            guard originY + labelSize.height >= 0, originY <= bounds.height else { continue }
            label.draw(at: NSPoint(x: originX, y: originY), withAttributes: attributes)
        }
    }

    private func drawWrapped(scrollY: CGFloat, attributes: [NSAttributedString.Key: Any]) {
        guard let heightMap, totalLines > 0 else { return }
        let topDocY = max(0, scrollY - topInset)
        let bottomDocY = max(topDocY, scrollY + bounds.height - topInset)
        let firstLocation = heightMap.lineAtY(topDocY)
        let lastLocation = heightMap.lineAtY(bottomDocY)
        let firstLine = max(0, min(firstLocation.line, totalLines - 1))
        let lastLine = max(firstLine, min(lastLocation.line, totalLines - 1))
        let availableWidth = bounds.width - horizontalPadding * 2

        for line in firstLine ... lastLine {
            let docTop = topInset + heightMap.heightAbove(line: line)
            let lineHeightForLine = max(lineHeight, heightMap.heightOfLine(line))
            let gutterY = docTop - scrollY
            let label = String(line + 1) as NSString
            let labelSize = label.size(withAttributes: attributes)
            let originX = horizontalPadding + max(0, availableWidth - labelSize.width)
            let originY = gutterY + (lineHeightForLine - labelSize.height) / 2
            guard originY + labelSize.height >= 0, originY <= bounds.height else { continue }
            label.draw(at: NSPoint(x: originX, y: originY), withAttributes: attributes)
        }
    }

    private func drawTrailingBorder() {
        borderColor.setStroke()
        let path = NSBezierPath()
        path.move(to: NSPoint(x: bounds.maxX - 0.5, y: 0))
        path.line(to: NSPoint(x: bounds.maxX - 0.5, y: bounds.height))
        path.lineWidth = 1
        path.stroke()
    }
}
