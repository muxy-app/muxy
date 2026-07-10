import AppKit
import SwiftUI

struct ResizeHandle: View {
    enum Axis {
        case horizontal
        case vertical
    }

    enum HitAreaBias {
        case centered
        case leading
        case trailing
    }

    let axis: Axis
    var hitAreaBias: HitAreaBias = .centered
    var onEnd: (() -> Void)?
    let onDrag: (DragGesture.Value) -> Void
    @State private var hovering = false
    @State private var cursorPushed = false
    @GestureState private var dragging = false

    private var active: Bool { hovering || dragging }

    var body: some View {
        ZStack(alignment: handleAlignment) {
            Rectangle()
                .fill(active ? MuxyTheme.accent : MuxyTheme.border)
                .frame(width: axis == .horizontal ? 1 : nil, height: axis == .vertical ? 1 : nil)
            Rectangle()
                .fill(Color.black.opacity(0.001))
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 1, coordinateSpace: .global)
                        .updating($dragging) { _, state, _ in state = true }
                        .onChanged { value in
                            activateCursor()
                            onDrag(value)
                        }
                        .onEnded { _ in
                            onEnd?()
                            if !hovering {
                                releaseCursor()
                            }
                        }
                )
                .onContinuousHover { phase in
                    switch phase {
                    case .active:
                        hovering = true
                        activateCursor()
                    case .ended:
                        hovering = false
                        if !dragging {
                            releaseCursor()
                        }
                    }
                }
        }
        .frame(
            width: axis == .horizontal ? UIMetrics.resizeHandleHitArea : nil,
            height: axis == .vertical ? UIMetrics.resizeHandleHitArea : nil
        )
        .zIndex(1)
        .onDisappear {
            releaseCursor()
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

    private var cursor: NSCursor {
        axis == .horizontal ? .resizeLeftRight : .resizeUpDown
    }

    private func activateCursor() {
        if cursorPushed {
            cursor.set()
        } else {
            cursor.push()
            cursorPushed = true
        }
    }

    private func releaseCursor() {
        guard cursorPushed else { return }
        NSCursor.pop()
        cursorPushed = false
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
    }

    let axis: ResizeHandle.Axis
    var edge: Edge = .leading
    let current: () -> CGFloat
    let apply: (CGFloat) -> Void

    var body: some View {
        AnchoredResizeHandle(
            axis: axis,
            hitAreaBias: hitAreaBias,
            captureAnchor: current,
            onTranslate: { start, delta in
                let signed = (edge == .leading || edge == .top) ? -delta : delta
                apply(start + signed)
            }
        )
        .accessibilityHidden(true)
    }

    private var hitAreaBias: ResizeHandle.HitAreaBias {
        switch edge {
        case .leading,
             .top:
            .leading
        case .trailing,
             .bottom:
            .trailing
        }
    }
}
