import SwiftUI

enum BuiltinPanel {
    static let vcs = "builtin:vcs"
    static let fileTree = "builtin:fileTree"
    static let richInput = "builtin:richInput"
    static let extensionConsole = "builtin:extensionConsole"
}

enum PanelLayoutMetrics {
    static let vcsWidthRange: ClosedRange<CGFloat> = 200 ... 800
    static let vcsDefaultWidth: Double = 400

    static let fileTreeWidthRange: ClosedRange<CGFloat> = 180 ... 600
    static let fileTreeDefaultWidth: Double = 260

    static let richInputWidthRange: ClosedRange<CGFloat> = 280 ... 800
    static let richInputDefaultWidth: Double = 380
    static let richInputHeightRange: ClosedRange<CGFloat> = 120 ... 600
    static let richInputDefaultHeight: Double = 220

    static let consoleHeightRange: ClosedRange<CGFloat> = 120 ... 600
    static let consoleDefaultHeight: Double = 220

    static let extensionWidthRange: ClosedRange<CGFloat> = 240 ... 800
    static let extensionDefaultWidth: Double = 360
    static let extensionHeightRange: ClosedRange<CGFloat> = 160 ... 600
    static let extensionDefaultHeight: Double = 240
}

struct PanelFrame: ViewModifier {
    let position: PanelPosition
    let size: Binding<Double>
    let range: ClosedRange<CGFloat>

    func body(content: Content) -> some View {
        switch position {
        case .right:
            HStack(spacing: 0) {
                handle(axis: .horizontal, edge: .leading)
                content.frame(width: CGFloat(size.wrappedValue))
            }
        case .bottom:
            VStack(spacing: 0) {
                handle(axis: .vertical, edge: .top)
                content.frame(height: CGFloat(size.wrappedValue))
            }
        }
    }

    private func handle(axis: ResizeHandle.Axis, edge: PanelResizeHandle.Edge) -> some View {
        PanelResizeHandle(
            axis: axis,
            edge: edge,
            current: { CGFloat(size.wrappedValue) },
            apply: { next in
                size.wrappedValue = Double(min(range.upperBound, max(range.lowerBound, next)))
            }
        )
    }
}
