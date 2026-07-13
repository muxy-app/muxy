import AppKit
import SwiftUI

struct ResizeHandle: View {
    enum Axis: Equatable {
        case horizontal
        case vertical

        var cursor: NSCursor {
            self == .horizontal ? .resizeLeftRight : .resizeUpDown
        }
    }

    enum HitAreaBias: Equatable {
        case centered
        case leading
        case trailing
    }

    let axis: Axis
    var hitAreaBias: HitAreaBias = .centered
    var onEnd: (() -> Void)?
    let onDrag: (DragGesture.Value) -> Void
    @State private var hovering = false
    @State private var dragCursorPushed = false
    @GestureState private var dragging = false

    private var active: Bool { hovering || dragging }

    var body: some View {
        Rectangle()
            .fill(active ? MuxyTheme.accent : MuxyTheme.border)
            .frame(width: axis == .horizontal ? 1 : nil, height: axis == .vertical ? 1 : nil)
            .overlay(alignment: handleAlignment) {
                Rectangle()
                    .fill(Color.black.opacity(0.001))
                    .frame(
                        width: axis == .horizontal ? UIMetrics.resizeHandleHitArea : nil,
                        height: axis == .vertical ? UIMetrics.resizeHandleHitArea : nil
                    )
                    .background {
                        ResizeCursorRegion(axis: axis) { hovering = $0 }
                    }
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture(minimumDistance: 1, coordinateSpace: .global)
                            .updating($dragging) { _, state, _ in state = true }
                            .onChanged { value in
                                activateDragCursor()
                                onDrag(value)
                            }
                            .onEnded { _ in
                                onEnd?()
                                releaseDragCursor()
                            }
                    )
            }
            .zIndex(1)
            .onChange(of: dragging) { _, isDragging in
                guard !isDragging else { return }
                releaseDragCursor()
            }
            .onDisappear {
                releaseDragCursor()
            }
    }

    private var handleAlignment: Alignment {
        switch hitAreaBias {
        case .centered:
            .center
        case .leading:
            axis == .horizontal ? .leading : .top
        case .trailing:
            axis == .horizontal ? .trailing : .bottom
        }
    }

    private func activateDragCursor() {
        if dragCursorPushed {
            axis.cursor.set()
        } else {
            axis.cursor.push()
            dragCursorPushed = true
        }
    }

    private func releaseDragCursor() {
        guard dragCursorPushed else { return }
        NSCursor.pop()
        dragCursorPushed = false
    }
}

struct ResizeCursorRegion: NSViewRepresentable {
    let axis: ResizeHandle.Axis
    let onHoverChange: (Bool) -> Void

    func makeNSView(context: Context) -> ResizeCursorNSView {
        let view = ResizeCursorNSView(axis: axis)
        view.onHoverChange = onHoverChange
        return view
    }

    func updateNSView(_ nsView: ResizeCursorNSView, context: Context) {
        nsView.axis = axis
        nsView.onHoverChange = onHoverChange
    }
}

final class ResizeCursorNSView: NSView {
    var onHoverChange: ((Bool) -> Void)?
    private var hoverTrackingArea: NSTrackingArea?
    var axis: ResizeHandle.Axis {
        didSet {
            guard oldValue != axis else { return }
            window?.invalidateCursorRects(for: self)
        }
    }

    var cursor: NSCursor {
        axis.cursor
    }

    init(axis: ResizeHandle.Axis) {
        self.axis = axis
        super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    override func resetCursorRects() {
        super.resetCursorRects()
        addCursorRect(bounds, cursor: cursor)
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let hoverTrackingArea {
            removeTrackingArea(hoverTrackingArea)
        }
        let nextTrackingArea = NSTrackingArea(
            rect: .zero,
            options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self
        )
        addTrackingArea(nextTrackingArea)
        hoverTrackingArea = nextTrackingArea
    }

    override func mouseEntered(with event: NSEvent) {
        super.mouseEntered(with: event)
        onHoverChange?(true)
    }

    override func mouseExited(with event: NSEvent) {
        super.mouseExited(with: event)
        onHoverChange?(false)
    }
}

struct AnchoredResizeHandle<Anchor>: View {
    let axis: ResizeHandle.Axis
    var hitAreaBias: ResizeHandle.HitAreaBias = .centered
    let captureAnchor: () -> Anchor
    let onTranslate: (Anchor, CGFloat) -> Void
    @State private var anchor: Anchor?

    var body: some View {
        ResizeHandle(
            axis: axis,
            hitAreaBias: hitAreaBias,
            onEnd: { anchor = nil },
            onDrag: { value in
                let current = anchor ?? captureAnchor()
                anchor = current
                let delta = axis == .horizontal ? value.translation.width : value.translation.height
                onTranslate(current, delta)
            }
        )
    }
}

struct PanelResizeHandle: View {
    enum Edge {
        case leading
        case trailing
        case top
        case bottom

        var hitAreaBias: ResizeHandle.HitAreaBias {
            switch self {
            case .leading,
                 .top:
                .leading
            case .trailing,
                 .bottom:
                .trailing
            }
        }
    }

    let axis: ResizeHandle.Axis
    var edge: Edge = .leading
    let current: () -> CGFloat
    let apply: (CGFloat) -> Void

    var body: some View {
        AnchoredResizeHandle(
            axis: axis,
            hitAreaBias: edge.hitAreaBias,
            captureAnchor: current,
            onTranslate: { start, delta in
                let signed = (edge == .leading || edge == .top) ? -delta : delta
                apply(start + signed)
            }
        )
        .accessibilityHidden(true)
    }
}
