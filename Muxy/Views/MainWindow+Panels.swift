import SwiftUI

enum BuiltinPanel {
    static let vcs = "builtin:vcs"
    static let fileTree = "builtin:fileTree"
    static let richInput = "builtin:richInput"
    static let extensionConsole = "builtin:extensionConsole"
}

enum PanelLayoutMetrics {
    static let extensionWidthRange: ClosedRange<CGFloat> = 240 ... 800
    static let extensionHeightRange: ClosedRange<CGFloat> = 160 ... 600
    static let extensionDefaultWidth: Double = 360
    static let extensionDefaultHeight: Double = 240
}

struct PanelFrame: ViewModifier {
    let position: PanelPosition
    let width: Binding<Double>
    let height: Binding<Double>
    let widthRange: ClosedRange<CGFloat>
    let heightRange: ClosedRange<CGFloat>

    func body(content: Content) -> some View {
        switch position {
        case .right:
            HStack(spacing: 0) {
                handle(axis: .horizontal, edge: .leading, value: width, range: widthRange)
                content.frame(width: CGFloat(width.wrappedValue))
            }
        case .bottom:
            VStack(spacing: 0) {
                handle(axis: .vertical, edge: .top, value: height, range: heightRange)
                content.frame(height: CGFloat(height.wrappedValue))
            }
        }
    }

    private func handle(
        axis: ResizeHandle.Axis,
        edge: PanelResizeHandle.Edge,
        value: Binding<Double>,
        range: ClosedRange<CGFloat>
    ) -> some View {
        PanelResizeHandle(
            axis: axis,
            edge: edge,
            current: { CGFloat(value.wrappedValue) },
            apply: { next in
                value.wrappedValue = Double(min(range.upperBound, max(range.lowerBound, next)))
            }
        )
    }
}
