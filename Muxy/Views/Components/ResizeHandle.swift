import AppKit
import SwiftUI

struct ResizeHandle: View {
    enum Axis {
        case horizontal
        case vertical
    }

    let axis: Axis
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
                        DragGesture(minimumDistance: 1)
                            .updating($dragging) { _, state, _ in state = true }
                            .onChanged { value in
                                cursor.set()
                                onDrag(value)
                            }
                    )
                    .onHover { on in
                        hovering = on
                        if on {
                            cursor.set()
                        } else if !dragging {
                            NSCursor.arrow.set()
                        }
                    }
            }
    }

    private var cursor: NSCursor {
        axis == .horizontal ? .resizeLeftRight : .resizeUpDown
    }
}
