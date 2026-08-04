import SwiftUI

struct ComposerResizeEdge: View {
    let edge: ResizeHandle.Edge
    let captureAnchor: () -> CGFloat
    let apply: (CGFloat) -> Void

    @State private var anchor: CGFloat?
    @State private var hovering = false
    @State private var dragging = false

    var body: some View {
        ResizeDragArea(
            axis: edge.axis,
            onHoverChange: { hovering = $0 },
            onDraggingChange: { dragging = $0 },
            onEnd: { anchor = nil },
            onDrag: resize
        )
        .overlay(grip.allowsHitTesting(false))
        .accessibilityHidden(true)
    }

    private var isActive: Bool {
        hovering || dragging
    }

    private var grip: some View {
        Capsule()
            .fill(MuxyTheme.fgMuted)
            .frame(
                width: edge.axis == .horizontal ? UIMetrics.scaled(3) : UIMetrics.scaled(28),
                height: edge.axis == .horizontal ? UIMetrics.scaled(28) : UIMetrics.scaled(3)
            )
            .opacity(isActive ? 0.8 : 0)
            .animation(.easeOut(duration: 0.12), value: isActive)
    }

    private func resize(_ value: DragGesture.Value) {
        let start = anchor ?? captureAnchor()
        anchor = start
        let translation = edge.axis == .horizontal ? value.translation.width : value.translation.height
        apply(ComposerLayoutPreferences.resizedLength(
            anchor: start,
            translation: UIMetrics.unscaled(translation),
            isLeadingEdge: edge.isLeading
        ))
    }
}
