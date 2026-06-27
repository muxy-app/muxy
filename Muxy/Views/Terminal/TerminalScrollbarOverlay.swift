import AppKit

final class TerminalScrollbarOverlay: NSView {
    var onScrollToRow: ((Int) -> Void)?

    private let scrollView = TerminalScrollbarScrollView()
    private let documentView = NSView()

    private var total = 0
    private var len = 0
    private var offset = 0
    private var cellHeight: CGFloat = 0
    private var isLiveScrolling = false
    private var lastSentRow: Int?
    nonisolated(unsafe) private var observers: [NSObjectProtocol] = []

    private var hasScrollableContent: Bool { cellHeight > 0 && total > len && total > 0 }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        configureScrollView()
        registerScrollObservers()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    deinit {
        observers.forEach { NotificationCenter.default.removeObserver($0) }
    }

    private func configureScrollView() {
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay
        scrollView.drawsBackground = false
        scrollView.contentView.clipsToBounds = false
        scrollView.documentView = documentView
        addSubview(scrollView)
    }

    private func registerScrollObservers() {
        observers.append(NotificationCenter.default.addObserver(
            forName: NSScrollView.willStartLiveScrollNotification,
            object: scrollView,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.isLiveScrolling = true }
        })
        observers.append(NotificationCenter.default.addObserver(
            forName: NSScrollView.didEndLiveScrollNotification,
            object: scrollView,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.isLiveScrolling = false }
        })
        observers.append(NotificationCenter.default.addObserver(
            forName: NSScrollView.didLiveScrollNotification,
            object: scrollView,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.handleLiveScroll() }
        })
    }

    override func layout() {
        super.layout()
        scrollView.frame = bounds
        documentView.frame.size.width = bounds.width
        synchronizeScrollView()
    }

    func update(total: Int, offset: Int, len: Int, cellHeight: CGFloat) {
        self.total = total
        self.len = len
        self.offset = min(max(offset, 0), max(total - len, 0))
        self.cellHeight = cellHeight
        synchronizeScrollView()
    }

    func flash() {
        guard hasScrollableContent else { return }
        scrollView.flashScrollers()
    }

    func flashIfNearScroller(point: NSPoint) {
        guard hasScrollableContent else { return }
        let scrollerWidth = NSScroller.scrollerWidth(for: .regular, scrollerStyle: scrollView.scrollerStyle)
        guard point.x >= bounds.maxX - scrollerWidth * 2 else { return }
        scrollView.flashScrollers()
    }

    private func synchronizeScrollView() {
        guard hasScrollableContent else {
            documentView.frame.size.height = scrollView.contentSize.height
            scrollView.reflectScrolledClipView(scrollView.contentView)
            return
        }

        documentView.frame.size.height = documentHeight()

        guard !isLiveScrolling else {
            scrollView.reflectScrolledClipView(scrollView.contentView)
            return
        }

        let offsetY = CGFloat(total - offset - len) * cellHeight
        scrollView.contentView.scroll(to: CGPoint(x: 0, y: offsetY))
        lastSentRow = offset
        scrollView.reflectScrolledClipView(scrollView.contentView)
    }

    private func documentHeight() -> CGFloat {
        let contentHeight = scrollView.contentSize.height
        let gridHeight = CGFloat(total) * cellHeight
        let padding = contentHeight - (CGFloat(len) * cellHeight)
        return gridHeight + padding
    }

    private func handleLiveScroll() {
        guard hasScrollableContent else { return }
        let visibleRect = scrollView.contentView.documentVisibleRect
        let scrollOffset = documentView.frame.height - visibleRect.origin.y - visibleRect.height
        let row = Int((scrollOffset / cellHeight).rounded())
        let clamped = min(max(row, 0), total - len)
        guard clamped != lastSentRow else { return }
        lastSentRow = clamped
        onScrollToRow?(clamped)
    }
}

private final class TerminalScrollbarScrollView: NSScrollView {
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let scroller = verticalScroller, !scroller.isHidden else { return nil }
        let scrollerPoint = scroller.convert(point, from: superview)
        guard scroller.bounds.contains(scrollerPoint) else { return nil }
        return super.hitTest(point)
    }

    override func scrollWheel(with event: NSEvent) {
        superview?.superview?.scrollWheel(with: event)
    }
}
