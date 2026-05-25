import AppKit
import SwiftUI

struct ResizeHandle: View {
    enum Axis {
        case horizontal
        case vertical
    }

    let axis: Axis
    var onEnd: (() -> Void)?
    let onDrag: (DragGesture.Value) -> Void
    @State private var hovering = false
    @GestureState private var dragging = false

    private var active: Bool { hovering || dragging }

    var body: some View {
        Rectangle()
            .fill(active ? MuxyTheme.accent : MuxyTheme.border)
            .frame(width: axis == .horizontal ? 1 : nil, height: axis == .vertical ? 1 : nil)
            .overlay {
                Color.clear
                    .frame(
                        width: axis == .horizontal ? UIMetrics.resizeHandleHitArea : nil,
                        height: axis == .vertical ? UIMetrics.resizeHandleHitArea : nil
                    )
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture(minimumDistance: 1, coordinateSpace: .global)
                            .updating($dragging) { _, state, _ in state = true }
                            .onChanged { value in
                                cursor.set()
                                onDrag(value)
                            }
                            .onEnded { _ in
                                onEnd?()
                            }
                    )
                    .onContinuousHover { phase in
                        switch phase {
                        case .active:
                            hovering = true
                            cursor.set()
                        case .ended:
                            hovering = false
                            if !dragging {
                                NSCursor.arrow.set()
                            }
                        }
                    }
            }
            .zIndex(1)
    }

    private var cursor: NSCursor {
        axis == .horizontal ? .resizeLeftRight : .resizeUpDown
    }
}

struct PanelResizeHandle: View {
    let axis: ResizeHandle.Axis
    let current: () -> CGFloat
    let apply: (CGFloat) -> Void
    @State private var startValue: CGFloat?

    var body: some View {
        ResizeHandle(
            axis: axis,
            onEnd: { startValue = nil },
            onDrag: { value in
                let start = startValue ?? current()
                if startValue == nil { startValue = start }
                let translation = axis == .horizontal ? value.translation.width : value.translation.height
                apply(start - translation)
            }
        )
        .accessibilityHidden(true)
    }
}
